//! Real encrypted Noq endpoints on an in-memory rate/delay/loss-limited link.
//! Virtual time measures congestion-control behaviour, not CPU or OS throughput.
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
const BUILD_VARIANT: &str = "probe-reset-packet-growth-and-stream-coalescing";

use std::{
    io::{self, IoSliceMut, Write},
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll, ready},
    time::Duration,
};

use anyhow::{Result, ensure};
use bytes::Bytes;
use clap::Parser;
use noq::{
    AsyncUdpSocket, UdpSender,
    udp::{RecvMeta, Transmit},
};
use serde::Serialize;
use tokio::{
    sync::mpsc,
    task::{JoinHandle, JoinSet},
    time::{Instant, sleep, sleep_until, timeout},
};

#[derive(Parser)]
struct Opt {
    #[arg(long, default_value_t = 5)]
    trials: u32,
    #[arg(long, default_value_t = 24)]
    seconds: u32,
    #[arg(long, default_value_t = 3)]
    warmup: u32,
    #[arg(long)]
    scenario: Option<String>,
    #[arg(long)]
    controller: Option<String>,
    #[arg(long)]
    wall_clock: bool,
    #[arg(long, default_value = BUILD_VARIANT)]
    variant: String,
}

#[derive(Clone, Copy, Serialize)]
struct Scenario {
    name: &'static str,
    mbps: f64,
    rtt_ms: u64,
    queue_ms: u64,
    loss_ppm: u64,
    bulk: bool,
    recovery: bool,
}
const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "clean",
        mbps: 20.,
        rtt_ms: 20,
        queue_ms: 100,
        loss_ppm: 0,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "wan",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 100,
        loss_ppm: 0,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "loss-0.1pct",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 100,
        loss_ppm: 1_000,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "loss-1pct",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 100,
        loss_ppm: 10_000,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "deep-queue",
        mbps: 20.,
        rtt_ms: 40,
        queue_ms: 500,
        loss_ppm: 0,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "shallow-queue",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 10,
        loss_ppm: 0,
        bulk: true,
        recovery: false,
    },
    Scenario {
        name: "recovery",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 100,
        loss_ppm: 1_000,
        bulk: true,
        recovery: true,
    },
    Scenario {
        name: "rpc-only",
        mbps: 20.,
        rtt_ms: 80,
        queue_ms: 100,
        loss_ppm: 1_000,
        bulk: false,
        recovery: false,
    },
];

#[derive(Clone, Copy, Debug, Default, Serialize)]
struct Counters {
    packets: u64,
    random_drops: u64,
    queue_drops: u64,
}

#[derive(Debug)]
struct LinkState {
    departure: Instant,
    rng: u64,
    counters: Counters,
}
#[derive(Clone, Debug)]
struct Sender {
    addr: SocketAddr,
    origin: Instant,
    scenario: ScenarioDebug,
    state: Arc<Mutex<LinkState>>,
    outbox: mpsc::UnboundedSender<(Instant, Bytes)>,
}
#[derive(Clone, Copy, Debug)]
struct ScenarioDebug {
    mbps: f64,
    rtt_ms: u64,
    queue_ms: u64,
    loss_ppm: u64,
    recovery: bool,
}

