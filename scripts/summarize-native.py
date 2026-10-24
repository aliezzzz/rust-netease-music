#!/usr/bin/env python3
"""Summarize a completed native smoke/soak run without treating partial output as PASS."""
import argparse
import json
from pathlib import Path
import re
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
args = parser.parse_args()
root = args.directory
result = root / 'result.txt'
if not result.exists() or result.read_text().strip() != 'PASS':
    raise SystemExit('Native run has no PASS result; do not report it as completed.')
log = (root / 'app.log').read_text()
samples = json.loads((root / 'memory.json').read_text())
times = [int(value) for value in re.findall(r'Window \d+ IPC restored in (\d+) ms', log)]
summary = {
    'result': 'PASS',
    'sampled_seconds': samples[-1]['seconds'],
    'restore_ms': {'min': min(times), 'median': statistics.median(times), 'max': max(times)},
    'last_process_group_sample': samples[-1],
}
soak = re.search(r'SOAK_OK: (\d+) seconds, (\d+) loops without WebViews', log)
if soak:
    summary['soak_seconds'] = int(soak[1])
    summary['soak_loops'] = int(soak[2])
    # Exclude window warmup and the final pause/shutdown samples.
    steady = [s for s in samples if 120 <= s['seconds'] <= samples[-1]['seconds'] - 10]
    if len(steady) >= 2:
        pss = [s['pss_kb'] for s in steady]
        first, last = steady[0], steady[-1]
        summary['background'] = {
            'samples': len(steady),
            'process_counts': sorted({s['processes'] for s in steady}),
            'pss_kib': {'first': pss[0], 'last': pss[-1], 'min': min(pss), 'median': statistics.median(pss), 'max': max(pss)},
        }
        # Sum of live process CPU times is only comparable while children persist.
        if len(summary['background']['process_counts']) == 1 and all(b['cpu_seconds'] >= a['cpu_seconds'] for a, b in zip(steady, steady[1:])):
            summary['background']['mean_cpu_percent_one_core'] = round(100 * (last['cpu_seconds'] - first['cpu_seconds']) / (last['seconds'] - first['seconds']), 3)
print(json.dumps(summary, indent=2, ensure_ascii=False))
