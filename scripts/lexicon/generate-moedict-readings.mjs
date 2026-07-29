#!/usr/bin/env node
// Regenerates sources/chiaki-modern-overlay/reading-supplements.tsv from
// audit-moedict-readings.mjs's output, using moedict-data (教育部《重編國語
// 辭典修訂本》) as the reading reference for this pass.
//
// reading-supplements.tsv itself is a generic "known-good alternate reading"
// table, not moedict-specific — it just happens that moedict-data is the
// reference this script draws from. The importer (reading_supplement_records
// in src/importers.rs) only ever ADDS a listed reading if the phrase is
// already in the lexicon and doesn't already have that exact reading; it
// never removes or demotes whatever reading currently wins, and caps a new
// reading's weight below any different phrase that already holds the same
// qstring. That makes it safe to apply broadly: a phrase whose current top
// reading is non-standard/colloquial keeps working exactly as before, it
// just also gains the standard reading as an equally typeable alternative
// (same weight, when there's no qstring collision).
//
// Readings are stored as qstring directly (not bopomofo) — this script does
// the bopomofo->qstring conversion once here via the same `bpmf-to-qstring`
// encoding every other importer uses, so the release build doesn't need to
// re-derive it.
//
// 教育部《重編國語辭典修訂本》採「創用CC－姓名標示－禁止改作 3.0 台灣」授權條款釋出
// （姓名標示：教育部（終身教育司）；<https://ti-wb.github.io/creativecommon-tw/index.html>），
// 允許重製與散布（含商業性利用），不允許改作。這裡只重製讀音本身（未改作），釋出時請保留
// 這份姓名標示 — 見 sources/chiaki-modern-overlay/README.md 與主 README 致謝。
//
// Usage:
//   node scripts/audit/audit-moedict-readings.mjs
//   cargo build --release
//   node scripts/lexicon/generate-moedict-readings.mjs [--in tmp/moedict-mismatches.tsv] [--out sources/chiaki-modern-overlay/reading-supplements.tsv] [--bin target/release/chiakey-lexicon]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

function parseArgs(argv) {
  const args = {
    in: path.join(ROOT, "tmp/moedict-mismatches.tsv"),
    out: path.join(ROOT, "sources/chiaki-modern-overlay/reading-supplements.tsv"),
    bin: path.join(ROOT, "target/release/chiakey-lexicon"),
  };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === "--in") args.in = argv[++i];
    else if (argv[i] === "--out") args.out = argv[++i];
    else if (argv[i] === "--bin") args.bin = argv[++i];
  }
  return args;
}

function convertBopomofoBatch(bin, bopomofoStrings) {
  const list = Array.from(bopomofoStrings);
  const result = spawnSync(bin, ["bpmf-to-qstring"], {
    input: list.join("\n") + "\n",
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 512,
  });
  if (result.status !== 0) {
    console.error(result.stderr);
    throw new Error(`${bin} bpmf-to-qstring exited with ${result.status}`);
  }
  const lines = result.stdout.split("\n");
  const map = new Map();
  for (let i = 0; i < list.length; i += 1) {
    const [qstring] = (lines[i] ?? "").split("\t");
    map.set(list[i], qstring || null);
  }
  return map;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!fs.existsSync(args.in)) {
    console.error(`${args.in} not found — run scripts/audit/audit-moedict-readings.mjs first.`);
    process.exit(1);
  }
  if (!fs.existsSync(args.bin)) {
    console.error(`binary not found at ${args.bin}; run \`cargo build --release\` first.`);
    process.exit(1);
  }

  const lines = fs.readFileSync(args.in, "utf8").split("\n").filter(Boolean);
  const [header, ...body] = lines;
  const columns = header.split("\t");
  const col = Object.fromEntries(columns.map((name, i) => [name, i]));

  // phrase -> Set(bopomofo)
  const readingsByPhrase = new Map();
  const allBopomofo = new Set();
  for (const line of body) {
    const fields = line.split("\t");
    const phrase = fields[col.phrase];
    const moedictOptions = fields[col.moedict_bpmf_options].split(" | ").filter(Boolean);
    if (moedictOptions.length === 0) continue;
    let set = readingsByPhrase.get(phrase);
    if (!set) {
      set = new Set();
      readingsByPhrase.set(phrase, set);
    }
    for (const bopomofo of moedictOptions) {
      set.add(bopomofo);
      allBopomofo.add(bopomofo);
    }
  }

  console.error(`converting ${allBopomofo.size} unique readings to qstring...`);
  const bopomofoToQstring = convertBopomofoBatch(args.bin, allBopomofo);

  const rows = [];
  let unconverted = 0;
  for (const [phrase, readings] of readingsByPhrase) {
    for (const bopomofo of readings) {
      const qstring = bopomofoToQstring.get(bopomofo);
      if (!qstring) {
        unconverted += 1;
        continue;
      }
      rows.push(`${qstring}\t${phrase}\tmoedict-reviewed`);
    }
  }
  rows.sort();

  fs.writeFileSync(args.out, rows.join("\n") + "\n", "utf8");
  console.error(`wrote ${rows.length} reading rows for ${readingsByPhrase.size} phrases to ${args.out}`);
  if (unconverted > 0) console.error(`  skipped ${unconverted} readings that failed to convert to qstring`);
  console.error(
    "the release importer only adds readings not already present and caps weight below any qstring collision, so this can be regenerated and committed directly — see reading_supplement_records in src/importers.rs.",
  );
}

main();
