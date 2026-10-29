#!/usr/bin/env python3
"""Compare final Vanguard CHT TextMap ranks with ChiaKey, without changing weights.

Reports both full-list ranks and ranks restricted to the SAME candidate pool.
Core CHT (type 6) and CHT+CNS+GBEX (types 6,7,11) are kept separate.
"""
import argparse
import csv
import hashlib
import json
import re
import sqlite3
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def encode_readings(keys):
    """Read Rust's component table; apply its bit encoding without running a CLI."""
    source = (ROOT / "src/phonetics.rs").read_text(encoding="utf-8")
    components = {text: int(value, 16) for text, value in re.findall(
        r'text:\s*"([^"]+)"\s*,\s*value:\s*(0x[0-9a-fA-F]+)', source)}
    if not components:
        raise ValueError("could not read phonetic component table")
    encoded = {}
    for key in keys:
        if len(re.split(r"[\s,]+", key.strip())) != 1:
            continue
        found = [components[ch] for ch in key if ch in components]
        if not found:
            continue
        syllable = 0
        for value in found:
            syllable |= value
        order = ((syllable & 0x001f) + ((syllable & 0x0060) >> 5) * 22
                 + ((syllable & 0x0780) >> 7) * 22 * 4
                 + ((syllable & 0x3800) >> 11) * 22 * 4 * 14)
        encoded[key] = chr(48 + order % 79) + chr(48 + order // 79)
    return encoded


def read_revision(upstream):
    """Read the cloned checkout's Git metadata without running an executable."""
    gitdir = upstream / ".git"
    if gitdir.is_file():
        text = gitdir.read_text(encoding="utf-8").strip()
        if not text.startswith("gitdir: "):
            raise ValueError("unrecognized gitdir file")
        gitdir = (upstream / text[8:]).resolve()
    head = (gitdir / "HEAD").read_text(encoding="utf-8").strip()
    if head.startswith("ref: "):
        ref = head[5:]
        # Linked worktrees keep branch refs in the common dir.
        common = gitdir
        if (gitdir / "commondir").exists():
            common = (gitdir / (gitdir / "commondir").read_text(encoding="utf-8").strip()).resolve()
        path = common / ref
        if path.exists():
            head = path.read_text(encoding="utf-8").strip()
        else:
            packed = (common / "packed-refs").read_text(encoding="utf-8").splitlines()
            head = next(line.split()[0] for line in packed if line.endswith(" " + ref))
    if not re.fullmatch(r"[0-9a-f]{40}", head):
        raise ValueError("unrecognized upstream revision")
    return head


def grouped_values(cell):
    if cell == "\x07":
        return []
    result, value = [], []
    escaped = False
    for ch in cell:
        if escaped:
            if ch not in {"s", "a", "\\", "|"}:
                raise ValueError(f"unknown TextMap escape: {ch!r}")
            value.append({"s": " ", "a": "\x07"}.get(ch, ch))
            escaped = False
        elif ch == "\\":
            escaped = True
        elif ch == "|":
            result.append("".join(value))
            value = []
        else:
            value.append(ch)
    if escaped:
        raise ValueError("unterminated TextMap escape")
    result.append("".join(value))
    return result


def load_textmap(path):
    header, values, keys = {}, [], []
    section = None
    with path.open(encoding="utf-8") as stream:
        for raw in stream:
            line = raw.rstrip("\r\n")
            if line.startswith("#PRAGMA:"):
                section = line.rsplit("_", 1)[-1]
                if line.endswith("KEY_LINE_MAP"):
                    section = "KEYS"
                continue
            if section == "HEADER":
                name, value = line.split("\t", 1)
                header[name] = value
            elif section == "VALUES":
                values.append(line)
            elif section == "KEYS":
                key, start, count = line.split("\t")
                keys.append((key, int(start), int(count)))
    if header.get("TYPE") != "TYPING" or header.get("VERSION") != "1.1":
        raise ValueError("expected Vanguard TYPING TextMap version 1.1")
    if len(values) != int(header["ENTRY_COUNT"]) or len(keys) != int(header["KEY_COUNT"]):
        raise ValueError("TextMap header counts do not match sections")
    result = defaultdict(list)
    for key, start, count in keys:
        if key.startswith("_") or header["READING_SEPARATOR"] in key:
            continue
        if start < 0 or start + count > len(values):
            raise ValueError("TextMap key points outside VALUES section")
        for order, line in enumerate(values[start:start + count], start):
            cells = line.split("\t")
            if line.startswith("@"):
                score = float(cells[0][1:])
                entries = [(6, v, score) for v in grouped_values(cells[2])]
            elif line.startswith(">"):
                kind = int(cells[0][1:])
                score = float(header[f"DEFAULT_PROB_{kind}"])
                entries = [(kind, v, score) for v in grouped_values(cells[1])]
            else:
                entries = [(int(cells[2]), cells[0], float(cells[1]))]
            for position, (kind, ch, score) in enumerate(entries):
                if len(ch) == 1 and kind in {6, 7, 11}:
                    result[key].append((ch, score, kind, order, position))
    return result


def ordered_unique(entries, kinds):
    seen, result = set(), []
    for ch, score, kind, order, position in sorted(entries, key=lambda e: (-e[1], e[3], e[4])):
        if kind in kinds and ch not in seen:
            seen.add(ch)
            result.append((ch, score, kind))
    return result


def compare(candidates, source, db, out, upstream, mcbopomofo=None):
    raw = load_textmap(source)
    keys = sorted(raw)
    encoded = encode_readings(keys)
    vanguard = defaultdict(list)
    q_for_key = {}
    for key, q in encoded.items():
        if len(q) == 2:
            q_for_key[key] = q
            vanguard[q].extend(raw[key])
    vg_core = {q: ordered_unique(entries, {6}) for q, entries in vanguard.items()}
    vg_extended = {q: ordered_unique(entries, {6, 7, 11}) for q, entries in vanguard.items()}
    mcb_ranks = {}
    if mcbopomofo is not None:
        mcb_groups = defaultdict(list)
        with mcbopomofo.open(encoding="utf-8") as stream:
            for line in stream:
                if line.startswith("#"):
                    continue
                cells = line.split()
                if len(cells) == 3 and len(cells[1]) == 1 and cells[0] in q_for_key:
                    q = q_for_key[cells[0]]
                    if cells[1] not in mcb_groups[q]:
                        mcb_groups[q].append(cells[1])
        mcb_ranks = {(q, ch): rank for q, group in mcb_groups.items()
                     for rank, ch in enumerate(group, 1)}
    chia = defaultdict(list)
    with sqlite3.connect(db.resolve().as_uri() + "?mode=ro", uri=True) as conn:
        for q, ch, score in conn.execute(
                "SELECT qstring,current,probability FROM unigrams "
                "WHERE length(current)=1 AND length(qstring)=2 ORDER BY probability DESC,rowid ASC"):
            if ch not in {item[0] for item in chia[q]}:
                chia[q].append((ch, score))
    core_ranks, ext_ranks = {}, {}
    shared_ranks = {}
    for q, group in chia.items():
        core = vg_core.get(q, [])
        extended = vg_extended.get(q, [])
        for mapping, other in ((core_ranks, core), (ext_ranks, extended)):
            for rank, (ch, score, kind) in enumerate(other, 1):
                mapping[q, ch] = (rank, score, kind)
        overlap = {x[0] for x in group} & {x[0] for x in core}
        chia_shared = [x[0] for x in group if x[0] in overlap]
        vg_shared = [x[0] for x in core if x[0] in overlap]
        vg_positions = {ch: rank for rank, ch in enumerate(vg_shared, 1)}
        for rank, ch in enumerate(chia_shared, 1):
            shared_ranks[q, ch] = (rank, vg_positions[ch], len(overlap))
    all_vg_chars = {item[0] for group in vg_extended.values() for item in group}
    with candidates.open(encoding="utf-8") as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    output = []
    for row in rows:
        q, ch = row["qstring"], row["character"]
        core_rank, vg_score, _ = core_ranks.get((q, ch), ("", "", ""))
        extended_rank, _, extended_type = ext_ranks.get((q, ch), ("", "", ""))
        chia_rank = next((rank for rank, item in enumerate(chia[q], 1) if item[0] == ch), "")
        current_score = next((item[1] for item in chia[q] if item[0] == ch), "")
        shared_chia, shared_vg, shared_count = shared_ranks.get((q, ch), ("", "", 0))
        delta = shared_chia - shared_vg if shared_chia != "" else ""
        if core_rank != "":
            status = "core-covered"
        elif extended_rank != "":
            status = "extended-only"
        elif ch in all_vg_chars:
            status = "reading-absent"
        else:
            status = "character-absent"
        core = vg_core.get(q, [])
        tie_ranks = [rank for rank, (_, score, _) in enumerate(core, 1) if score == vg_score]
        output.append({**row, "candidate_rank": chia_rank, "current_weight": current_score,
                       "vanguard_status": status, "vanguard_core_rank": core_rank,
                       "vanguard_extended_rank": extended_rank, "vanguard_extended_type": extended_type,
                       "vanguard_weight": vg_score,
                       "vanguard_core_tie_first": min(tie_ranks) if tie_ranks else "",
                       "vanguard_core_tie_last": max(tie_ranks) if tie_ranks else "",
                       "chiakey_candidate_count": len(chia[q]), "vanguard_core_count": len(core),
                       "chiakey_shared_rank": shared_chia, "vanguard_shared_rank": shared_vg,
                       "shared_count": shared_count, "shared_rank_improvement": delta,
                       "mcbopomofo_export_rank": mcb_ranks.get((q, ch), ""),
                       "chiakey_top10": " ".join(x[0] for x in chia[q][:10]),
                       "vanguard_core_top10": " ".join(x[0] for x in core[:10])})
    output.sort(key=lambda row: (row["evidence_tier"] != "supported",
                                -(row["shared_rank_improvement"] if row["shared_rank_improvement"] != "" else -10000), row["character"], row["qstring"]))
    supported = [r for r in output if r["evidence_tier"] == "supported"
                 and r.get("frequency_gap", "1") == "1"]
    priority = [r for r in output if r["shared_rank_improvement"] != ""
                and r["shared_rank_improvement"] >= 10]
    export_differences = [r for r in output if mcbopomofo is not None
                          and r["vanguard_core_rank"] != ""
                          and r["vanguard_core_rank"] != r["mcbopomofo_export_rank"]]
    remaining_priority = [r for r in priority if r.get("frequency_gap", "1") != "1"]
    out.mkdir(parents=True, exist_ok=True)
    fields = list(output[0])
    for name, data in (("all-comparison.tsv", output), ("supported-comparison.tsv", supported),
                       ("large-rank-gaps.tsv", priority),
                       ("remaining-large-rank-gaps.tsv", remaining_priority),
                       ("remaining-supported-large-rank-gaps.tsv",
                        [r for r in remaining_priority if r["evidence_tier"] == "supported"]),
                       ("export-rank-differences.tsv", export_differences),
                       ("remaining-comparison.tsv", [r for r in output if r.get("frequency_gap", "1") != "1"]),
                       ("large-opposite-rank-gaps.tsv", [r for r in output if r["shared_rank_improvement"] != ""
                                                         and r["shared_rank_improvement"] <= -10])):
        with (out / name).open("w", encoding="utf-8", newline="") as stream:
            writer = csv.DictWriter(stream, fields, delimiter="\t")
            writer.writeheader()
            writer.writerows(data)
    by_character = defaultdict(list)
    for row in output:
        by_character[row["character"]].append(row)
    character_rows = []
    for ch, group in sorted(by_character.items()):
        deltas = [r["shared_rank_improvement"] for r in group if r["shared_rank_improvement"] != ""]
        character_rows.append(dict(
            character=ch, reading_count=len(group),
            core_covered=sum(r["vanguard_status"] == "core-covered" for r in group),
            extended_only=sum(r["vanguard_status"] == "extended-only" for r in group),
            absent_readings=" / ".join(r["reading"] for r in group if r["vanguard_status"] == "reading-absent"),
            max_shared_improvement=max(deltas) if deltas else "",
            max_shared_decline=-min(deltas) if deltas and min(deltas) < 0 else 0,
            readings_with_large_difference=" / ".join(
                f"{r['reading']}:{r['shared_rank_improvement']:+d}" for r in group
                if r["shared_rank_improvement"] != "" and abs(r["shared_rank_improvement"]) >= 10)))
    with (out / "characters-comparison.tsv").open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, list(character_rows[0]), delimiter="\t")
        writer.writeheader()
        writer.writerows(character_rows)
    snapshot = read_revision(upstream)
    stats = dict(upstream_commit=snapshot, source_format="Vanguard TYPING TextMap 1.1",
                 total=len(output), supported=len(supported),
                 characters=len({r["character"] for r in output}),
                 remaining_readings=sum(r.get("frequency_gap", "1") != "1" for r in output),
                 remaining_characters=len({r["character"] for r in output if r.get("frequency_gap", "1") != "1"}),
                 remaining_coverage=dict(Counter(r["vanguard_status"] for r in output
                                                 if r.get("frequency_gap", "1") != "1")),
                 chiakey_absent=sum(r["candidate_rank"] == "" for r in output),
                 all_coverage=dict(Counter(r["vanguard_status"] for r in output)),
                 supported_coverage=dict(Counter(r["vanguard_status"] for r in supported)),
                 shared_rank_improvement_at_least10=len(priority),
                 supported_shared_rank_improvement_at_least10=sum(r in supported for r in priority),
                 remaining_shared_rank_improvement_at_least10=sum(r.get("frequency_gap", "1") != "1" for r in priority),
                 remaining_large_improvement_with_phrase_support=sum(
                     r["evidence_tier"] == "supported" for r in remaining_priority),
                 shared_rank_decline_at_least10=sum(r["shared_rank_improvement"] != ""
                                                    and r["shared_rank_improvement"] <= -10 for r in output),
                 supported_top10_crossings=sum(r["candidate_rank"] != "" and r["candidate_rank"] > 10
                                               and r["vanguard_core_rank"] != "" and r["vanguard_core_rank"] <= 10
                                               for r in supported),
                 sha256={str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                         for p in (source, db, candidates, ROOT / "src/phonetics.rs")})
    if "frequency_gap" in output[0]:
        stats["remaining_shared_rank_decline_at_least10"] = sum(
            r["frequency_gap"] != "1" and r["shared_rank_improvement"] != ""
            and r["shared_rank_improvement"] <= -10 for r in output)
    if mcbopomofo is not None:
        stats["supported_export_crosscheck"] = dict(
            checked=len(supported),
            matched=sum(r["mcbopomofo_export_rank"] == r["vanguard_core_rank"]
                        and r["vanguard_core_rank"] != "" for r in supported))
        stats["all_core_export_crosscheck"] = dict(
            checked=sum(r["vanguard_core_rank"] != "" for r in output),
            differences=len(export_differences))
        stats["sha256"][str(mcbopomofo)] = hashlib.sha256(mcbopomofo.read_bytes()).hexdigest()
    (out / "summary.json").write_text(json.dumps(stats, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(stats, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--upstream", type=Path, default=ROOT / "tmp/vanguard-lexicon-current")
    parser.add_argument("--textmap", type=Path)
    parser.add_argument("--mcbopomofo", type=Path)
    parser.add_argument("--all-readings", action="store_true", help="compare every libchewing polyphone reading")
    parser.add_argument("--candidates", type=Path)
    parser.add_argument("--db", type=Path, default=ROOT / "dist/dev/ChiaKeySource-dev.db")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    source = args.textmap or args.upstream / "Build/Release/vanguard-textmap/VanguardFactoryDict4Typing.txtMap"
    crosscheck = args.mcbopomofo or args.upstream / "Build/Release/mcbopomofo-cht/data.txt"
    candidates = args.candidates or ROOT / "tmp/polyphone-frequency-audit" / (
        "readings.tsv" if args.all_readings else "low-frequency-readings.tsv")
    out = args.out or ROOT / "tmp" / ("vanguard-all-polyphone-audit" if args.all_readings else "vanguard-polyphone-audit")
    compare(candidates, source, args.db, out, args.upstream,
            crosscheck if crosscheck.exists() else None)
