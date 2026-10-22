# Why the reset-only candidate failed bandwidth recovery

The failing test exposed an additional Noq implementation mismatch: the
PROBE_UP inflight-limit growth formula did not preserve the packet-sized
increments used by BBRv3. Restoring the ACK-phase/probe-sample resets alone
exposed that mismatch; changing the growth accounting to match the reference
resolves the unchanged recovery test.

## References and units

Google's Linux TCP BBRv3 reference is
[`tcp_bbr.c` at `90210de4b779d40496dee0b89081780eeddf2a60`](https://github.com/google/bbr/blob/90210de4b779d40496dee0b89081780eeddf2a60/net/ipv4/tcp_bbr.c#L1047),
the `v3` branch head checked on 2026-10-22. See
`bbr_raise_inflight_hi_slope` and `bbr_probe_inflight_hi_upward`.
TCP's congestion window, ACK budget and inflight limit in these functions
are packet counts, so one earned increment adds one packet.

Noq uses byte counters. The transport-neutral
[BBRv3 draft, section 5.3.3.6](https://www.ietf.org/archive/id/draft-ietf-ccwg-bbr-06.html#section-5.3.3.6),
and [Google's QUIC implementation](https://github.com/google/quiche/blob/main/quiche/quic/core/congestion_control/bbr2_probe_bw.cc#L307)
provide the byte-accounted form: calculate an ACK budget in bytes, accumulate
ACKed bytes, and add `delta * SMSS` bytes to the inflight limit. The budget is
floored at one SMSS, preserving a maximum growth rate of one packet per
packet ACKed.

Noq instead multiplied the exponential growth factor by SMSS, divided the
window by that value, and added `delta` directly as bytes. Although the
average growth can look algebraically similar, the integer rounding and
per-ACK increments differ. These differences feed back into the cwnd-limited
and inflight-limit conditions used to continue or stop probing.

For a 12,500-byte window and 1,200-byte SMSS at the first growth slope, the
old code raised the inflight limit by 120 bytes on the first ACK. The
corrected form waits until 12,500 bytes have been ACKed, then raises the
limit by a whole 1,200-byte packet and carries the unused ACK budget forward.

The correction keeps the ACK budget in bytes:

```text
acked_bytes_per_increment = max(cwnd / (1 << probe_rounds), SMSS)
delta = accumulated_acked_bytes / acked_bytes_per_increment
accumulated_acked_bytes -= delta * acked_bytes_per_increment
inflight_longterm += delta * SMSS
```

Both ACK-phase/probe-sample resets remain in place. No recovery-test
threshold or success condition was weakened. The existing failure
diagnostic was improved in the preceding experiment, but the test still
requires rediscovery of at least 97% of a 100 Mbps link.

## Controlled validation

| Implementation | Evidence |
| --- | --- |
| Original pinned Noq `535bd884` | Original protocol suite: 456 passed. Existing 10x recovery test passed. |
| Resets only, patch `8113467` | 458 passed, 1 failed. The 10x recovery test stalled below its required throughput. |
| Resets plus packet-growth accounting | All 460 protocol tests pass, including the unchanged 10x recovery test and one new packet-growth regression test. |

The new test,
`probe_up_cap_grows_in_packet_steps_and_preserves_acked_remainder`, uses a
non-packet-aligned window and verifies both the timing/size of inflight-limit
growth and preservation of the ACK remainder. Substituting only the old
growth formula makes it fail (`12620` bytes versus the expected `12500`
after the first ACK); restoring the correction passes. The final full suite
was rerun after this negative witness and again passed all 460 tests. Strict
Clippy passes for `noq-proto --lib` and the benchmark; formatting checks pass
for the changed BBR module and the standalone harness. See
[packet-growth-before.log](packet-growth-before.log).

## Targeted network-model rerun

The same encrypted-Noq emulated-link harness was rerun for the bandwidth
change case only: five trials, three seconds warm-up and 24 seconds measured,
20→5→20 Mbps at link ages 8 and 16 seconds, 80 ms RTT, 100 ms queue, 0.1%
random loss, unlimited upload plus a 1 KiB request/echo every 50 ms.

| BBRv3 variant | Median application Mbps | Median per-trial request p95, ms |
| --- | ---: | ---: |
| Original | 10.83 | 160 |
| Resets only | 9.67 | 183 |
| Resets plus packet growth | 11.49 | 184 |

The new five throughput results range from 7.50 to 12.09 Mbps; all 2,400
request probes completed. BBR's internal probe RNG remains randomized. The
median improvement supports the unit-test diagnosis but is not a claim
that every bandwidth change is fixed or that all performance scenarios
improve. The other seven cases and wall-clock checks were not rerun for
this follow-up. These remain in-memory virtual-time network-model results,
not Linux UDP/GSO or Internet measurements.

## Reproduce and status

From this worktree root:

```sh
rtk cargo test --offline -p noq-proto --lib
rtk proxy cargo run --locked --release --manifest-path bench/cc-compare/Cargo.toml -- \
  --scenario recovery --controller bbr3 --trials 5 --warmup 3 --seconds 24 \
  > bench/cc-compare/results/recovery-packet-growth.jsonl
```

This is a separate follow-up on `investigate/bbr3-recovery` under
`noq-worktrees/bbr-recovery`. The original benchmark branch and its results
remain unchanged. No production pin or congestion-controller default was
changed. Resolving this test is not a full BBRv3 conformance audit or a reason
by itself to switch defaults; browser, native-network and broader
performance validation still apply.
