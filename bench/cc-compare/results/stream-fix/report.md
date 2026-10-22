# Stream-buffer compaction fix

The chunk-count guard could reject a legitimate receive stream after one
missing packet. Allocation trimming retained fully utilized adjacent buffers
separately, then counted each against the 1024-span limit.

The fix is on [fix/stream-buffer-compaction](https://github.com/GetAster/noq/tree/fix/stream-buffer-compaction),
based on Aster's integration pin `535bd884`:

- [69a1df075](https://github.com/GetAster/noq/commit/69a1df075d7c13265ae25cf809523b674db9a36d)
  coalesces adjacent buffers before enforcing the span cap. It preserves
  isolated gaps and reuses an owned prefix allocation as the run grows.
- [3ae189bba](https://github.com/GetAster/noq/commit/3ae189bba41024a6daf29a353460f373860c0e26)
  fixes unordered compaction: `bytes_read` counts delivered bytes in that mode,
  so it cannot serve as a lower offset bound. Carry both commits.

The contiguous-packet regression [fails before coalescing](contiguous-before.log).
The unordered regression [fails with coalescing alone](unordered-before.log).
Both pass with the complete fix, alongside byte-exact partial reads,
`read_into`, reordered/retransmitted runs and the existing gap/overlap/duplicate
flood tests. The span cap and batching threshold are unchanged.

## Validation

With both BBR corrections and both compaction commits, the suites pass
[464 protocol tests](protocol-tests.log) and [39 Noq tests](noq-tests.log)
(3 ignored). [Clippy](clippy.log) and the [no-std check](no-std.log) pass.

The enlarged-window replay uses the original workload and windows: 32 MiB per
stream, 64 MiB connection receive and 64 MiB send. Measurements on 22 October
2026 used the same Linux hosts, encrypted UDP and three trials per controller.
All 38 native runs completed with zero connection failures and 7,840 successful
RPC probes, with no probe deadline failures.
Native goodput medians for the previously failing profiles:

| Profile | CUBIC | BBRv3 |
|---|---:|---:|
| Physical LAN | 85.37 Mbps | 77.88 Mbps |
| OVH → linbox | 149.26 Mbps | 142.24 Mbps |
| OVH → Hetzner | 906.92 Mbps | 908.33 Mbps |
| Controlled 100 Mbps, 80 ms RTT | 93.80 Mbps | 92.71 Mbps |

The original [comparison](../qualification-report.md) records 15 connection
failures across these profiles. The replay also includes the clean
Hetzner→OVH path with GSO enabled and disabled. Raw data and resource logs are
in [paths](paths/) and [high-bdp](high-bdp/); [summary.csv](summary.csv) retains
trial counts and RPC latency.

Those replay binaries contain the coalescing commit and both BBR fixes.
The unordered follow-up was then checked by the full suites and an additional
OVH→Hetzner run under each controller using the final combined source; see
[final-paths](final-paths/). The replay workload uses ordered bulk reads;
unordered byte preservation is covered by the regression tests.

See the [Aster adoption handoff](../adoption-handoff.md) for patch order,
the separate BBR default change, Salvo adoption and pending ledger entries.
