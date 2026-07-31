#!/usr/bin/env node
// Finds same-qstring phrase groups where rime-essay overlap-rerank made a
// candidate win, but another candidate in the same reading has stronger
// libchewing support or is close enough to deserve a Taiwan-usage review.
//
// Usage:
//   node scripts/audit/audit-rime-rerank-variants.mjs [--top 100] [--max-gap 0.35] [--min-tsi-ratio 1]
//
// This is a heuristic screen, not an automatic fix. For each hit, inspect both
// phrases with:
//   node scripts/audit/explain-weight.mjs <winner> <challenger>

import fs from "node:fs";
import readline from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const NORMALIZED_PATH =
  process.env.NORMALIZED_PATH ?? path.join(ROOT, "normalized/smart-mandarin.tsv");
const ESSAY_PATH = path.join(ROOT, "sources/rime-essay/raw/essay.txt");
const TSI_PATH = path.join(ROOT, "sources/libchewing-data/raw/dict/chewing/tsi.csv");

const RIME_SOURCE_ID = "rime-essay";
const LIBCHEWING_SOURCE_ID = "libchewing-data";
const OVERLAY_SOURCE_IDS = new Set([
  "chiaki-modern-overlay",
  "chiaki-web-overlay",
  "chiaki-auto-hotwords-overlay",
]);

function parseArgs(argv) {
  const opts = {
    top: 100,
    maxGap: 0.35,
    minTsiRatio: 1,
    minSharedPositions: 1,
    includeOverlayWinners: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--top") opts.top = Number(argv[++i]);
    else if (arg === "--max-gap") opts.maxGap = Number(argv[++i]);
    else if (arg === "--min-tsi-ratio") opts.minTsiRatio = Number(argv[++i]);
    else if (arg === "--min-shared-positions") opts.minSharedPositions = Number(argv[++i]);
    else if (arg === "--include-overlay-winners") opts.includeOverlayWinners = true;
    else if (arg === "--help" || arg === "-h") {
      console.log(`Usage:
  node scripts/audit/audit-rime-rerank-variants.mjs [--top 100] [--max-gap 0.35] [--min-tsi-ratio 1]

Options:
  --top N                    Number of rows to print.
  --max-gap N                Only report challengers within this weight gap.
  --min-tsi-ratio N          Challenger libchewing freq must be >= N * winner libchewing freq.
  --min-shared-positions N   Require this many same-position shared characters.
  --include-overlay-winners  Include groups already topped by overlay corrections.
`);
      process.exit(0);
    }
  }
  if (!Number.isFinite(opts.top) || opts.top <= 0) throw new Error("invalid --top");
  if (!Number.isFinite(opts.maxGap) || opts.maxGap < 0) throw new Error("invalid --max-gap");
  if (!Number.isFinite(opts.minTsiRatio) || opts.minTsiRatio < 0) {
    throw new Error("invalid --min-tsi-ratio");
  }
  if (!Number.isFinite(opts.minSharedPositions) || opts.minSharedPositions < 0) {
    throw new Error("invalid --min-shared-positions");
  }
  return opts;
}

async function forEachLine(filePath, onLine) {
  const stream = fs.createReadStream(filePath, { encoding: "utf8" });
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  for await (const line of rl) {
    if (!line || line.startsWith("#")) continue;
    onLine(line);
  }
}

async function loadRimeFreq() {
  const map = new Map();
  await forEachLine(ESSAY_PATH, (line) => {
    const [phrase, freqText] = line.split("\t");
    const freq = Number(freqText);
    if (!phrase || Number.isNaN(freq)) return;
    const previous = map.get(phrase);
    if (previous === undefined || freq > previous) map.set(phrase, freq);
  });
  return map;
}

async function loadTsiFreq() {
  const byPhrase = new Map();
  await forEachLine(TSI_PATH, (line) => {
    const [phrase, freqText, bopomofo] = line.split(",");
    const freq = Number(freqText);
    if (!phrase || !bopomofo || Number.isNaN(freq)) return;
    const previous = byPhrase.get(phrase);
    if (previous === undefined || freq > previous) byPhrase.set(phrase, freq);
  });
  return { byPhrase };
}

async function loadNormalizedGroups() {
  const groups = new Map();
  await forEachLine(NORMALIZED_PATH, (line) => {
    const [qstring, phrase, weightText, sourceId, tags] = line.split("\t");
    const weight = Number(weightText);
    if (!qstring || !phrase || Number.isNaN(weight)) return;
    if (phrase.length < 2) return;
    const entry = { qstring, phrase, weight, sourceId, tags: tags ?? "" };
    if (!groups.has(qstring)) groups.set(qstring, []);
    groups.get(qstring).push(entry);
  });
  for (const group of groups.values()) {
    group.sort((a, b) => b.weight - a.weight || a.phrase.localeCompare(b.phrase, "zh-Hant"));
  }
  return groups;
}

function safeRatio(numerator, denominator) {
  if (numerator === 0) return 0;
  return numerator / Math.max(denominator, 1);
}

