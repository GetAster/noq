#!/usr/bin/env python3
"""Summarize repeated benchmark trials without third-party dependencies."""
import csv
import json
import statistics
from collections import defaultdict
from pathlib import Path

root = Path(__file__).parent
groups = defaultdict(list)
rows = []
for filename in ("baseline.jsonl", "probe-reset.jsonl", "wall-clock.jsonl"):
    path = root / "results" / filename
    if path.exists():
        for line in path.read_text().splitlines():
            row = json.loads(line)
            rows.append(row)
            groups[(row["clock"], row["scenario"]["name"], row["variant"], row["controller"])].append(row)

fields = ["clock", "scenario", "variant", "controller", "trials", "mbps_median",
          "mbps_min", "mbps_max", "p50_ms_median", "p95_ms_median", "p99_ms_median",
          "successful_probes", "failed_probes"]
with (root / "results" / "summary.csv").open("w", newline="") as output:
    writer = csv.DictWriter(output, fieldnames=fields)
    writer.writeheader()
    for key, trials in groups.items():
        speeds = [r["bulk_mbps"] for r in trials]
        writer.writerow(dict(zip(fields[:4], key)) | {
            "trials": len(trials), "mbps_median": statistics.median(speeds),
            "mbps_min": min(speeds), "mbps_max": max(speeds),
            "p50_ms_median": statistics.median(r["p50_ms"] for r in trials),
            "p95_ms_median": statistics.median(r["p95_ms"] for r in trials),
            "p99_ms_median": statistics.median(r["p99_ms"] for r in trials),
            "successful_probes": sum(r["probes"] for r in trials),
            "failed_probes": sum(r["failed_probes"] for r in trials),
        })

def cell(clock, scenario, variant, controller):
    trials = groups.get((clock, scenario, variant, controller), [])
    if not trials:
        return "—"
    speed = statistics.median(r["bulk_mbps"] for r in trials)
    latency = statistics.median(r["p95_ms"] for r in trials)
    return f"{speed:.2f} / {latency:.0f}"

lines = ["# Noq congestion-control benchmark results", "",
         "2026-10-22, Apple Silicon macOS, Rust 1.96.0. Baseline: Noq 1.4.0 at "
         "`535bd88409193ae577f445a3207351bb83525f4e`. Candidate: the same code with "
         "probe-feedback resets and ACKS_INIT initialization.", "",
         "**The reset candidate is not ready for adoption: an existing bandwidth-recovery test fails.**", "",
         "Cells show application goodput in Mbps / small-request p95 latency in ms.", "",
         "| Emulated scenario | CUBIC | Current BBRv3 | BBRv3 with probe resets |",
         "| --- | ---: | ---: | ---: |"]
scenarios = list(dict.fromkeys(r["scenario"]["name"] for r in rows if r["clock"] == "virtual"))
for name in scenarios:
    lines.append(f"| {name} | {cell('virtual', name, 'baseline', 'cubic')} | "
                 f"{cell('virtual', name, 'baseline', 'bbr3')} | {cell('virtual', name, 'probe-reset', 'bbr3')} |")
virtual = [r for r in rows if r["clock"] == "virtual"]
lines += ["", f"{len(virtual)} virtual-time trials; {sum(r['probes'] for r in virtual):,} completed probes; "
          f"{sum(r['failed_probes'] for r in virtual)} failed probes.", "",
          "Five trials per cell, 3 seconds warm-up plus 24 seconds measured. Values are medians "
          "of per-trial results; latency percentiles are not pooled. All bottlenecks are 20 Mbps "
          "per direction, including IPv4/UDP headers. RTT is 20 ms (clean), 40 ms (deep queue), "
          "or 80 ms (other cases). Queues are 100 ms except 500 ms deep / 10 ms shallow. "
          "Recovery changes 20→5→20 Mbps at simulated link ages 8 and 16 seconds. "
          "RPC-only has no bulk transfer and 0.1% random loss; zero bulk goodput is intentional.", "",
          "## Interpretation", "",
          "The resets increase sustained throughput in several scenarios, but increase tail "
          "latency in several of those cases and worsen recovery in this experiment. Current "
          "BBRv3 substantially outperforms CUBIC under random loss and holds shorter deep queues; "
          "CUBIC wins throughput on clean and shallow-buffer paths. Neither this comparison nor "
          "the issue report supports changing the default globally.", "",
          "The reset candidate passes the three new state-transition tests, which fail on "
          "the baseline. It fails the existing `probe_up_rediscovers_full_bw_after_10x_increase` "
          "test, which passes on the baseline. The original suite passes 456 tests; the "
          "candidate passes 458 with 1 failure. See [validation.txt](validation.txt).", "",
          "## Limits", "",
          "Real encrypted Noq endpoints and congestion/pacing/loss-recovery code execute, "
          "but packet I/O is in memory and Tokio time is virtual. These are algorithm-level "
          "network-model results, not measured Internet throughput or CPU/OS/GSO performance. "
          "MTU is fixed at 1200; flow-control windows are enlarged identically; no jitter, "
          "cross-connection competition, ECN or offload is modeled. Small requests use "
          "independent bidirectional streams (1 KiB request and response, every 50 ms), "
          "concurrently with an unlimited upload on a unidirectional stream. A five-second "
          "timeout is counted as a failed probe. Bulk stays active while final probes drain. "
          "Loss uses fixed per-trial/direction seeds; BBR's internal probe RNG remains random. "
          "The clock has roughly 1 ms timer resolution. This does not reproduce the issue's "
          "specific jitter/100 MB transfer or two-priority video-track workload, and it does "
          "not satisfy the Salvo Quinn-versus-Noq H3/WebTransport/Linux performance gate."]
wall = [r for r in rows if r["clock"] == "wall"]
if wall:
    lines += ["", "## Wall-clock sanity checks", "",
              "The same in-memory link runs with real wall-clock timers. These short runs "
              "check timer/model sensitivity; they still do not exercise native UDP/GSO.", "",
              "| Scenario | CUBIC | Current BBRv3 | BBRv3 with probe resets |",
              "| --- | ---: | ---: | ---: |"]
    for name in dict.fromkeys(r["scenario"]["name"] for r in wall):
        lines.append(f"| {name} | {cell('wall', name, 'baseline', 'cubic')} | "
                     f"{cell('wall', name, 'baseline', 'bbr3')} | {cell('wall', name, 'probe-reset', 'bbr3')} |")
    lines += ["", "One short trial per cell: 2 seconds warm-up plus 6 seconds measured; "
              "do not compare these directly to the longer virtual runs as equivalent samples."]
lines += ["", "Sources: [Noq #819](https://github.com/n0-computer/noq/issues/819) and "
          "[BBR draft correction #114](https://github.com/ietf-wg-ccwg/draft-ietf-ccwg-bbr/pull/114).", ""]
(root / "results" / "report.md").write_text("\n".join(lines))
print(root / "results" / "report.md")
