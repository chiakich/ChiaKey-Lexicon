#!/usr/bin/env node
// Answers "why does phrase X have weight Y" without re-running the Rust
// pipeline: greps the final normalized tsv plus every raw upstream source
// for the requested phrase(s) and prints what each one says.
//
// Usage:
//   node scripts/explain-weight.mjs 童音 同音
//
// This is a snapshot of the *raw inputs*, not a re-run of the rerank
// pipeline (importers.rs) — it won't show intermediate rerank math, only
// each source's own frequency/weight and the final winner in the
// normalized tsv. Good enough for "which source won and roughly why";
// for exact rerank-stage-by-stage math, read src/importers.rs directly.

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

// Raw sources consumed by the pipeline (see src/paths.rs / src/config.rs for
// the canonical list). Each entry describes how to find the phrase column.
const RAW_SOURCES = [
  {
    label: "libchewing tsi.csv",
    file: "sources/libchewing-data/raw/dict/chewing/tsi.csv",
    delimiter: ",",
    phraseCol: 0,
    format: (cols) => `freq=${cols[1]} bopomofo=${cols[2]}`,
  },
  {
    label: "libchewing word.csv",
    file: "sources/libchewing-data/raw/dict/chewing/word.csv",
    delimiter: ",",
    phraseCol: 2,
    format: (cols) => `freq=${cols[1]} bopomofo=${cols[0]}`,
  },
  {
    label: "rime-essay essay.txt",
    file: "sources/rime-essay/raw/essay.txt",
    delimiter: "\t",
    phraseCol: 0,
    format: (cols) => `freq=${cols[1]}`,
  },
  {
    label: "chiaki-modern-overlay/phrases.tsv",
    file: "sources/chiaki-modern-overlay/phrases.tsv",
    delimiter: "\t",
    phraseCol: 0,
    format: (cols) => `weight=${cols[1]} tags=${cols[2]}`,
  },
  {
    label: "chiaki-modern-overlay/explicit.tsv",
    file: "sources/chiaki-modern-overlay/explicit.tsv",
    delimiter: "\t",
    phraseCol: 1,
    format: (cols) => `qstring=${cols[0]} weight=${cols[2]} tags=${cols[3]}`,
  },
  {
    label: "chiaki-web-overlay/explicit.tsv",
    file: "sources/chiaki-web-overlay/explicit.tsv",
    delimiter: "\t",
    phraseCol: 1,
    format: (cols) => `qstring=${cols[0]} weight=${cols[2]} tags=${cols[3]}`,
  },
  {
    label: "chiaki-synthetic-overlay/unigrams.tsv",
    file: "sources/chiaki-synthetic-overlay/unigrams.tsv",
    delimiter: "\t",
    phraseCol: 1,
    format: (cols) => `qstring=${cols[0]} weight=${cols[2]} tags=${cols[3]}`,
  },
  {
    label: "chiaki-auto-hotwords-overlay/phrases.tsv",
    file: "sources/chiaki-auto-hotwords-overlay/phrases.tsv",
    delimiter: "\t",
    phraseCol: 0,
    format: (cols) => `weight=${cols[1]} tags=${cols[2]}`,
  },
  {
    label: "chiaki-fragment-denylist/fragment-demotions.tsv",
    file: "sources/chiaki-fragment-denylist/fragment-demotions.tsv",
    delimiter: "\t",
    phraseCol: 0,
    format: (cols) => `max_weight=${cols[1]} tags=${cols[2]}`,
  },
];

async function forEachLine(filePath, onLine) {
  const stream = fs.createReadStream(filePath, { encoding: "utf8" });
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  for await (const line of rl) {
    if (!line || line.startsWith("#")) continue;
    onLine(line);
  }
}

async function scanRawSource(source, words) {
  const filePath = path.join(ROOT, source.file);
  if (!fs.existsSync(filePath)) {
    return words.map(() => []);
  }
  const hits = new Map(words.map((w) => [w, []]));
  await forEachLine(filePath, (line) => {
    const cols = line.split(source.delimiter);
    const phrase = cols[source.phraseCol];
    if (hits.has(phrase)) {
      hits.get(phrase).push(source.format(cols));
    }
  });
  return words.map((w) => hits.get(w));
}

function scanBoneyardDb(words) {
  // The pipeline starts by copying this sqlite DB verbatim (see
  // src/release.rs `fs::copy(&cfg.boneyard_db, ...)`), before any importer
  // runs. Phrases already present here keep whatever weight is baked in
  // unless a later stage explicitly reranks them, and — unlike the tsv/csv
  // sources above — this weight isn't derived from any greppable corpus.
  if (!fs.existsSync(BONEYARD_DB_PATH)) {
    return new Map(words.map((w) => [w, []]));
  }
  const db = new DatabaseSync(BONEYARD_DB_PATH, { readOnly: true });
  try {
    const stmt = db.prepare(
      "select qstring, probability, backoff from unigrams where current = ?",
    );
    return new Map(
      words.map((w) => [
        w,
        stmt.all(w).map((row) => `qstring=${row.qstring} weight=${row.probability}`),
      ]),
    );
  } finally {
    db.close();
  }
}

