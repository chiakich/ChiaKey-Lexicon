#!/usr/bin/env python3
"""Demote secondary polyphone readings that inherited the whole character's frequency.

Mirror of experiment-vanguard-precise.py: every non-target unigram is a fixed
anchor and only the selected target moves, this time downward. A target is
placed just below the lowest-weighted anchor that the Vanguard core ranks above
it; anchors keep their weights and relative order. Two Rime supplemental words
that let 繫 win sentences are capped with the existing rare-word-cap formula.

Outputs explicit.tsv rows plus review tables; sources/ is not edited here.
"""
import argparse
import csv
import importlib.util
import json
import sqlite3
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("reference", ROOT / "scripts/audit/audit-vanguard-single-char.py")
REF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REF)

TEXTMAP = "Build/Release/vanguard-textmap/VanguardFactoryDict4Typing.txtMap"
TSI = ROOT / "sources/libchewing-data/raw/dict/chewing/tsi.csv"
DEMOTE_TAGS = "chiaki-modern-overlay,polyphone-reading-weight,vanguard-fixed-anchor,reading-demote"
CAP_TAGS = "chiaki-modern-overlay,generated,rare-word-cap,reading-demote"
RARE_WORD_CAP_MARGIN = 0.30
PHRASE_LENGTH_BONUS = 1.0
TAIL_STEP = 0.05

# group, qstring, character, reading. 衝 and 波 are listed so the review table
# records why the reference refuses them; --variant forces them below FORCED.
TARGETS = [
    (1, "=A", "挾", "ㄒㄧㄚˊ"),
    (2, ":j", "稱", "ㄔㄣˋ"),
    (3, "<C", "甚", "ㄕㄜˊ"),
    (3, "<C", "什", "ㄕㄜˊ"),
    (3, "<C", "折", "ㄕㄜˊ"),
    (5, "xl", "衝", "ㄔㄨㄥˋ"),
    (6, "D2", "波", "ㄆㄛ"),
]
FORCED = {("xl", "衝"): ["銃"], ("D2", "波"): ["坡", "潑"]}
CAPS = [(4, "n?A_", "直繫"), (4, "CRA_", "所繫")]


def demotion_placements(rows, reference, target_chars, forced=None):
    """Place each target below the anchors ranked above it; anchors never move."""
    forced = forced or {}
    ordered = sorted(rows, key=lambda r: (-r[3], r[0]))
    anchors = [r for r in ordered if r[2] not in target_chars
               and (r[2] in reference or any(r[2] in v for v in forced.values()))]
    groups, skipped = defaultdict(list), {}
    for row in (r for r in ordered if r[2] in target_chars):
        if row[2] in forced:
            superior = [a for a in anchors if a[2] in forced[row[2]]]
        elif row[2] in reference:
            superior = [a for a in anchors if a[2] in reference and reference[a[2]] < reference[row[2]]]
        else:
            skipped[row[2]] = "no-reference-rank"
            continue
        if not superior:
            skipped[row[2]] = "reference-does-not-demote"
            continue
        floor = superior[-1]
        if row[3] < floor[3]:
            skipped[row[2]] = "already-below-superior-anchors"
            continue
        groups[floor[0]].append(row)
    result = {}
    for floor_id, group in groups.items():
        floor = next(a for a in anchors if a[0] == floor_id)
        below = [a for a in anchors if a[3] < floor[3]]
        lower = below[0] if below else None
        group.sort(key=lambda r: (reference.get(r[2], 0), r[0]))
        for index, row in enumerate(group, 1):
            if lower is None:
                weight = floor[3] - TAIL_STEP * index
            else:
                weight = floor[3] + (lower[3] - floor[3]) * index / (len(group) + 1)
            result[row[0]] = (round(weight, 6), floor[2], lower[2] if lower else "",
                              len([a for a in anchors if a[3] >= floor[3]]))
    return result, skipped


def rare_word_cap(groups, qstring, phrase):
    """cap = baseline − margin − bonus×(n−1); baseline = max(char split, same-length rival)."""
    syllables = [qstring[i:i + 2] for i in range(0, len(qstring), 2)]
    split = 0.0
    for syllable in syllables:
        singles = [r[3] for r in groups.get(syllable, []) if len(r[2]) == 1]
        if not singles:
            return None, "no-single-character-path"
        split += max(singles)
    length = len(phrase)
    rivals = [r[3] + PHRASE_LENGTH_BONUS * (length - 1) for r in groups.get(qstring, [])
              if len(r[2]) == length and r[2] != phrase]
    baseline = max([split] + rivals)
    cap = round(baseline - RARE_WORD_CAP_MARGIN - PHRASE_LENGTH_BONUS * (length - 1), 6)
    return cap, dict(split_path=round(split, 6), best_rival=round(max(rivals), 6) if rivals else "")


