//! Native UDP comparison: receiver-measured bulk download with independent RPC probes.
#[cfg(any(
    all(feature = "candidate", feature = "baseline"),
    not(any(feature = "candidate", feature = "baseline"))
))]
compile_error!("select exactly one of candidate or baseline");
#[cfg(feature = "baseline")]
use noq_baseline as noq;
#[cfg(feature = "baseline")]
const BUILD_VARIANT: &str = "baseline";
#[cfg(feature = "candidate")]
const BUILD_VARIANT: &str = "probe-reset-and-packet-growth";

use anyhow::{Result, ensure};
use bytes::Bytes;
use clap::Parser;
use serde::Serialize;
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    task::JoinSet,
    time::{Instant, sleep, sleep_until, timeout},
};

#[derive(Parser)]
struct Opt {
    #[arg(long)]
    server: bool,
    /// Exit the server after one connection, allowing resource accounting to complete.
    #[arg(long)]
    once: bool,
    #[arg(long, default_value = "127.0.0.1:44330")]
    address: SocketAddr,
    #[arg(long, default_value = "cubic", value_parser = ["cubic", "bbr3"])]
    controller: String,
    #[arg(long, default_value = "cert.der")]
    cert: PathBuf,
    #[arg(long, default_value_t = 24)]
    seconds: u32,
    #[arg(long, default_value_t = 3)]
    warmup: u32,
    #[arg(long)]
    no_gso: bool,
    #[arg(long)]
    fixed_mtu: bool,
    #[arg(long)]
    rpc_only: bool,
    /// Retain Noq's default flow-control windows instead of enlarged benchmark windows.
    #[arg(long)]
    default_flow_control: bool,
    #[arg(long, default_value = "native")]
    label: String,
    #[arg(long, default_value_t = 0)]
    trial: u32,
}

fn transport(opt: &Opt) -> Arc<noq::TransportConfig> {
    let mut cfg = noq::TransportConfig::default();
    cfg.congestion_controller_factory(match opt.controller.as_str() {
        "bbr3" => Arc::new(noq::congestion::Bbr3Config::default()),
        _ => Arc::new(noq::congestion::CubicConfig::default()),
    });
    if !opt.default_flow_control {
        cfg.stream_receive_window((32u32 * 1024 * 1024).into());
        cfg.receive_window((64u32 * 1024 * 1024).into());
        cfg.send_window(64 * 1024 * 1024);
    }
    cfg.enable_segmentation_offload(!opt.no_gso);
    if opt.fixed_mtu {
        cfg.initial_mtu(1200)
            .min_mtu(1200)
            .mtu_discovery_config(None);
    }
    Arc::new(cfg)
}