impl UdpSender for Sender {
    fn poll_send(
        self: Pin<&mut Self>,
        transmit: &Transmit<'_>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let now = Instant::now();
        let cfg = self.scenario;
        let seconds = now.duration_since(self.origin).as_secs_f64();
        let rate = if cfg.recovery && (8.0..16.0).contains(&seconds) {
            5.0
        } else {
            cfg.mbps
        };
        let mut state = self.state.lock().unwrap();
        for bytes in transmit
            .contents
            .chunks(transmit.segment_size.unwrap_or(transmit.contents.len()))
        {
            state.counters.packets += 1;
            // Charge IPv4/UDP headers as well as the encrypted UDP payload.
            let serialization =
                Duration::from_secs_f64((bytes.len() + 28) as f64 * 8.0 / (rate * 1e6));
            let departure = state.departure.max(now) + serialization;
            if departure.duration_since(now) > Duration::from_millis(cfg.queue_ms) {
                state.counters.queue_drops += 1;
                continue;
            }
            state.departure = departure;
            state.rng ^= state.rng << 13;
            state.rng ^= state.rng >> 7;
            state.rng ^= state.rng << 17;
            if state.rng % 1_000_000 < cfg.loss_ppm {
                state.counters.random_drops += 1;
                continue;
            }
            let due = departure + Duration::from_micros(cfg.rtt_ms * 500);
            let _ = self.outbox.send((due, Bytes::copy_from_slice(bytes)));
        }
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug)]
struct Socket {
    sender: Sender,
    inbox: mpsc::UnboundedReceiver<Bytes>,
    peer: SocketAddr,
}
impl AsyncUdpSocket for Socket {
    fn create_sender(&self) -> Pin<Box<dyn UdpSender>> {
        Box::pin(self.sender.clone())
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.sender.addr)
    }
    fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let Some(data) = ready!(self.inbox.poll_recv(cx)) else {
            return Poll::Pending;
        };
        assert!(data.len() <= bufs[0].len());
        bufs[0][..data.len()].copy_from_slice(&data);
        meta[0] = RecvMeta::default();
        meta[0].addr = self.peer;
        meta[0].dst_ip = Some(self.sender.addr.ip());
        meta[0].len = data.len();
        meta[0].stride = data.len();
        Poll::Ready(Ok(1))
    }
}

fn link(
    addr: SocketAddr,
    cfg: Scenario,
    seed: u64,
    target: mpsc::UnboundedSender<Bytes>,
) -> (Sender, JoinHandle<()>) {
    let (outbox, mut packets) = mpsc::unbounded_channel::<(Instant, Bytes)>();
    let origin = Instant::now();
    let sender = Sender {
        addr,
        origin,
        scenario: ScenarioDebug {
            mbps: cfg.mbps,
            rtt_ms: cfg.rtt_ms,
            queue_ms: cfg.queue_ms,
            loss_ppm: cfg.loss_ppm,
            recovery: cfg.recovery,
        },
        state: Arc::new(Mutex::new(LinkState {
            departure: origin,
            rng: seed.max(1),
            counters: Counters::default(),
        })),
        outbox,
    };
    let task = tokio::spawn(async move {
        // Serialization makes due times monotonic within each directional link.
        while let Some((due, data)) = packets.recv().await {
            sleep_until(due).await;
            if target.send(data).is_err() {
                break;
            }
        }
    });
    (sender, task)
}

fn transport(controller: &str) -> Arc<noq::TransportConfig> {
    let mut cfg = noq::TransportConfig::default();
    cfg.congestion_controller_factory(match controller {
        "cubic" => Arc::new(noq::congestion::CubicConfig::default()),
        "bbr3" => Arc::new(noq::congestion::Bbr3Config::default()),
        _ => panic!("unknown controller"),
    });
    // Hold MTU constant and make flow control generous for both controllers.
    cfg.initial_mtu(1200)
        .min_mtu(1200)
        .mtu_discovery_config(None);
    cfg.stream_receive_window((8u32 * 1024 * 1024).into());
    cfg.receive_window((16u32 * 1024 * 1024).into());
    cfg.send_window(16 * 1024 * 1024);
    Arc::new(cfg)
}

#[derive(Serialize)]
struct Row {
    variant: String,
    clock: &'static str,
    controller: String,
    trial: u32,
    scenario: Scenario,
    measurement_seconds: u32,
    warmup_seconds: u32,
    bulk_mbps: f64,
    bulk_bytes: u64,
    per_second_mbps: Vec<f64>,
    probes: usize,
    failed_probes: usize,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    forward: Counters,
    reverse: Counters,
}
fn percentile(samples: &[f64], percent: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples[((samples.len() as f64 * percent).ceil() as usize)
        .saturating_sub(1)
        .min(samples.len() - 1)]
}

