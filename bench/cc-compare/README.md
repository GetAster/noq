# CUBIC / BBRv3 comparison

**Follow-up investigation:** this branch includes the packet-growth correction
that resolves the reset-only candidate's failing recovery test. See
[the recovery diagnosis and validation](results/recovery-investigation.md)
for current status and commands. The original comparison and its reset-only
results below are retained as historical evidence from `bench/bbr3-vs-cubic-v1.4.0`.

This standalone harness compares Aster's pinned Noq 1.4.0
(`535bd88409193ae577f445a3207351bb83525f4e`) with the same implementation
after the probe-feedback resets discussed in
[Noq #819](https://github.com/n0-computer/noq/issues/819) and
[BBR draft PR #114](https://github.com/ietf-wg-ccwg/draft-ietf-ccwg-bbr/pull/114).

**The reset patch is experimental and must not be adopted as-is.** It fixes
three state-transition regressions but fails an existing bandwidth-recovery
test. The benchmark also measures a recovery regression. It changes no
production congestion-controller defaults.

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
  --controller bbr3 --trials 5 --warmup 3 --seconds 24 > bench/cc-compare/results/probe-reset.jsonl
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

The second command currently has one known failure:
`probe_up_rediscovers_full_bw_after_10x_increase`. Resolve that regression
and validate real networks before recommending a default switch.
