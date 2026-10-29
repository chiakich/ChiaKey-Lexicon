#!/usr/bin/env python3
"""Compare two walker runs on the same gold set, per target reading group.

For each tracked reading: how often the walker produced the demoted character
where the gold set wanted another character, and how the expected characters
fared, before and after. Also lists every single-character reading whose head
candidate differs between the two databases.
"""
import argparse
import json
import re
import sqlite3
from collections import Counter, defaultdict
from pathlib import Path

GROUPS = {
    "=A": ("ㄒㄧㄚˊ", ["挾"]),
    ":j": ("ㄔㄣˋ", ["稱"]),
    "<C": ("ㄕㄜˊ", ["甚", "什", "折"]),
    "A_": ("ㄐㄧˋ", ["繫"]),
    "xl": ("ㄔㄨㄥˋ", ["衝"]),
    "D2": ("ㄆㄛ", ["波"]),
}


def read_gold(path):
    gold = {}
    with path.open(encoding="utf-8") as stream:
        for line in stream:
            if line.startswith("#") or not line.strip():
                continue
            readings, expected = line.rstrip("\n").split("\t")[:2]
            gold[expected] = readings.split()
    return gold


def read_mismatches(path):
    rows = {}
    with path.open(encoding="utf-8") as stream:
        for line in stream:
            if line.startswith("#") or not line.strip():
                continue
            expected, got = line.rstrip("\n").split("\t")[:2]
            rows[expected] = got
    return rows


def tally(gold, mismatches):
    wrong_as, expected_wrong, expected_total, got_instead = Counter(), Counter(), Counter(), defaultdict(Counter)
    for expected, qs in gold.items():
        got = mismatches.get(expected, expected)
        if len(got) != len(expected):
            continue
        for e, g, q in zip(expected, got, qs):
            if q not in GROUPS:
                continue
            expected_total[q, e] += 1
            if e != g:
                expected_wrong[q, e] += 1
                got_instead[q, e][g] += 1
                if g in GROUPS[q][1]:
                    wrong_as[q, g] += 1
    return wrong_as, expected_wrong, expected_total, got_instead


def heads(db):
    result = {}
    with sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True) as conn:
        for q, ch in conn.execute(
                "SELECT qstring,current FROM unigrams WHERE length(qstring)=2 AND length(current)=1 "
                "AND qstring NOT LIKE '\\_%' ESCAPE '\\' ORDER BY qstring, probability DESC, rowid"):
            result.setdefault(q, ch)
    return result


def summary(log):
    text = log.read_text()
    exact = int(re.search(r"exact match:\s+(\d+)", text)[1])
    chars, total = map(int, re.search(r"character accuracy:\s+(\d+)/(\d+)", text).groups())
    count = int(re.search(r"sentences:\s+(\d+)", text)[1])
    return dict(sentences=count, exact=exact, exact_pct=round(100 * exact / count, 4),
                chars=chars, total=total, char_pct=round(100 * chars / total, 4))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gold", type=Path, required=True)
    parser.add_argument("--before", type=Path, required=True, help="folder with eval.log and mismatches.tsv")
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--before-db", type=Path)
    parser.add_argument("--after-db", type=Path)
    args = parser.parse_args()
    gold = read_gold(args.gold)
    before = tally(gold, read_mismatches(args.before / "mismatches.tsv"))
    after = tally(gold, read_mismatches(args.after / "mismatches.tsv"))
    print(json.dumps(dict(before=summary(args.before / "eval.log"), after=summary(args.after / "eval.log")),
                     ensure_ascii=False))
    print("\nreading\tdemoted char chosen wrongly: before -> after")
    for q, (name, chars) in GROUPS.items():
        for ch in chars:
            print(f"{name}\t{ch}\t{before[0][q, ch]} -> {after[0][q, ch]}")
    print("\nreading\texpected char\ttotal\twrong before -> after\twrong outputs after")
    for q, (name, _) in GROUPS.items():
        keys = sorted({k for k in before[2] if k[0] == q}, key=lambda k: -before[2][k])
        for key in keys:
            got = ", ".join(f"{g}×{n}" for g, n in after[3][key].most_common(4))
            print(f"{name}\t{key[1]}\t{before[2][key]}\t{before[1][key]} -> {after[1][key]}\t{got}")
    if args.before_db and args.after_db:
        old, new = heads(args.before_db), heads(args.after_db)
        changed = [(q, old[q], new.get(q)) for q in old if old[q] != new.get(q)]
        print(f"\nhead changes among {len(old)} single-character readings: {len(changed)}")
        for q, a, b in changed:
            print(f"  {q}\t{a} -> {b}")
    fixed = [e for e in gold if e in read_mismatches(args.before / "mismatches.tsv")
             and e not in read_mismatches(args.after / "mismatches.tsv")]
    broken = [e for e in gold if e not in read_mismatches(args.before / "mismatches.tsv")
              and e in read_mismatches(args.after / "mismatches.tsv")]
    print(f"\nsentences fixed: {len(fixed)}, newly wrong: {len(broken)}")
    after_rows = read_mismatches(args.after / "mismatches.tsv")
    for e in broken:
        print(f"  broke: {e} -> {after_rows[e]}")
    for e in fixed:
        print(f"  fixed: {e}")


if __name__ == "__main__":
    main()
