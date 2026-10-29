"""Verify language filtering, final TextMap groups and shared-pool comparison."""
import contextlib
import csv
import importlib.util
import io
import sqlite3
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "vanguard_audit", Path(__file__).with_name("audit-vanguard-single-char.py"))
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)

TEXTMAP = """#PRAGMA:VANGUARD_HOMA_LEXICON_HEADER
VERSION\t1.1
TYPE\tTYPING
READING_SEPARATOR\t-
ENTRY_COUNT\t4
KEY_COUNT\t2
DEFAULT_PROB_7\t-11
#PRAGMA:VANGUARD_HOMA_LEXICON_VALUES
@-2\t谢\t謝
@-3\t械\t械|解
>7\t械|妎
液\t-2.5\t6
#PRAGMA:VANGUARD_HOMA_LEXICON_KEY_LINE_MAP
ㄒㄧㄝˋ\t0\t3
ㄧㄝˋ\t3\t1
"""


class VanguardRankTest(unittest.TestCase):
    def test_grouped_cells_and_duplicate_extensions(self):
        self.assertEqual(AUDIT.grouped_values(r"a\|b|c\sd|e\\f"), ["a|b", "c d", "e\\f"])
        self.assertEqual(AUDIT.grouped_values("\x07"), [])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "dict.txtMap"
            path.write_text(TEXTMAP, encoding="utf-8")
            entries = AUDIT.load_textmap(path)["ㄒㄧㄝˋ"]
            self.assertEqual(AUDIT.ordered_unique(entries, {6}),
                             [("謝", -2, 6), ("械", -3, 6), ("解", -3, 6)])
            self.assertEqual(AUDIT.ordered_unique(entries, {6, 7, 11}),
                             [("謝", -2, 6), ("械", -3, 6), ("解", -3, 6), ("妎", -11, 7)])

    def test_shared_pool_does_not_mistake_missing_candidates_for_rank_improvement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "dict.txtMap"
            source.write_text(TEXTMAP, encoding="utf-8")
            db = root / "dict.db"
            with sqlite3.connect(db) as conn:
                conn.execute("CREATE TABLE unigrams(qstring TEXT,current TEXT,probability REAL)")
                conn.executemany("INSERT INTO unigrams VALUES(?,?,?)",
                                 [("gc", "蟹", 0), ("gc", "械", -1), ("gc", "謝", -2),
                                  ("gc", "妎", -3), ("gc", "液", -4)])
            candidates = root / "candidates.tsv"
            candidates.write_text("character\treading\tqstring\tevidence_tier\n"
                                  "械\tㄒㄧㄝˋ\tgc\tsupported\n"
                                  "妎\tㄒㄧㄝˋ\tgc\tno-evidence\n"
                                  "液\tㄒㄧㄝˋ\tgc\tno-evidence\n", encoding="utf-8")
            upstream = root / "upstream"
            (upstream / ".git").mkdir(parents=True)
            (upstream / ".git/HEAD").write_text("0" * 40 + "\n", encoding="utf-8")
            with contextlib.redirect_stdout(io.StringIO()):
                AUDIT.compare(candidates, source, db, root / "out", upstream)
            with (root / "out/all-comparison.tsv").open(encoding="utf-8", newline="") as stream:
                data = {r["character"]: r for r in csv.DictReader(stream, delimiter="\t")}
            self.assertEqual(data["械"]["candidate_rank"], "2")
            self.assertEqual(data["械"]["vanguard_core_rank"], "2")
            self.assertEqual(data["械"]["chiakey_shared_rank"], "1")
            self.assertEqual(data["械"]["vanguard_shared_rank"], "2")
            self.assertEqual(data["械"]["shared_rank_improvement"], "-1")
            self.assertEqual(data["妎"]["vanguard_status"], "extended-only")
            self.assertEqual(data["液"]["vanguard_status"], "reading-absent")

    def test_python_encoding_matches_existing_rust_audit_snapshot(self):
        path = AUDIT.ROOT / "tmp/polyphone-frequency-audit/readings.tsv"
        if not path.exists():
            self.skipTest("run polyphone frequency audit first")
        with path.open(encoding="utf-8", newline="") as stream:
            rows = list(csv.DictReader(stream, delimiter="\t"))
        encoded = AUDIT.encode_readings({row["reading"] for row in rows})
        for row in rows:
            self.assertEqual(encoded[row["reading"]], row["qstring"])


if __name__ == "__main__":
    unittest.main()
