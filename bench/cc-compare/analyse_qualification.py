#!/usr/bin/env python3
"""Rebuild comparison tables from raw qualification JSONL and resource logs."""
import collections
import csv
import json
import pathlib
import random
import re
import statistics as st

ROOT=pathlib.Path(__file__).resolve().parent/'results'


def med(rows,key): return st.median(r[key] for r in rows)


def resources(path):
    if not path.exists(): return {}
    text=path.read_text()
    def value(pattern):
        m=re.search(pattern,text)
        return float(m[1]) if m else None
    user=value(r'User time \(seconds\): ([\d.]+)')
    system=value(r'System time \(seconds\): ([\d.]+)')
    return {'cpu_seconds': None if user is None or system is None else user+system,
            'max_rss_kib':value(r'Maximum resident set size \(kbytes\): (\d+)')}


def main():
    virtual=[json.loads(x) for x in (ROOT/'patched-full-matrix.jsonl').read_text().splitlines()]
    vg=collections.defaultdict(dict)
    for r in virtual: vg[r['scenario']['name']].setdefault(r['controller'],[]).append(r)
    lines=['# Qualification comparison tables','',
           'All latency entries are medians of per-trial percentiles, not percentiles pooled across requests.',
           'Ranges show observed per-trial goodput; native cells generally have three repetitions.',
           '', '## Controlled virtual-time matrix','',
           '| Scenario | CUBIC Mbps | BBRv3 Mbps | Change | CUBIC p95 ms | BBRv3 p95 ms | BBRv3 Mbps range |',
           '|---|---:|---:|---:|---:|---:|---:|']
    comparisons=[]
    rng=random.Random(20261022)
    for name,g in vg.items():
        c,b=g['cubic'],g['bbr3']; cm,bm=med(c,'bulk_mbps'),med(b,'bulk_mbps')
        delta=f'{(bm/cm-1)*100:+.1f}%' if cm else '—'
        lines.append(f'| {name} | {cm:.2f} | {bm:.2f} | {delta} | {med(c,"p95_ms"):.1f} | {med(b,"p95_ms"):.1f} | {min(r["bulk_mbps"] for r in b):.2f}–{max(r["bulk_mbps"] for r in b):.2f} |')
        cp={r['trial']:r for r in c}; bp={r['trial']:r for r in b}
        diffs=[bp[t]['bulk_mbps']-cp[t]['bulk_mbps'] for t in sorted(cp.keys() & bp.keys())]
        boot=sorted(st.median(rng.choices(diffs,k=len(diffs))) for _ in range(10000))
        comparisons.append({'scenario':name,'paired_median_difference_mbps':st.median(diffs),
                            'bootstrap_95pct_interval_mbps':[boot[250],boot[9749]],'trials':len(diffs)})
    (ROOT/'virtual-paired-comparison.json').write_text(json.dumps(comparisons,indent=2)+'\n')
    groups=collections.defaultdict(list)
    all_native=[]
    for path in sorted((ROOT/'linux').rglob('native.jsonl')):
        mode=('supplementary' if path.parent.name.startswith('supplementary')
              else 'paths' if path.parent.name.startswith('paths') else path.parent.name)
        for raw in path.read_text().splitlines():
            r=json.loads(raw)
            host=path.parent.parent.name if mode!='paths' else f'{r["server"]}->{r["client"]}'
            tag=f'{r["label"]}-{r["trial"]}-{r["controller"]}'
            for role in ('client','server'):
                for key,value in resources(path.parent/f'{tag}.{role}.log').items():
                    r[role+'_'+key]=value
            groups[(mode,host,r['label'],r['controller'])].append(r)
            all_native.append(r)
    summaries=[]
    for (mode,host,label,cc),rs in groups.items():
        row={'mode':mode,'host_or_path':host,'scenario':label,'controller':cc,'trials':len(rs),
             'median_mbps':med(rs,'bulk_mbps'),'min_mbps':min(r['bulk_mbps'] for r in rs),
             'max_mbps':max(r['bulk_mbps'] for r in rs),
             'median_p50_ms':med(rs,'p50_ms'),'median_p95_ms':med(rs,'p95_ms'),'median_p99_ms':med(rs,'p99_ms'),
             'failed_probes':sum(r['failed_probes'] for r in rs),'successful_probes':sum(r['probes'] for r in rs)}
        for role in ('client','server'):
            for key in ('cpu_seconds','max_rss_kib'):
                values=[r[role+'_'+key] for r in rs if r.get(role+'_'+key) is not None]
                row['median_'+role+'_'+key]=st.median(values) if values else None
        summaries.append(row)
    if summaries:
        with (ROOT/'native-summary.csv').open('w') as f:
            w=csv.DictWriter(f,fieldnames=list(summaries[0]),lineterminator='\n'); w.writeheader();w.writerows(summaries)
        paired=collections.defaultdict(dict)
        for r in summaries: paired[(r['mode'],r['host_or_path'],r['scenario'])][r['controller']]=r
        for mode in ('controlled','supplementary','paths'):
            lines += ['',f'## Native {mode} comparisons','',
                      '| Host/path | Scenario | CUBIC Mbps | BBRv3 Mbps | CUBIC p95 ms | BBRv3 p95 ms |',
                      '|---|---|---:|---:|---:|---:|']
            for (m,host,label),g in paired.items():
                if m!=mode: continue
                c,b=g.get('cubic'),g.get('bbr3')
                def fmt(row,key,places): return f'{row[key]:.{places}f}' if row else 'no completed runs'
                lines.append(f'| {host} | {label} | {fmt(c,"median_mbps",2)} | {fmt(b,"median_mbps",2)} | {fmt(c,"median_p95_ms",1)} | {fmt(b,"median_p95_ms",1)} |')
    failures=[]
    for path in sorted((ROOT/'linux').rglob('failed-connections.jsonl')):
        failures.extend(json.loads(x) for x in path.read_text().splitlines())
    if failures:
        lines += ['', '## Failed native connections', '',
                  'Failures are retained separately and are not included in completed-run throughput medians.',
                  '', '| Host/path | Scenario | Controller | Failed connections |',
                  '|---|---|---|---:|']
        fg=collections.Counter((r.get('host') or f'{r["server"]}->{r["client"]}',r['label'],r['controller']) for r in failures)
        for (host,label,cc),count in sorted(fg.items()): lines.append(f'| {host} | {label} | {cc} | {count} |')
    totals={'virtual_runs':len(virtual),'native_completed_runs':len(all_native),
            'native_failed_connections':len(failures),'native_attempted_runs':len(all_native)+len(failures),
            'successful_probes':sum(r['probes'] for r in virtual+all_native),
            'failed_probes':sum(r['failed_probes'] for r in virtual+all_native)}
    (ROOT/'qualification-counts.json').write_text(json.dumps(totals,indent=2)+'\n')
    (ROOT/'qualification-tables.md').write_text('\n'.join(lines)+'\n')
    print(json.dumps(totals))


if __name__=='__main__': main()