function sharedPositionCount(left, right) {
  const leftChars = Array.from(left);
  const rightChars = Array.from(right);
  const length = Math.min(leftChars.length, rightChars.length);
  let count = 0;
  for (let i = 0; i < length; i++) {
    if (leftChars[i] === rightChars[i]) count++;
  }
  return count;
}

function rowScore(hit) {
  const tsiSignal = hit.challengerTsi > hit.winnerTsi ? 2 : 0;
  const closeGapSignal = Math.max(0, 1 - hit.weightGap / Math.max(hit.maxGap, 0.000001));
  const sourceSignal = hit.challengerSource === LIBCHEWING_SOURCE_ID ? 1 : 0;
  return tsiSignal + closeGapSignal + sourceSignal + Math.log10(hit.challengerTsi + 1) / 10;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!fs.existsSync(NORMALIZED_PATH)) {
    console.error(`normalized tsv not found at ${NORMALIZED_PATH}`);
    console.error("Run `cargo run --release -- prepare-release` first, or set NORMALIZED_PATH.");
    process.exit(1);
  }

  console.error("loading rime-essay and libchewing frequency tables...");
  const [rimeFreq, tsiFreq, groups] = await Promise.all([
    loadRimeFreq(),
    loadTsiFreq(),
    loadNormalizedGroups(),
  ]);

  const hits = [];
  for (const [qstring, group] of groups) {
    if (group.length < 2) continue;
    const winner = group[0];
    const overlayWinner = OVERLAY_SOURCE_IDS.has(winner.sourceId);
    if (overlayWinner && !opts.includeOverlayWinners) continue;
    if (
      winner.sourceId !== RIME_SOURCE_ID ||
      !winner.tags.includes("overlap-rerank")
    ) {
      continue;
    }

    const winnerRime = rimeFreq.get(winner.phrase) ?? 0;
    const winnerTsi = tsiFreq.byPhrase.get(winner.phrase) ?? 0;
    for (const challenger of group.slice(1)) {
      const weightGap = winner.weight - challenger.weight;
      if (weightGap < 0 || weightGap > opts.maxGap) continue;
      const sharedPositions = sharedPositionCount(winner.phrase, challenger.phrase);
      if (sharedPositions < opts.minSharedPositions) continue;

      const challengerRime = rimeFreq.get(challenger.phrase) ?? 0;
      const challengerTsi = tsiFreq.byPhrase.get(challenger.phrase) ?? 0;
      const tsiRatio = safeRatio(challengerTsi, winnerTsi);
      const rimeRatio = safeRatio(winnerRime, challengerRime);
      const libchewingSupportsChallenger =
        challengerTsi > 0 && tsiRatio >= opts.minTsiRatio && challengerTsi >= winnerTsi;
      const closeLibchewingPhrase =
        challenger.sourceId === LIBCHEWING_SOURCE_ID && weightGap <= opts.maxGap / 2;
      if (!libchewingSupportsChallenger && !closeLibchewingPhrase) continue;

      const reason = [
        libchewingSupportsChallenger ? "libchewing-supports-challenger" : null,
        closeLibchewingPhrase ? "close-libchewing-phrase" : null,
        rimeRatio >= 2 ? "rime-prefers-winner" : null,
      ]
        .filter(Boolean)
        .join(",");

      const hit = {
        qstring,
        winner: winner.phrase,
        winnerWeight: winner.weight,
        winnerRime,
        winnerTsi,
        challenger: challenger.phrase,
        challengerWeight: challenger.weight,
        challengerSource: challenger.sourceId,
        challengerTags: challenger.tags,
        challengerRime,
        challengerTsi,
        weightGap,
        tsiRatio,
        rimeRatio,
        sharedPositions,
        reason,
        maxGap: opts.maxGap,
      };
      hit.score = rowScore(hit);
      hits.push(hit);
    }
  }

  hits.sort(
    (a, b) =>
      b.score - a.score ||
      a.weightGap - b.weightGap ||
      b.challengerTsi - a.challengerTsi ||
      a.winner.localeCompare(b.winner, "zh-Hant"),
  );
  const top = hits.slice(0, opts.top);

  console.error(
    `\n${hits.length} candidate(s) found; showing top ${top.length}. ` +
      "Review before adding overlays.\n",
  );
  console.log(
    [
      "qstring",
      "rime_winner",
      "winner_weight",
      "winner_rime",
      "winner_tsi",
      "challenger",
      "challenger_weight",
      "challenger_source",
      "challenger_rime",
      "challenger_tsi",
      "weight_gap",
      "shared_positions",
      "tsi_ratio",
      "rime_ratio",
      "reason",
    ].join("\t"),
  );
  for (const hit of top) {
    console.log(
      [
        hit.qstring,
        hit.winner,
        hit.winnerWeight.toFixed(6),
        hit.winnerRime,
        hit.winnerTsi,
        hit.challenger,
        hit.challengerWeight.toFixed(6),
        hit.challengerSource,
        hit.challengerRime,
        hit.challengerTsi,
        hit.weightGap.toFixed(6),
        hit.sharedPositions,
        hit.tsiRatio.toFixed(2),
        hit.rimeRatio.toFixed(2),
        hit.reason,
      ].join("\t"),
    );
  }
}

main().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