async function scanNormalized(words) {
  // matches: word -> [{ qstring, phrase, weight, sourceId, tags }]
  const matches = new Map(words.map((w) => [w, []]));
  // bestByQstring: qstring -> strongest unigram candidate, used to explain
  // why a whole phrase can lose to a split path.
  const bestByQstring = new Map();
  // homophoneGroups: qstring -> [{ phrase, weight, sourceId, tags }] for any
  // qstring that at least one target word matched, so we can show the full
  // ranking context (useful for the "why is A above B" question).
  const wantedQstrings = new Set();
  await forEachLine(NORMALIZED_PATH, (line) => {
    const [qstring, phrase, weight, sourceId, tags] = line.split("\t");
    const entry = { qstring, phrase, weight: Number(weight), sourceId, tags };
    const best = bestByQstring.get(qstring);
    if (
      !best ||
      entry.weight > best.weight ||
      (entry.weight === best.weight && entry.phrase.localeCompare(best.phrase, "zh-Hant") < 0)
    ) {
      bestByQstring.set(qstring, entry);
    }
    if (matches.has(phrase)) {
      matches.get(phrase).push(entry);
      wantedQstrings.add(qstring);
    }
  });

  const groups = new Map(Array.from(wantedQstrings, (q) => [q, []]));
  await forEachLine(NORMALIZED_PATH, (line) => {
    const [qstring, phrase, weight, sourceId, tags] = line.split("\t");
    if (groups.has(qstring)) {
      groups.get(qstring).push({ phrase, weight: Number(weight), sourceId, tags });
    }
  });
  for (const group of groups.values()) {
    group.sort((a, b) => b.weight - a.weight);
  }

  return { matches, groups, bestByQstring };
}

function bestSplit(row, bestByQstring) {
  const syllables = row.qstring.length / 2;
  if (!Number.isInteger(syllables) || syllables < 2) return null;

  let best = null;
  for (let splitSyllable = 1; splitSyllable < syllables; splitSyllable += 1) {
    const splitAt = splitSyllable * 2;
    const prefix = row.qstring.slice(0, splitAt);
    const suffix = row.qstring.slice(splitAt);
    const left = bestByQstring.get(prefix);
    const right = bestByQstring.get(suffix);
    if (!left || !right) continue;
    const weight = left.weight + right.weight;
    if (
      !best ||
      weight > best.weight ||
      (weight === best.weight &&
        `${left.phrase}${right.phrase}`.localeCompare(
          `${best.left.phrase}${best.right.phrase}`,
          "zh-Hant",
        ) < 0)
    ) {
      best = { left, right, weight };
    }
  }
  return best;
}

async function main() {
  const words = process.argv
    .slice(2)
    .flatMap((arg) => arg.split(","))
    .map((w) => w.trim())
    .filter(Boolean);

  if (words.length === 0) {
    console.error("Usage: node scripts/explain-weight.mjs <phrase> [phrase2 ...]");
    console.error("       node scripts/explain-weight.mjs 童音,同音");
    process.exit(1);
  }

  if (!fs.existsSync(NORMALIZED_PATH)) {
    console.error(`normalized tsv not found at ${NORMALIZED_PATH}`);
    console.error("Run `cargo run --release -- prepare-release` first, or set NORMALIZED_PATH.");
    process.exit(1);
  }

  const { matches, groups, bestByQstring } = await scanNormalized(words);
  const rawHitsBySource = await Promise.all(
    RAW_SOURCES.map((source) => scanRawSource(source, words)),
  );
  const boneyardHits = scanBoneyardDb(words);

  for (const word of words) {
    console.log(`\n=== ${word} ===`);

    const finalRows = matches.get(word);
    if (finalRows.length === 0) {
      console.log("  (not found in normalized/smart-mandarin.tsv)");
    }
    for (const row of finalRows) {
      console.log(
        `  final: qstring=${row.qstring} weight=${row.weight} source=${row.sourceId} tags=${row.tags}`,
      );
      const split = bestSplit(row, bestByQstring);
      if (split) {
        const delta = row.weight - split.weight;
        console.log(
          `  best split: ${split.left.phrase}+${split.right.phrase} weight=${split.weight.toFixed(6)} delta=${delta.toFixed(6)}`,
        );
      }
    }

    console.log("  raw sources:");
    let anyRawHit = false;
    for (const hit of boneyardHits.get(word)) {
      anyRawHit = true;
      console.log(`    [keykey-boneyard-bootstrap (base dictionary)] ${hit}`);
    }
    RAW_SOURCES.forEach((source, i) => {
      const hits = rawHitsBySource[i][words.indexOf(word)];
      for (const hit of hits) {
        anyRawHit = true;
        console.log(`    [${source.label}] ${hit}`);
      }
    });
    if (!anyRawHit) {
      console.log("    (no raw source rows found for this phrase)");
    }

    for (const row of finalRows) {
      const group = groups.get(row.qstring) ?? [];
      if (group.length <= 1) continue;
      console.log(`  homophone ranking for qstring ${row.qstring}:`);
      for (const entry of group) {
        const marker = entry.phrase === word ? "*" : " ";
        console.log(
          `    ${marker} ${entry.phrase}\tweight=${entry.weight}\tsource=${entry.sourceId}\ttags=${entry.tags}`,
        );
      }
    }
  }
}

main();
