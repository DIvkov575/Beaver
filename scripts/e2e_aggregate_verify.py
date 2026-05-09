"""
Compares observed Dataflow detection events against expected.yaml. Reads
detection rule names (one per line) on stdin, plus paths to expected.yaml
and an output report file. Exit 0 if every expected rule fired the predicted
number of times AND no unexpected rules fired.
"""
from __future__ import annotations

import collections
import sys
from pathlib import Path

import yaml


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit("usage: e2e_aggregate_verify.py <expected.yaml>")
    expected_path = Path(sys.argv[1])
    expected = yaml.safe_load(expected_path.read_text())

    # Build expected counts per rule.
    expected_counts: collections.Counter = collections.Counter()
    for entry in expected.values():
        for rule in entry.get("matches", []) or []:
            expected_counts[rule] += 1

    # Read observed rule names from stdin.
    observed_counts: collections.Counter = collections.Counter()
    for line in sys.stdin:
        rule = line.strip()
        if rule:
            observed_counts[rule] += 1

    all_rules = set(expected_counts) | set(observed_counts)
    pad = max(len(r) for r in all_rules) if all_rules else 0
    print(f"  {'rule'.ljust(pad)}  expected  observed")
    print(f"  {'-' * pad}  --------  --------")
    failures: list[str] = []
    for rule in sorted(all_rules):
        e = expected_counts.get(rule, 0)
        o = observed_counts.get(rule, 0)
        marker = " " if e == o else "X"
        print(f"  {rule.ljust(pad)}  {e:>8}  {o:>8}  {marker}")
        if e != o:
            failures.append(f"{rule}: expected {e}, got {o}")

    print()
    if failures:
        print(f"FAIL: {len(failures)} rule(s) had unexpected counts:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"PASS: all {len(expected_counts)} expected rules fired the right number of times")
    return 0


if __name__ == "__main__":
    sys.exit(main())
