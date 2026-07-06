#!/usr/bin/env node
// Bulk version of explain-weight.mjs's insight into the "傻屄 vs 傻逼" case:
// finds keykey-boneyard-bootstrap phrases that still win their qstring slot
// in the final normalized tsv, but whose real-world corpus support
// (rime-essay / libchewing frequency) is far weaker than a same-qstring
// sibling that lost to them. These are legacy seed-dictionary weights that
// never got re-validated against a modern corpus.
//
// Usage:
//   node scripts/audit-boneyard-legacy-weights.mjs [--top 50] [--min-ratio 3]
//
// This is a heuristic screen, not a verdict — every hit still needs a human
// read before touching chiaki-modern-overlay. See scripts/explain-weight.mjs
// for a per-phrase deep dive on any candidate this surfaces.

import fs from "node:fs";
import readline from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const NORMALIZED_PATH =
  process.env.NORMALIZED_PATH ?? path.join(ROOT, "normalized/smart-mandarin.tsv");
const BONEYARD_DB_PATH = path.join(
  ROOT,
  "sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db",
);
const ESSAY_PATH = path.join(ROOT, "sources/rime-essay/raw/essay.txt");
const TSI_PATH = path.join(ROOT, "sources/libchewing-data/raw/dict/chewing/tsi.csv");

const BONEYARD_SOURCE_ID = "keykey-boneyard-bootstrap";

function parseArgs(argv) {
  const opts = { top: 50, minRatio: 3 };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--top") opts.top = Number(argv[++i]);
    if (argv[i] === "--min-ratio") opts.minRatio = Number(argv[++i]);
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

async function loadFreqMap(filePath, delimiter, phraseCol, freqCol) {
  const map = new Map();
  await forEachLine(filePath, (line) => {
    const cols = line.split(delimiter);
    const phrase = cols[phraseCol];
    const freq = Number(cols[freqCol]);
    if (!phrase || Number.isNaN(freq)) return;
    const existing = map.get(phrase);
    if (existing === undefined || freq > existing) map.set(phrase, freq);
  });
  return map;
}

function loadBoneyardPhrases() {
  const db = new DatabaseSync(BONEYARD_DB_PATH, { readOnly: true });
  try {
    const rows = db.prepare("select qstring, current, probability from unigrams").all();
    return rows.filter((r) => r.current && r.current.length >= 2);
  } finally {
    db.close();
  }
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));

  console.error("loading rime-essay / tsi.csv frequency tables...");
  const [essayFreq, tsiFreq] = await Promise.all([
    loadFreqMap(ESSAY_PATH, "\t", 0, 1),
    loadFreqMap(TSI_PATH, ",", 0, 1),
  ]);

  console.error("loading boneyard base dictionary...");
  const boneyardPhrases = loadBoneyardPhrases();
  const boneyardByQstring = new Map();
  for (const row of boneyardPhrases) {
    if (!boneyardByQstring.has(row.qstring)) boneyardByQstring.set(row.qstring, []);
    boneyardByQstring.get(row.qstring).push(row);
  }

  console.error("scanning normalized/smart-mandarin.tsv for qstring groups...");
  const wantedQstrings = new Set(boneyardByQstring.keys());
  const groups = new Map();
  await forEachLine(NORMALIZED_PATH, (line) => {
    const [qstring, phrase, weight, sourceId] = line.split("\t");
    if (!wantedQstrings.has(qstring)) return;
    if (phrase.length < 2) return;
    if (!groups.has(qstring)) groups.set(qstring, []);
    groups.get(qstring).push({ phrase, weight: Number(weight), sourceId });
  });

  const corpusFreq = (phrase) => Math.max(essayFreq.get(phrase) ?? 0, tsiFreq.get(phrase) ?? 0);

  const candidates = [];
  for (const [qstring, group] of groups) {
    const winner = group.reduce((a, b) => (a.weight >= b.weight ? a : b));
    if (winner.sourceId !== BONEYARD_SOURCE_ID) continue;
    if (group.length < 2) continue;

    const winnerFreq = corpusFreq(winner.phrase);
    for (const sibling of group) {
      if (sibling.phrase === winner.phrase) continue;
      if (sibling.weight >= winner.weight) continue; // sibling already wins, not interesting
      const siblingFreq = corpusFreq(sibling.phrase);
      const ratio = siblingFreq / Math.max(winnerFreq, 1);
      if (siblingFreq === 0) continue; // no corpus signal either way, nothing to compare
      if (ratio < opts.minRatio) continue;
      candidates.push({
        qstring,
        winner: winner.phrase,
        winnerWeight: winner.weight,
        winnerFreq,
        loser: sibling.phrase,
        loserWeight: sibling.weight,
        loserFreq: siblingFreq,
        ratio,
      });
    }
  }

  candidates.sort((a, b) => b.ratio - a.ratio);
  const top = candidates.slice(0, opts.top);

  console.error(
    `\n${candidates.length} candidate(s) found (boneyard word outranks a same-qstring sibling with >= ${opts.minRatio}x its corpus frequency); showing top ${top.length}\n`,
  );
  console.log("qstring\twinner(boneyard)\twinner_weight\twinner_freq\tloser\tloser_weight\tloser_freq\tratio");
  for (const c of top) {
    console.log(
      `${c.qstring}\t${c.winner}\t${c.winnerWeight}\t${c.winnerFreq}\t${c.loser}\t${c.loserWeight}\t${c.loserFreq}\t${c.ratio.toFixed(1)}`,
    );
  }
}

main();
