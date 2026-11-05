"""Test downward placement below Vanguard-superior anchors and the rare-word cap."""
import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "demote", Path(__file__).resolve().parents[1] / "lexicon/demote-inherited-readings.py")
DEMOTE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEMOTE)


class DemotionPlacementTest(unittest.TestCase):
    def test_target_moves_below_every_superior_anchor_only(self):
        rows = [(1, "q", "甚", -1.0, 0), (2, "q", "蛇", -1.5, 0), (3, "q", "佘", -2.0, 0),
                (4, "q", "闍", -3.0, 0)]
        planned, skipped = DEMOTE.demotion_placements(rows, {"蛇": 1, "佘": 2, "甚": 3, "闍": 4}, {"甚"})
        self.assertEqual(set(planned), {1})
        weight, floor, lower, _ = planned[1]
        self.assertEqual((floor, lower), ("佘", "闍"))
        self.assertTrue(-3.0 < weight < -2.0)
        self.assertFalse(skipped)

    def test_two_targets_share_a_gap_in_reference_order(self):
        rows = [(1, "q", "甚", -1.0, 0), (2, "q", "什", -1.1, 0), (3, "q", "蛇", -1.5, 0),
                (4, "q", "闍", -3.0, 0)]
        planned, _ = DEMOTE.demotion_placements(rows, {"蛇": 1, "什": 2, "甚": 3, "闍": 4}, {"甚", "什"})
        self.assertGreater(planned[2][0], planned[1][0])
        self.assertTrue(-3.0 < planned[1][0] < planned[2][0] < -1.5)

    def test_reference_head_is_not_demoted(self):
        rows = [(1, "q", "衝", -2.1, 0), (2, "q", "銃", -2.3, 0)]
        planned, skipped = DEMOTE.demotion_placements(rows, {"衝": 1, "銃": 2}, {"衝"})
        self.assertFalse(planned)
        self.assertEqual(skipped["衝"], "reference-does-not-demote")

    def test_forced_anchor_overrides_reference(self):
        rows = [(1, "q", "衝", -2.1, 0), (2, "q", "銃", -2.3, 0)]
        planned, _ = DEMOTE.demotion_placements(rows, {"衝": 1, "銃": 2}, {"衝"}, {"衝": ["銃"]})
        self.assertAlmostEqual(planned[1][0], -2.3 - DEMOTE.TAIL_STEP)

    def test_already_below_is_left_alone(self):
        rows = [(1, "q", "寄", -1.0, 0), (2, "q", "繫", -1.2, 0)]
        planned, skipped = DEMOTE.demotion_placements(rows, {"寄": 1, "繫": 2}, {"繫"})
        self.assertFalse(planned)
        self.assertEqual(skipped["繫"], "already-below-superior-anchors")


class RareWordCapTest(unittest.TestCase):
    def test_cap_sits_below_split_path_by_margin(self):
        groups = {"aa": [(1, "aa", "直", -1.2)], "bb": [(2, "bb", "記", -1.1), (3, "bb", "寄", -1.15)],
                  "aabb": [(4, "aabb", "直繫", -2.2)]}
        cap, detail = DEMOTE.rare_word_cap(groups, "aabb", "直繫")
        self.assertAlmostEqual(detail["split_path"], -2.3)
        self.assertAlmostEqual(cap, -2.3 - 0.30 - 1.0)

    def test_same_length_rival_raises_the_baseline(self):
        groups = {"aa": [(1, "aa", "所", -0.7)], "bb": [(2, "bb", "記", -1.1)],
                  "aabb": [(3, "aabb", "所繫", -1.9), (4, "aabb", "瑣記", -1.95)]}
        cap, detail = DEMOTE.rare_word_cap(groups, "aabb", "所繫")
        self.assertAlmostEqual(detail["best_rival"], -0.95)
        self.assertAlmostEqual(cap, -0.95 - 0.30 - 1.0)


if __name__ == "__main__":
    unittest.main()