def load_reference(upstream):
    raw = REF.load_textmap(upstream / TEXTMAP)
    merged = defaultdict(list)
    for reading, q in REF.encode_readings(raw).items():
        merged[q].extend(raw[reading])
    return {q: {ch: rank for rank, (ch, _, _) in enumerate(REF.ordered_unique(rows, {6}), 1)}
            for q, rows in merged.items()}


def load_tsi_characters():
    scores, readings = {}, set()
    with TSI.open(encoding="utf-8") as stream:
        for line in stream:
            if line.startswith("#"):
                continue
            cells = line.rstrip("\n").split(",")
            if len(cells) == 3 and len(cells[0]) == 1:
                readings.add(cells[2])
                scores[cells[0], cells[2]] = int(cells[1])
    encoded = REF.encode_readings(readings)
    return {(ch, encoded[b]): s for (ch, b), s in scores.items() if b in encoded}


def inherited_readings(groups, reference, tsi):
    """Every reading whose weight equals the character's best weight, with evidence."""
    by_char = defaultdict(list)
    for q, rows in groups.items():
        if len(q) != 2 or q.startswith("_"):
            continue
        for row in rows:
            if len(row[2]) == 1:
                by_char[row[2]].append((q, row[3]))
    heads = {q: max((r for r in rows if len(r[2]) == 1), key=lambda r: (r[3], -r[0]))[2]
             for q, rows in groups.items() if any(len(r[2]) == 1 for r in rows)}
    tsi_best = defaultdict(int)
    for (ch, _), score in tsi.items():
        tsi_best[ch] = max(tsi_best[ch], score)
    output = []
    for ch, readings in by_char.items():
        if len(readings) < 2:
            continue
        best = max(w for _, w in readings)
        tied = [q for q, w in readings if w == best]
        if len(tied) < 2:
            continue
        for q in tied:
            ranks = reference.get(q, {})
            score = tsi.get((ch, q))
            ordered = sorted((r for r in groups[q] if len(r[2]) == 1), key=lambda r: (-r[3], r[0]))
            rank = next(i for i, r in enumerate(ordered, 1) if r[2] == ch)
            superior_below = sum(1 for r in ordered[rank:] if r[2] in ranks and ch in ranks
                                 and ranks[r[2]] < ranks[ch])
            if ch not in ranks:
                status = "no-reference"
            elif superior_below:
                status = "reference-would-demote"
            else:
                status = "reference-agrees"
            output.append(dict(character=ch, qstring=q, weight=best, tied_readings=len(tied),
                               readings=len(readings), tsi_score="" if score is None else score,
                               tsi_best=tsi_best.get(ch, ""), tsi_dominant=int(score == tsi_best.get(ch)) if score else 0,
                               chiakey_rank=rank, chiakey_head=heads.get(q, ""),
                               vanguard_rank=ranks.get(ch, ""), superior_anchors_below=superior_below, status=status))
    output.sort(key=lambda r: (r["status"], -r["superior_anchors_below"], r["character"], r["qstring"]))
    return output


def write_tsv(path, rows, fields):
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fields, delimiter="\t", extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)