async fn run(cfg: Scenario, controller: &str, trial: u32, opt: &Opt) -> Result<Row> {
    let a: SocketAddr = "127.0.0.1:40000".parse()?;
    let b: SocketAddr = "127.0.0.1:40001".parse()?;
    let (a_tx, a_rx) = mpsc::unbounded_channel();
    let (b_tx, b_rx) = mpsc::unbounded_channel();
    let (forward, forward_task) = link(a, cfg, trial as u64 * 1234567 + 1, b_tx);
    let (reverse, reverse_task) = link(b, cfg, trial as u64 * 7654321 + 2, a_tx);
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let der = rustls::pki_types::CertificateDer::from(cert.cert);
    let mut server_cfg = noq::ServerConfig::with_single_cert(vec![der.clone()], key.into())?;
    server_cfg.transport = transport(controller);
    let mut roots = rustls::RootCertStore::empty();
    roots.add(der)?;
    let crypto = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut client_cfg = noq::ClientConfig::new(Arc::new(
        noq::crypto::rustls::QuicClientConfig::try_from(crypto)?,
    ));
    client_cfg.transport_config(transport(controller));
    let runtime = Arc::new(noq::TokioRuntime);
    let mut ecfg = noq::EndpointConfig::default();
    ecfg.rng_seed(Some([trial as u8 + 1; 32]));
    let server_ep = noq::Endpoint::new_with_abstract_socket(
        ecfg.clone(),
        Some(server_cfg),
        Box::new(Socket {
            sender: reverse.clone(),
            inbox: b_rx,
            peer: a,
        }),
        runtime.clone(),
    )?;
    let client_ep = noq::Endpoint::new_with_abstract_socket(
        ecfg,
        None,
        Box::new(Socket {
            sender: forward.clone(),
            inbox: a_rx,
            peer: b,
        }),
        runtime,
    )?;
    let server_accept = tokio::spawn({
        let ep = server_ep.clone();
        async move { ep.accept().await.unwrap().await }
    });
    let client = timeout(
        Duration::from_secs(10),
        client_ep.connect_with(client_cfg, b, "localhost")?,
    )
    .await??;
    let server = server_accept.await??;
    let received = Arc::new(AtomicU64::new(0));
    let mut workers = JoinSet::new();
    workers.spawn({
        let conn = server.clone();
        let received = received.clone();
        async move {
            let mut recv = conn.accept_uni().await?;
            while let Some(data) = recv.read_chunk(64 * 1024).await? {
                received.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
            Ok::<_, anyhow::Error>(())
        }
    });
    workers.spawn({
        let conn = server.clone();
        async move {
            let mut echoes = JoinSet::new();
            loop {
                let (mut send, mut recv) = conn.accept_bi().await?;
                echoes.spawn(async move {
                    let data = recv.read_to_end(4096).await?;
                    send.write_all(&data).await?;
                    send.finish()?;
                    Ok::<_, anyhow::Error>(())
                });
                while let Some(result) = echoes.try_join_next() {
                    result??;
                }
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        }
    });
    if cfg.bulk {
        workers.spawn({
            let conn = client.clone();
            async move {
                let mut stream = conn.open_uni().await?;
                let payload = Bytes::from(vec![0x55; 64 * 1024]);
                loop {
                    stream.write_chunk(payload.clone()).await?;
                }
                #[allow(unreachable_code)]
                Ok::<_, anyhow::Error>(())
            }
        });
    }
    sleep(Duration::from_secs(opt.warmup.into())).await;
    let first = received.load(Ordering::Relaxed);
    let start = Instant::now();
    let deadline = start + Duration::from_secs(opt.seconds.into());
    let probes = tokio::spawn({
        let conn = client.clone();
        async move {
            let mut tasks = JoinSet::new();
            let mut next = start;
            while next < deadline {
                sleep_until(next).await;
                let conn = conn.clone();
                let due = next;
                tasks.spawn(async move {
                    timeout(Duration::from_secs(5), async {
                        let (mut send, mut recv) = conn.open_bi().await?;
                        send.write_all(&[0x42; 1024]).await?;
                        send.finish()?;
                        let reply = recv.read_to_end(4096).await?;
                        ensure!(reply == [0x42; 1024], "bad echo payload");
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
    for second in 1..=opt.seconds {
        sleep_until(start + Duration::from_secs(second.into())).await;
        let count = received.load(Ordering::Relaxed);
        rates.push((count - last) as f64 * 8.0 / 1e6);
        last = count;
    }
    let bulk_bytes = last - first;
    let forward_counts = forward.state.lock().unwrap().counters;
    let reverse_counts = reverse.state.lock().unwrap().counters;
    // Keep the bulk transfer active while the last probes complete.
    let (mut latencies, failed) = probes.await?;
    latencies.sort_by(f64::total_cmp);
    for _ in 0..workers.len() {
        if let Some(result) = workers.try_join_next() {
            result??;
        }
    }
    workers.abort_all();
    client.close(0u32.into(), b"benchmark done");
    server.close(0u32.into(), b"benchmark done");
    while workers.join_next().await.is_some() {}
    client_ep.close(0u32.into(), b"done");
    server_ep.close(0u32.into(), b"done");
    client_ep.wait_idle().await;
    server_ep.wait_idle().await;
    forward_task.abort();
    reverse_task.abort();
    Ok(Row {
        variant: opt.variant.clone(),
        clock: if opt.wall_clock { "wall" } else { "virtual" },
        controller: controller.into(),
        trial,
        scenario: cfg,
        measurement_seconds: opt.seconds,
        warmup_seconds: opt.warmup,
        bulk_mbps: bulk_bytes as f64 * 8.0 / (opt.seconds as f64 * 1e6),
        bulk_bytes,
        per_second_mbps: rates,
        probes: latencies.len(),
        failed_probes: failed,
        p50_ms: percentile(&latencies, 0.50),
        p95_ms: percentile(&latencies, 0.95),
        p99_ms: percentile(&latencies, 0.99),
        forward: forward_counts,
        reverse: reverse_counts,
    })
}

fn main() -> Result<()> {
    let opt = Opt::parse();
    ensure!(
        opt.seconds > 0 && opt.trials > 0,
        "seconds/trials must be positive"
    );
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(!opt.wall_clock)
        .build()?;
    runtime.block_on(async {
        let mut runs = 0;
        for &scenario in SCENARIOS {
            if opt.scenario.as_ref().is_some_and(|s| s != scenario.name) {
                continue;
            }
            for trial in 0..opt.trials {
                // Alternate controller order to reduce wall-clock order bias.
                let order = if trial % 2 == 0 {
                    ["cubic", "bbr3"]
                } else {
                    ["bbr3", "cubic"]
                };
                for controller in order {
                    if opt.controller.as_ref().is_some_and(|c| c != controller) {
                        continue;
                    }
                    let row = run(scenario, controller, trial, &opt).await?;
                    eprintln!(
                        "{} {} trial {}: {:.2} Mbps, p95 {:.1} ms, {}/{} failed probes",
                        scenario.name,
                        controller,
                        trial,
                        row.bulk_mbps,
                        row.p95_ms,
                        row.failed_probes,
                        row.probes + row.failed_probes
                    );
                    println!("{}", serde_json::to_string(&row)?);
                    io::stdout().flush()?;
                    runs += 1;
                }
            }
        }
        ensure!(runs > 0, "no scenario/controller matched");
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn link_accounts_for_serialization_and_propagation() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let cfg = Scenario {
            mbps: 8.,
            rtt_ms: 20,
            queue_ms: 100,
            ..SCENARIOS[0]
        };
        let (mut sender, task) = link("127.0.0.1:1".parse().unwrap(), cfg, 1, tx);
        let transmit = Transmit {
            destination: "127.0.0.1:2".parse().unwrap(),
            ecn: None,
            contents: &[0; 972],
            segment_size: None,
            src_ip: None,
        };
        let start = Instant::now();
        for _ in 0..10 {
            std::future::poll_fn(|cx| Pin::new(&mut sender).poll_send(&transmit, cx))
                .await
                .unwrap();
        }
        for _ in 0..10 {
            rx.recv().await.unwrap();
        }
        // 10 x 1000 bytes at 8 Mbps = 10ms, plus 10ms propagation.
        let ms = start.elapsed().as_millis();
        assert!((20..=22).contains(&ms), "{ms}");
        task.abort();
    }
    #[tokio::test(start_paused = true)]
    async fn link_tail_drops_when_queue_is_full() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let cfg = Scenario {
            mbps: 8.,
            queue_ms: 2,
            ..SCENARIOS[0]
        };
        let (mut sender, task) = link("127.0.0.1:1".parse().unwrap(), cfg, 1, tx);
        let transmit = Transmit {
            destination: "127.0.0.1:2".parse().unwrap(),
            ecn: None,
            contents: &[0; 972],
            segment_size: None,
            src_ip: None,
        };
        for _ in 0..3 {
            std::future::poll_fn(|cx| Pin::new(&mut sender).poll_send(&transmit, cx))
                .await
                .unwrap();
        }
        assert_eq!(sender.state.lock().unwrap().counters.queue_drops, 1);
        task.abort();
    }
}
