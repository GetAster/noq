# Patched BBRv3 default evaluation

**Recommendation:** the evidence supports choosing patched BBRv3 over CUBIC
for Aster's normal transport settings. Take this candidate to Aster for the
default decision. Retain the measured tradeoffs below, and fix the independent
stream-assembler limit before treating enlarged receive-window configurations
as qualified. No production default, consumer pin or deployment was changed.

Evaluation date: 2026-10-22. The candidate contains the probe-feedback reset
and packet-sized probe-growth corrections, relative to Aster Noq pin
`535bd88409193ae577f445a3207351bb83525f4e`. Controller changes are in
`8113467` and `1e0c42c`; the evaluated controller source is unchanged from
`5a20cb4`. Benchmark additions do not change library code or defaults.

## Evidence collected

- **460/460 protocol tests passed**, including all 22 existing Appendix-A
  scenarios and the unchanged tenfold bandwidth-recovery test. The two fixes
  also have focused regression tests. See [reference review](reference-review.md)
  and [test log](protocol-validation.log).
- **160 virtual-time runs:** eight scenarios, two controllers, ten repetitions
  each; three-second warm-up and 24-second measurement. These exercise real
  encrypted Noq endpoints through the calibrated in-memory network model.
- **144 core native Linux runs:** eight scenarios, two controllers and three
  repetitions on each of linbox, OVH London and Hetzner k2. All completed.
- **18 supplementary Linux attempts:** ECN/AQM, jitter/reordering and a
  100 Mbps/80 ms high-BDP case, three repetitions per controller on OVH.
- **42 physical-path attempts:** LAN, OVH to linbox, both OVH–Hetzner
  directions, and Hetzner to OVH with GSO disabled. Includes six LAN and six
  reverse-WAN repeats using Noq's normal flow-control windows.

That is **364 scheduled comparison attempts: 160 virtual and 204 native**.
There were **189 completed native runs and 15 failed native connections**.
Completed runs recorded **125,280 successful RPC probes and zero probe deadline
failures**, including virtual runs. Probe totals from failed connections are
not available; those connections remain explicitly counted as failures.
Early harness setup/diagnostic attempts are retained as diagnostic evidence
where useful and are separate from the scheduled comparison matrix.

The full [tables](qualification-tables.md), [native CSV](native-summary.csv),
[virtual paired comparisons](virtual-paired-comparison.json), and raw
[Linux evidence](linux/) retain repetitions, observed ranges, connection
failures, resource logs and cleanup checks. Latency table entries are medians
of per-trial percentiles, not pooled request percentiles.

## What the comparisons show

Representative native results, medians of three trials:

| Profile | CUBIC goodput | Patched BBRv3 goodput | CUBIC RPC p95 | BBRv3 RPC p95 |
|---|---:|---:|---:|---:|
| Clean 20 Mbps, 20 ms RTT, OVH | 18.63 Mbps | 18.28 Mbps | 117 ms | 50 ms |
| 1% random loss, 80 ms RTT, OVH | 1.93 Mbps | 17.07 Mbps | 132 ms | 142 ms |
| Deep queue, 40 ms RTT, OVH | 19.01 Mbps | 18.25 Mbps | 537 ms | 83 ms |
| Physical LAN, normal flow control | 78.88 Mbps | 76.15 Mbps | 153 ms | 26 ms |
| Hetzner → OVH, GSO enabled | 929.81 Mbps | 915.26 Mbps | 25.2 ms | 24.9 ms |
| Hetzner → OVH, GSO disabled | 929.70 Mbps | 903.44 Mbps | 25.0 ms | 24.7 ms |
| OVH → Hetzner, normal flow control | 402.16 Mbps | 394.88 Mbps | 25.3 ms | 25.2 ms |

The three Linux hosts broadly agree. BBRv3 gives up a few percent of clean-link
goodput, sustains much more traffic under independent random loss, and greatly
reduces deep-queue latency. The corrected virtual shallow-queue result is
17.51 Mbps versus CUBIC's 17.42 Mbps; native shallow-queue cells vary by host,
with a modest OVH deficit rather than the earlier roughly 40% deficit.

Recovery is functional and has no connection failures in the core matrix.
Native BBRv3 recovery medians are 12.33–13.10 Mbps versus CUBIC's
9.02–12.69 Mbps across the three hosts. RPC p95 is higher with BBRv3 during
this mixed-rate scenario, approximately 181 ms versus 159–174 ms. The virtual
recovery comparison is more favorable to BBRv3 in goodput, but also has higher
RPC latency. Recovery goodput is an average across the entire scheduled
20 → 5 → 20 Mbps period; it is not a measurement of instantaneous recovery time.

The supplementary cases expose genuine costs:

- **Jitter/reordering:** with 5 ms normally distributed jitter and 1% correlated
  reordering, BBRv3 reached 9.33 Mbps versus CUBIC's 11.96 Mbps; RPC p95 was
  170 ms versus 156 ms. This is one controlled profile with three repetitions,
  not evidence that every variable-delay path has that deficit.
- **fq_codel with ECN:** BBRv3 reached 17.14 Mbps versus 17.65 Mbps, with
  p95 148 ms versus 91 ms. Router counters recorded 104–114 CE marks per BBRv3
  trial versus 21 for CUBIC, and no qdisc packet drops. Thus the scenario did
  exercise actual CE marking. Exact controller-phase ECN behavior remains a
  reference-review follow-up.