def build(baseline, upstream, out, variant):
    reference = load_reference(upstream)
    with sqlite3.connect(baseline.resolve().as_uri() + "?mode=ro", uri=True) as conn:
        rows = conn.execute("SELECT rowid,qstring,current,probability FROM unigrams").fetchall()
    groups = defaultdict(list)
    for row in rows:
        groups[row[1]].append(row)
    out.mkdir(parents=True, exist_ok=True)
    explicit, review = [], []
    for q in sorted({q for _, q, _, _ in TARGETS}):
        chars = {ch for _, tq, ch, _ in TARGETS if tq == q}
        forced = {ch: FORCED[q, ch] for ch in chars if (q, ch) in FORCED} if variant else {}
        planned, skipped = demotion_placements(groups[q], reference.get(q, {}), chars, forced)
        single = sorted((r for r in groups[q] if len(r[2]) == 1), key=lambda r: (-r[3], r[0]))
        for group, tq, ch, reading in TARGETS:
            if tq != q:
                continue
            row = next(r for r in single if r[2] == ch)
            old_rank = single.index(row) + 1
            plan = planned.get(row[0])
            entry = dict(group=group, qstring=q, character=ch, reading=reading, old_weight=row[3],
                         old_rank=old_rank, vanguard_rank=reference.get(q, {}).get(ch, ""),
                         new_weight=plan[0] if plan else "", floor_anchor=plan[1] if plan else "",
                         lower_anchor=plan[2] if plan else "", anchors_above=plan[3] if plan else "",
                         reason=skipped.get(ch, "forced-below:" + "/".join(forced[ch]) if ch in forced
                                            else "vanguard-superior-anchors"))
            if plan:
                after = sorted(single, key=lambda r: (-(plan[0] if r[0] == row[0] else
                                                      planned.get(r[0], (r[3],))[0]), r[0]))
                entry["new_rank"] = after.index(row) + 1
                explicit.append(dict(qstring=q, phrase=ch, weight=f"{plan[0]:.6f}", tags=DEMOTE_TAGS))
            review.append(entry)
    caps = []
    for group, q, phrase in CAPS:
        row = next((r for r in groups[q] if r[2] == phrase), None)
        if row is None:
            caps.append(dict(group=group, qstring=q, phrase=phrase, reason="absent"))
            continue
        cap, detail = rare_word_cap(groups, q, phrase)
        entry = dict(group=group, qstring=q, phrase=phrase, old_weight=row[3], cap=cap)
        if cap is None:
            entry["reason"] = detail
        elif row[3] <= cap:
            entry["reason"] = "already-capped"
        else:
            entry.update(detail, reason="rare-word-cap")
            explicit.append(dict(qstring=q, phrase=phrase, weight=f"{cap:.6f}", tags=CAP_TAGS))
        caps.append(entry)
    analysis = inherited_readings(groups, reference, load_tsi_characters())
    existing = {}
    with (ROOT / "sources/chiaki-modern-overlay/explicit.tsv").open(encoding="utf-8") as stream:
        for line in stream:
            cells = line.rstrip("\n").split("\t")
            if len(cells) == 4 and not line.startswith("#"):
                existing[cells[0], cells[1]] = cells[2]
    # dedupe_records keeps the higher weight, so an existing row must be edited in place.
    for row in explicit:
        row["existing_explicit_weight"] = existing.get((row["qstring"], row["phrase"]), "")
    write_tsv(out / "explicit-rows.tsv", explicit, ["qstring", "phrase", "weight", "tags", "existing_explicit_weight"])
    write_tsv(out / "review.tsv", review, ["group", "qstring", "character", "reading", "old_weight", "old_rank",
                                          "vanguard_rank", "new_weight", "new_rank", "floor_anchor",
                                          "lower_anchor", "anchors_above", "reason"])
    write_tsv(out / "caps.tsv", caps, ["group", "qstring", "phrase", "old_weight", "cap", "split_path",
                                      "best_rival", "reason"])
    write_tsv(out / "inherited-readings.tsv", analysis, list(analysis[0]) if analysis else ["character"])
    summary = dict(variant=variant, demoted=sum(1 for r in review if r.get("new_weight") != ""),
                   refused={r["character"]: r["reason"] for r in review if r.get("new_weight") == ""},
                   capped=sum(1 for c in caps if c.get("reason") == "rare-word-cap"),
                   inherited_readings=len(analysis),
                   inherited_secondary=sum(1 for r in analysis) - len({r["character"] for r in analysis}),
                   edit_in_place=[r["qstring"] + " " + r["phrase"] for r in explicit if r["existing_explicit_weight"]],
                   inherited_by_status={s: sum(1 for r in analysis if r["status"] == s)
                                        for s in sorted({r["status"] for r in analysis})},
                   inherited_characters=len({r["character"] for r in analysis}))
    (out / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, default=ROOT / "dist/dev/ChiaKeySource-dev.db")
    parser.add_argument("--upstream", type=Path, default=ROOT / "tmp/vanguard-lexicon-current")
    parser.add_argument("--out", type=Path, default=ROOT / "dist/reading-demotion")
    parser.add_argument("--variant", action="store_true",
                        help="also force 衝/波 below their gold-set rivals (reference does not support this)")
    args = parser.parse_args()
    build(args.baseline, args.upstream, args.out, args.variant)