async fn serve(opt: &Opt) -> Result<()> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let der = rustls::pki_types::CertificateDer::from(cert.cert);
    std::fs::write(&opt.cert, der.as_ref())?;
    let mut cfg = noq::ServerConfig::with_single_cert(vec![der], key.into())?;
    cfg.transport = transport(opt);
    let ep = noq::Endpoint::server(cfg, opt.address)?;
    eprintln!("ready {} {}", opt.address, opt.controller);
    while let Some(incoming) = ep.accept().await {
        let task = tokio::spawn(async move {
            let conn = incoming.await?;
            let mut tasks = JoinSet::new();
            loop {
                let (mut tx, mut rx) = match conn.accept_bi().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                let conn = conn.clone();
                tasks.spawn(async move {
                    let data = rx.read_to_end(4096).await?;
                    tx.write_all(&data).await?;
                    tx.finish()?;
                    if data == b"bulk" {
                        let mut bulk = conn.open_uni().await?;
                        let bytes = Bytes::from(vec![0x55; 64 * 1024]);
                        loop {
                            bulk.write_chunk(bytes.clone()).await?;
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                });
                while let Some(result) = tasks.try_join_next() {
                    if let Ok(Err(err)) = result {
                        eprintln!("stream: {err}");
                    }
                }
            }
            eprintln!("connection end {:?}", conn.stats());
            eprintln!("path end {:?}", conn.path_stats(noq::PathId::ZERO));
            tasks.abort_all();
            Ok::<_, anyhow::Error>(())
        });
        if opt.once {
            task.await??;
            break;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct Row {
    variant: &'static str,
    label: String,
    controller: String,
    trial: u32,
    seconds: u32,
    warmup: u32,
    gso: bool,
    fixed_mtu: bool,
    rpc_only: bool,
    default_flow_control: bool,
    bulk_bytes: u64,
    bulk_mbps: f64,
    per_second_mbps: Vec<f64>,
    probes: usize,
    failed_probes: usize,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    connection_stats: String,
}
fn percentile(x: &[f64], p: f64) -> f64 {
    if x.is_empty() {
        return 0.;
    }
    x[((x.len() as f64 * p).ceil() as usize)
        .saturating_sub(1)
        .min(x.len() - 1)]
}

async fn client(opt: &Opt) -> Result<()> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(rustls::pki_types::CertificateDer::from(std::fs::read(
        &opt.cert,
    )?))?;
    let mut cfg = noq::ClientConfig::with_root_certificates(Arc::new(roots))?;
    cfg.transport_config(transport(opt));
    let ep = noq::Endpoint::client("0.0.0.0:0".parse()?)?;
    let conn = timeout(
        Duration::from_secs(10),
        ep.connect_with(cfg, opt.address, "localhost")?,
    )
    .await??;
    let received = Arc::new(AtomicU64::new(0));
    let mut workers = JoinSet::new();
    if !opt.rpc_only {
        let (mut tx, mut rx) = conn.open_bi().await?;
        tx.write_all(b"bulk").await?;
        tx.finish()?;
        ensure!(rx.read_to_end(4096).await? == b"bulk", "bulk start reply");
        workers.spawn({
            let conn = conn.clone();
            let received = received.clone();
            async move {
                let mut rx = conn.accept_uni().await?;
                while let Some(data) = rx.read_chunk(64 * 1024).await? {
                    received.fetch_add(data.len() as u64, Ordering::Relaxed);
                }
                Ok::<_, anyhow::Error>(())
            }
        });
    }
    sleep(Duration::from_secs(opt.warmup.into())).await;
    let first = received.load(Ordering::Relaxed);
    let start = Instant::now();
    let end = start + Duration::from_secs(opt.seconds.into());
    let probes = tokio::spawn({
        let conn = conn.clone();
        async move {
            let mut tasks = JoinSet::new();
            let mut next = start;
            while next < end {
                sleep_until(next).await;
                let conn = conn.clone();
                let due = next;
                tasks.spawn(async move {
                    timeout(Duration::from_secs(5), async {
                        let (mut tx, mut rx) = conn.open_bi().await?;
                        tx.write_all(&[0x42; 1024]).await?;
                        tx.finish()?;
                        ensure!(rx.read_to_end(4096).await? == [0x42; 1024], "echo payload");
                        Ok::<_, anyhow::Error>(due.elapsed().as_secs_f64() * 1000.)
                    })
                    .await
                });
                next += Duration::from_millis(50);
            }
            let mut latencies = Vec::new();
            let mut failed = 0;
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok(Ok(ms))) => latencies.push(ms),
                    _ => failed += 1,
                }
            }
            (latencies, failed)
        }
    });
    let mut last = first;
    let mut rates = Vec::new();
    for sec in 1..=opt.seconds {
        sleep_until(start + Duration::from_secs(sec.into())).await;
        let count = received.load(Ordering::Relaxed);
        rates.push((count - last) as f64 * 8. / 1e6);
        last = count;
    }
    let bulk_bytes = last - first;
    let (mut latencies, failed) = probes.await?;
    latencies.sort_by(f64::total_cmp);
    if let Some(result) = workers.try_join_next() {
        result??;
    }
    let row = Row {
        variant: BUILD_VARIANT,
        label: opt.label.clone(),
        controller: opt.controller.clone(),
        trial: opt.trial,
        seconds: opt.seconds,
        warmup: opt.warmup,
        gso: !opt.no_gso,
        fixed_mtu: opt.fixed_mtu,
        rpc_only: opt.rpc_only,
        default_flow_control: opt.default_flow_control,
        bulk_bytes,
        bulk_mbps: bulk_bytes as f64 * 8. / (opt.seconds as f64 * 1e6),
        per_second_mbps: rates,
        probes: latencies.len(),
        failed_probes: failed,
        p50_ms: percentile(&latencies, 0.50),
        p95_ms: percentile(&latencies, 0.95),
        p99_ms: percentile(&latencies, 0.99),
        connection_stats: format!("{:?}", conn.stats()),
    };
    println!("{}", serde_json::to_string(&row)?);
    workers.abort_all();
    conn.close(0u32.into(), b"done");
    ep.close(0u32.into(), b"done");
    ep.wait_idle().await;
    Ok(())
}

fn main() -> Result<()> {
    let opt = Opt::parse();
    ensure!(opt.seconds > 0, "seconds must be positive");
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            if opt.server {
                serve(&opt).await
            } else {
                client(&opt).await
            }
        })
}
