# Noq BBRv3 fixes and CUBIC comparison

We compared CUBIC with two BBRv3 fixes following [Noq #819](https://github.com/n0-computer/noq/issues/819),
using [Aster's Noq 1.4.0 fork at 535bd884](https://forge.emrul.dev/Aster/noq/commit/535bd88409193ae577f445a3207351bb83525f4e).
Patched BBRv3 improved goodput under independent random loss and reduced
deep-queue latency. Clean-link goodput was 1–4% lower, with higher sender CPU
and regressions under ECN and reordering. These results favor BBRv3 for
Aster's default settings; enlarged receive windows exposed a separate
stream-buffer failure affecting both controllers.

Measurements were taken on 22 October 2026. Production defaults remain unchanged.

## Patches

1. [Reset probe-feedback state — 8113467](https://forge.emrul.dev/Aster/noq/commit/8113467277d02665688c2ddf41f98dafa51bee58)
   ([download patch](https://forge.emrul.dev/Aster/noq/commit/8113467277d02665688c2ddf41f98dafa51bee58.patch)).
   Clear `bw_probe_samples` and reset `ack_phase` to `Init` at the end of the
   feedback round, including application-limited samples and transitions
   outside ProbeBW. Bandwidth-filter advancement remains conditional.
2. [Grow the probe inflight cap in packet-sized increments — 1e0c42c](https://forge.emrul.dev/Aster/noq/commit/1e0c42cbe096c78f70df6c42743bdf9bfedda943)
   ([download patch](https://forge.emrul.dev/Aster/noq/commit/1e0c42cbe096c78f70df6c42743bdf9bfedda943.patch)).
   Keep `probe_up_cnt = max(cwnd / (1 << rounds), SMSS)` in bytes and grow
   `inflight_longterm` by `delta * SMSS`. The old byte-sized increments differed
   under integer rounding and the per-ACK cwnd gate.

Apply both patches. The reset alone exposed a failure in
`probe_up_rediscovers_full_bw_after_10x_increase`; the growth fix passes its
original target and round limit. Both patches include regression tests.
All [460 protocol tests pass](protocol-validation.log), including the 22
existing Appendix-A scenarios. The [reference review](reference-review.md)
covers the BBR draft, Google's implementations and remaining differences in Noq.

## Method

The workload combined encrypted bulk transfer with a 1 KiB request/echo every
50 ms and a five-second RPC deadline. Both endpoints used the selected
controller with identical transport settings. Goodput was measured at the receiver.

We ran 160 simulator trials (10 per controller/profile) and 204 native
connection attempts (3 per controller/profile/host). Native hosts were linbox,
OVH London and Hetzner k2; the physical LAN test used a Mac receiver.
Linux binaries were built with Rust 1.96.0. Hosts retained their normal services.

Simulator runs measured 24 s after 3 s warm-up. Controlled Linux runs measured
12 s after 2 s warm-up, with 24 s for recovery. Physical-path runs measured
10 s after 2 s warm-up. Linux namespaces separated netem delay/loss from
HTB/bfifo queuing. Controlled tests fixed MTU at 1200; physical tests retained
MTU discovery and GSO unless specified. BBR's probe RNG remained entropy-seeded.
Netem loss was seeded on OVH and Hetzner; linbox used unseeded loss.

Of the native attempts, 189 completed and 15 failed with the stream-buffer
error below. Completed simulator and native runs logged 125,280 successful
RPC probes and zero deadline failures; failed-connection probe totals are unavailable.

## Results

Native medians of three trials. Latency entries are medians of per-trial p95s.

| Profile | CUBIC goodput | BBRv3 goodput | CUBIC RPC p95 | BBRv3 RPC p95 |
|---|---:|---:|---:|---:|
| Clean 20 Mbps, 20 ms RTT, OVH | 18.63 Mbps | 18.28 Mbps | 117 ms | 50 ms |
| 1% random loss, 80 ms RTT, OVH | 1.93 Mbps | 17.07 Mbps | 132 ms | 142 ms |
| Deep queue, 40 ms RTT, OVH | 19.01 Mbps | 18.25 Mbps | 537 ms | 83 ms |
| Physical LAN, default flow control | 78.88 Mbps | 76.15 Mbps | 153 ms | 26 ms |
| Hetzner → OVH, GSO enabled | 929.81 Mbps | 915.26 Mbps | 25.2 ms | 24.9 ms |
| Hetzner → OVH, GSO disabled | 929.70 Mbps | 905.05 Mbps | 25.0 ms | 24.6 ms |
| OVH → Hetzner, default flow control | 402.16 Mbps | 394.88 Mbps | 25.3 ms | 25.2 ms |
| 5 ms jitter, 1% correlated reordering, OVH | 11.96 Mbps | 9.33 Mbps | 156 ms | 170 ms |
| fq_codel with ECN, OVH | 17.65 Mbps | 17.14 Mbps | 91 ms | 148 ms |

During 20 → 5 → 20 Mbps capacity changes, native BBRv3 goodput medians across
the three hosts were 12.33–13.10 Mbps versus CUBIC's 9.02–12.69 Mbps.
RPC p95 was higher: approximately 181 ms versus 159–174 ms. Goodput averages
cover the full reduced-capacity interval; they do not measure recovery time.

On Hetzner→OVH, median sender CPU time was 5.21 s with BBRv3 versus 4.00 s
with CUBIC over the trial process lifetime. With GSO disabled it was 5.59 s
versus 5.36 s. Per-cell median peak RSS stayed below 8 MiB at both endpoints.

The fq_codel tests recorded 104–114 CE marks per BBRv3 trial versus 21 for
CUBIC, with no qdisc drops. ECN response and the reordering deficit need
further investigation.

## Stream-buffer failure

With 32 MiB stream receive, 64 MiB connection receive and 64 MiB send windows,
some transfers failed with `too many gaps in stream buffer`:

| Profile | CUBIC failures | BBRv3 failures |
|---|---:|---:|
| Physical LAN | 3/3 | 0/3 |
| OVH → linbox | 3/3 | 0/3 |
| OVH → Hetzner | 3/3 | 3/3 |
| Controlled 100 Mbps, 80 ms RTT | 3/3 | 0/3 |

The [reproducer](linux/diagnostics/stream-gap-reproducer.rs) leaves one
1200-byte packet missing, then inserts 2,049 contiguous full buffers.
Compaction retains well-utilized buffers separately, so the chunk-count guard
rejects a single received span behind one hole. The [diagnostic log](linux/diagnostics/stream-gap-reproducer.log)
reproduces this independently of either controller.

All six LAN and six reverse-WAN repeats passed with Noq's default flow control
(1,250,000-byte stream receive window). Aster retains that default unless
overridden. Enlarged-window transfers need a separate assembler fix.
Failed connections are counted above and excluded from goodput medians.

See the [full tables](qualification-tables.md), [per-cell CSV](native-summary.csv),
[raw Linux results](linux/) and [reproduction instructions](../README.md).
