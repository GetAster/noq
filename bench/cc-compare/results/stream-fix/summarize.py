#!/usr/bin/env python3
"""Summarize the stream-compaction replay separately from the pre-fix comparison."""
import collections
import csv
import json
import pathlib
import statistics

root = pathlib.Path(__file__).resolve().parent
groups = collections.defaultdict(list)
for path in sorted(root.rglob('native.jsonl')):
    for line in path.read_text().splitlines():
        row = json.loads(line)
        groups[(path.parent.name, row['label'], row['controller'])].append(row)

with (root / 'summary.csv').open('w') as output:
    writer = csv.writer(output, lineterminator='\n')
    writer.writerow(['phase', 'scenario', 'controller', 'trials', 'median_mbps',
                     'median_p95_ms', 'successful_probes', 'failed_probes'])
    for key, rows in sorted(groups.items()):
        writer.writerow([*key, len(rows), statistics.median(r['bulk_mbps'] for r in rows),
                         statistics.median(r['p95_ms'] for r in rows),
                         sum(r['probes'] for r in rows), sum(r['failed_probes'] for r in rows)])

rows = [row for group in groups.values() for row in group]
failures = sum(len(path.read_text().splitlines())
               for path in root.rglob('failed-connections.jsonl'))
counts = {'completed_runs': len(rows), 'failed_connections': failures,
          'successful_probes': sum(r['probes'] for r in rows),
          'failed_probes': sum(r['failed_probes'] for r in rows)}
(root / 'counts.json').write_text(json.dumps(counts, indent=2) + '\n')
print(json.dumps(counts))
