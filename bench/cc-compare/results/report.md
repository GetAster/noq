# Noq congestion-control benchmark results

2026-10-22, Apple Silicon macOS, Rust 1.96.0. Baseline: Noq 1.4.0 at `535bd88409193ae577f445a3207351bb83525f4e`. Candidate: the same code with probe-feedback resets and ACKS_INIT initialization.

**The reset candidate is not ready for adoption: an existing bandwidth-recovery test fails.**

Cells show application goodput in Mbps / small-request p95 latency in ms.

| Emulated scenario | CUBIC | Current BBRv3 | BBRv3 with probe resets |
| --- | ---: | ---: | ---: |
| clean | 18.84 / 120 | 17.29 / 35 | 18.39 / 46 |
| wan | 18.84 / 178 | 16.63 / 117 | 18.27 / 142 |
| loss-0.1pct | 4.99 / 95 | 16.63 / 118 | 17.77 / 142 |
| loss-1pct | 1.21 / 119 | 14.47 / 97 | 17.02 / 113 |
| deep-queue | 19.03 / 541 | 16.64 / 59 | 18.31 / 67 |
| shallow-queue | 17.42 / 91 | 10.14 / 84 | 10.54 / 92 |
| recovery | 4.88 / 108 | 10.83 / 160 | 9.67 / 183 |
| rpc-only | 0.00 / 82 | 0.00 / 82 | 0.00 / 82 |

120 virtual-time trials; 57,600 completed probes; 0 failed probes.

Five trials per cell, 3 seconds warm-up plus 24 seconds measured. Values are medians of per-trial results; latency percentiles are not pooled. All bottlenecks are 20 Mbps per direction, including IPv4/UDP headers. RTT is 20 ms (clean), 40 ms (deep queue), or 80 ms (other cases). Queues are 100 ms except 500 ms deep / 10 ms shallow. Recovery changes 20→5→20 Mbps at simulated link ages 8 and 16 seconds. RPC-only has no bulk transfer and 0.1% random loss; zero bulk goodput is intentional.

## Interpretation

The resets increase sustained throughput in several scenarios, but increase tail latency in several of those cases and worsen recovery in this experiment. Current BBRv3 substantially outperforms CUBIC under random loss and holds shorter deep queues; CUBIC wins throughput on clean and shallow-buffer paths. Neither this comparison nor the issue report supports changing the default globally.

The reset candidate passes the three new state-transition tests, which fail on the baseline. It fails the existing `probe_up_rediscovers_full_bw_after_10x_increase` test, which passes on the baseline. The original suite passes 456 tests; the candidate passes 458 with 1 failure. See [validation.txt](validation.txt).

## Limits

Real encrypted Noq endpoints and congestion/pacing/loss-recovery code execute, but packet I/O is in memory and Tokio time is virtual. These are algorithm-level network-model results, not measured Internet throughput or CPU/OS/GSO performance. MTU is fixed at 1200; flow-control windows are enlarged identically; no jitter, cross-connection competition, ECN or offload is modeled. Small requests use independent bidirectional streams (1 KiB request and response, every 50 ms), concurrently with an unlimited upload on a unidirectional stream. A five-second timeout is counted as a failed probe. Bulk stays active while final probes drain. Loss uses fixed per-trial/direction seeds; BBR's internal probe RNG remains random. The clock has roughly 1 ms timer resolution. This does not reproduce the issue's specific jitter/100 MB transfer or two-priority video-track workload, and it does not satisfy the Salvo Quinn-versus-Noq H3/WebTransport/Linux performance gate.

## Wall-clock sanity checks

The same in-memory link runs with real wall-clock timers. These short runs check timer/model sensitivity; they still do not exercise native UDP/GSO.

| Scenario | CUBIC | Current BBRv3 | BBRv3 with probe resets |
| --- | ---: | ---: | ---: |
| wan | 18.85 / 158 | 17.97 / 142 | 18.83 / 163 |
| loss-1pct | 1.64 / 111 | 14.93 / 97 | 17.34 / 150 |

One short trial per cell: 2 seconds warm-up plus 6 seconds measured; do not compare these directly to the longer virtual runs as equivalent samples.

Sources: [Noq #819](https://github.com/n0-computer/noq/issues/819) and [BBR draft correction #114](https://github.com/ietf-wg-ccwg/draft-ietf-ccwg-bbr/pull/114).
