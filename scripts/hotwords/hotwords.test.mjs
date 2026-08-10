import assert from "node:assert/strict";
import test from "node:test";

import { filterShiftedPttFragments, shouldEmitSignal } from "./hotwords.mjs";

test("filters shifted four-character PTT fragments from the same signal", () => {
  const shared = {
    source: "ptt-gossiping",
    observed_on: "2026-08-08",
    window_hours: 24,
    traffic: 398,
  };
  const observations = [
    { ...shared, term: "垃圾民進" },
    { ...shared, term: "圾民進黨" },
    { ...shared, term: "食藥署" },
  ];

  assert.deepEqual(filterShiftedPttFragments(observations).map(({ term }) => term), ["食藥署"]);
});

test("keeps overlapping terms when their PTT signals differ", () => {
  const observations = [
    { term: "垃圾民進", source: "ptt-gossiping", observed_on: "2026-08-08", traffic: 398 },
    { term: "圾民進黨", source: "ptt-gossiping", observed_on: "2026-08-08", traffic: 258 },
  ];

  assert.equal(filterShiftedPttFragments(observations).length, 2);
});

test("admits a popular PTT-only term observed on two days", () => {
  const signal = {
    hasShortWindow14: true,
    windows14: ["24h"],
    score14: 2,
    seenDays14: 2,
  };

  assert.equal(shouldEmitSignal(signal, { sources: new Set(["ptt-gossiping"]), max_traffic: 60 }), true);
  assert.equal(shouldEmitSignal(signal, { sources: new Set(["ptt-gossiping"]), max_traffic: 59 }), false);
  assert.equal(shouldEmitSignal(signal, { sources: new Set(["google-trends"]), max_traffic: 400 }), false);
});
