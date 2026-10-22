# CUBIC / BBRv3 comparison

This branch evaluates Aster's Noq with the probe-feedback reset, packet-growth
and stream-compaction corrections. The combined protocol suite passes 464
tests. Production defaults remain CUBIC.

See the [qualification report](results/qualification-report.md),
[comparison tables](results/qualification-tables.md),
[reference review](results/reference-review.md), and
[recovery diagnosis](results/recovery-investigation.md).
The [stream-buffer follow-up](results/stream-fix/report.md) and
[Aster adoption handoff](results/adoption-handoff.md) cover the receive fix,
clean topic branches and ledger work.

This standalone harness compares Aster's pinned Noq 1.4.0
(`535bd88409193ae577f445a3207351bb83525f4e`) with the same implementation
after the probe-feedback resets discussed in
[Noq #819](https://github.com/n0-computer/noq/issues/819) and
[BBR draft PR #114](https://github.com/ietf-wg-ccwg/draft-ietf-ccwg-bbr/pull/114).

The reset-only candidate in the original results is superseded by the
two-fix candidate. Its failed recovery test is retained as historical evidence;
it should not be adopted by itself.

See [results/report.md](results/report.md) for the comparison, tradeoffs and
limitations; [results/summary.csv](results/summary.csv) includes min/max
goodput as well as median throughput and latency; the JSONL files contain
every trial and per-second throughput. [results/validation.txt](results/validation.txt)
records the failing and passing test evidence. Results were collected on
an Apple Silicon Mac using Rust 1.96.0 on 2026-10-22.

## Method

Two real encrypted Noq endpoints use custom in-memory sockets. Each direction
has an independent FIFO serializer, fixed propagation delay, tail-drop queue
and seeded independent packet loss after serialization. The model charges
28-byte IPv4/UDP headers plus the actual encrypted packet size. Thus random
loss consumes link capacity, whereas tail-dropped packets do not. GSO segments
are split before modeling, but the socket advertises no offload capability.
Two calibration tests check serialization/propagation timing and queue drops.

Both endpoints use the selected controller's default parameters. MTU is fixed
at 1200 bytes, MTU discovery is disabled, and flow-control windows are enlarged
identically (8 MiB per stream, 16 MiB connection receive/send). Keep-alive,
ACK policy and other transport settings use the pinned Noq defaults.

A client sends an unlimited bulk upload on a unidirectional stream. Independently,
it schedules a 1 KiB request/echo on a fresh bidirectional stream every 50 ms,
without waiting for the previous reply. This avoids hiding congestion by
slowing the request generator when replies take longer. Latency starts at the
scheduled request time; a five-second deadline counts a failure. The bulk
upload stays active while the last probes drain. Goodput counts delivered
application bytes at the receiver during the fixed measurement interval.

Each virtual-time cell uses five trials, three seconds warm-up and 24 seconds
measured. Trial order alternates the controllers. Packet-loss seeds and
endpoint RNG seeds vary by trial; BBR's private probe RNG remains randomized,
so results are not bit-for-bit deterministic. The recovery scenario lowers
link capacity from 20 to 5 Mbps at link age 8 seconds and restores it at
16 seconds; already scheduled packets retain their original serialization.
Link counters in JSONL cover the handshake/warm-up and measurement, whereas
application goodput and probe percentiles cover only the measured interval.

This measures congestion behaviour under a model, **not** native UDP or CPU
throughput. Virtual time excludes CPU scheduling costs and uses Tokio's
approximately 1 ms timer resolution. The wall-clock option retains the same
in-memory network but uses real time as a sensitivity check. No jitter,
ECN, competing connections or video priorities are modeled. This is not a
reproduction of the issue's exact workload and does not satisfy Salvo's
Quinn-versus-Noq H3/WebTransport/Linux/GSO adoption gate.

## Reproduce

Run from the Noq worktree root. `baseline` compiles the exact pinned Git
source; the default `candidate` feature compiles this worktree's patched
source. Select exactly one feature. The lockfile for this standalone package
is tracked so the benchmark dependency versions can be reproduced.

```sh
rtk proxy cargo test --locked --release --manifest-path bench/cc-compare/Cargo.toml
rtk proxy cargo run --locked --release --manifest-path bench/cc-compare/Cargo.toml \
  --no-default-features --features baseline -- \
  --trials 5 --warmup 3 --seconds 24 > bench/cc-compare/results/baseline.jsonl
rtk proxy cargo run --locked --release --manifest-path bench/cc-compare/Cargo.toml -- \
  --trials 10 --warmup 3 --seconds 24 > bench/cc-compare/results/patched-full-matrix.jsonl
rtk proxy python3 bench/cc-compare/summarize.py
```

Use `--scenario wan` to select one case and `--wall-clock` to use real timers.
The included wall-clock checks use one trial per cell, two seconds warm-up
and six seconds measured. They are short sanity checks, not equivalent to
the longer virtual-time samples.

The candidate patch adds `ACKS_INIT`, initializes the ACK phase to it, and
clears the probe-sample flag and resets the ACK phase at the end of probe
feedback. Crucially, these resets occur even for app-limited samples or
outside ProbeBW; only bandwidth-filter advancement remains conditional.
It also improves the existing recovery test's failure diagnostic without
weakening its assertion. It is not a full port of every change in draft 06.

Run the state-transition tests and the full suite from the Noq worktree root:

```sh
rtk cargo test --offline -p noq-proto probe_feedback_ --lib
rtk cargo test --offline -p noq-proto --lib
```

Both commands pass for the two-fix candidate.

## Native Linux and physical paths

`src/bin/native.rs` uses native encrypted UDP endpoints and receiver-measured
bulk downloads, with independent 1 KiB RPC echoes every 50 ms. It checks echo
payloads, counts deadline failures, and keeps bulk active while probes drain.
Both endpoints select the same controller. Flow-control windows are identical
and deliberately generous; these are congestion-control comparisons rather
than tests of Aster's exact application settings.

Build with `cargo build --locked --release --manifest-path
bench/cc-compare/Cargo.toml --bin native`. For a standalone smoke run:

```sh
rtk proxy cargo run --locked --release --manifest-path bench/cc-compare/Cargo.toml --bin native -- \
  --server --once --controller bbr3 --cert /tmp/noq-cert.der
# In another terminal, after the certificate exists:
rtk proxy cargo run --locked --release --manifest-path bench/cc-compare/Cargo.toml --bin native -- \
  --controller bbr3 --cert /tmp/noq-cert.der --seconds 12 --warmup 2
```

`native_matrix.py` creates two endpoint namespaces and a forwarding-router
namespace. Propagation/loss are separate from the HTB/bfifo bottleneck queue.
It changes no host interface, host sysctl or host firewall. Cleanup runs in
`finally`, including on interruption. Run as root with `--binary`, `--output`
and `--trials 3`; linbox needs `--unseeded-netem` for its older iproute2.
The core eight cases run by default. Select supplementary cases using
`--only ecn-aqm,jitter-reorder,high-bdp`.

`native_paths.py` is the environment-specific physical-path driver for the
three authorized test hosts. It uses a temporary source-restricted Hetzner
UDP ingress rule and removes that exact rule in `finally`. Servers have
timeouts and owned process groups. Use it only while those hosts and their
documented access rules remain applicable. It expects the accounting-enabled
Linux binary at `/tmp/noq-bbr-eval-20261022/native-v2` on each host, plus
`--local-binary` for the Mac and `--output` for the results directory.

Rebuild the tables and CSV from downloaded raw evidence with:

```sh
rtk proxy python3 bench/cc-compare/analyse_qualification.py
```