- **CPU:** on the clean approximately 1 Gbps Hetzner→OVH path, median sender
  CPU time was 5.21 s for BBRv3 versus 4.00 s for CUBIC over the trial process
  lifetime, about 30% more CPU time for slightly less data. With GSO disabled,
  it was 5.57 s versus 5.36 s. Normal-window reverse-WAN sender CPU was 2.65 s
  versus 2.22 s. No comparable memory jump appeared: recorded per-cell median
  sender/receiver peak RSS stayed below 8 MiB.

These costs should accompany adoption. The main reasons to choose this
candidate are its loss tolerance and queue-latency behavior, while retaining
near-CUBIC clean-path throughput.

## Independent receive-buffer defect

The enlarged-window stress runs found a shared Noq failure:
`too many gaps in stream buffer`.

| Enlarged-window case | CUBIC failures | BBRv3 failures |
|---|---:|---:|
| Physical LAN | 3/3 | 0/3 |
| OVH → linbox | 3/3 | 0/3 |
| OVH → Hetzner | 3/3 | 3/3 |
| Controlled 100 Mbps, 80 ms RTT | 3/3 | 0/3 |

BBRv3's completed medians were 78.54 Mbps on the LAN, 128.89 Mbps OVH→linbox,
and 91.79 Mbps in the controlled high-BDP case. There is no valid CUBIC
completed-run throughput median for those cases; failures must not be
treated as successful zero-throughput measurements or silently excluded.

A small [standalone reproducer](linux/diagnostics/stream-gap-reproducer.rs)
isolates the mechanism: one missing 1200-byte packet followed by 2,049
contiguous full buffers triggers `TooManyChunks`. See its
[expected failing diagnostic log](linux/diagnostics/stream-gap-reproducer.log).
The assembler's compaction retains already well-utilized buffers separately,
so its chunk-count guard can reject a single contiguous received span behind
one hole. This is independent of congestion-controller choice and was not
introduced by either BBR patch. The temporary diagnostic test was removed
from library source, and the restored 460-test suite passed afterward.

Normal-window repeats succeeded for both controllers on all three LAN and
reverse-WAN trials. Noq's default stream receive window is 1,250,000 bytes;
the stress harness used 32 MiB per stream, 64 MiB connection receive and
64 MiB send windows. Aster's current core configuration leaves these overrides
unset by default, and Iroh's default builder retains Noq's flow-control
windows. Users explicitly enlarging those windows need the separate assembler
fix; switching congestion controllers does not remove that requirement.

## Method and practical limits

The native harness uses encrypted UDP, receiver-measured unlimited bulk
downloads and independently scheduled 1 KiB request/echo streams every 50 ms,
with a five-second probe deadline. Bulk continues while final probes drain.
Both endpoints select the same controller. The virtual harness sends bulk in
the opposite direction; method and receiver accounting remain explicit.

Each controlled Linux run has separate client, router and server namespaces.
Endpoint netem supplies propagation/loss; router HTB plus bfifo supplies rate
and bounded queuing, independent of propagation delay. Core runs use fixed
1200-byte MTU and 12-second measurement after two-second warm-up; recovery
uses 24 seconds with capacity changes at client-process ages 8 and 16 seconds.
The ECN case replaces bfifo with fq_codel. Packet-loss seeds are recorded on
OVH and Hetzner; linbox's older iproute2 requires unseeded netem. BBR's private
probe RNG remains entropy-seeded in all cases. Core qdisc counters can be
cumulative across trials; supplementary qdiscs are recreated per trial.

Physical runs use ordinary IP routing, native sockets, default MTU discovery
and GSO unless explicitly disabled. SSH transfers certificates/results and
controls processes; it does not carry the measured QUIC traffic. The LAN
case uses the Mac as receiver; the other physical-path endpoints are Linux.
Physical trials measure ten seconds after two seconds of warm-up.

The hosts retained their normal services, with no dedicated CPU isolation.
Results are useful repeated comparisons on these deployments, not precision
microbenchmarks or an exhaustive Internet/interoperability qualification.
They test Noq directly; they do not satisfy the separate Salvo Quinn-versus-Noq
H3/WebTransport adoption comparison. No additional canary or mixed-controller
fairness program was introduced as a prerequisite for this evaluation.

Linux binaries were built once on OVH with Rust 1.96.0 and copied to all
three Linux hosts. Later harness versions add one-connection server exit,
path statistics and a normal-flow-control option; they retain identical Noq
controller source. Binary and source fingerprints are in [Linux evidence](linux/).

## Cleanup and handoff

All three remote scratch trees were removed. Final checks found no task
processes, task namespaces, UDP benchmark listeners or temporary Hetzner
firewall rules. No host service, host-interface qdisc or host sysctl was
changed. Per-host `final-cleanup.json` files retain the verification.

The review package is local on `investigate/bbr3-recovery` under
`bench/cc-compare`. Aster can evaluate the two-fix candidate and default choice
from this evidence. Carry the assembler reproducer as a separate correctness
fix, and retain the measured CPU, ECN/AQM and reordering tradeoffs in that
decision. The reset-only candidate is superseded and should not be adopted.
