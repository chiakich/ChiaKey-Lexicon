#!/usr/bin/env node
// Compares each phrase's engine-canonical qstring (the highest-weight row in
// normalized/smart-mandarin.tsv, i.e. what the walker actually defaults to)
// against the reading(s) 教育部《重編國語辭典修訂本》(moedict-data's
// dict-revised.json) records for that title, and reports phrases where the
// engine's default reading does not match any moedict heteronym.
//
// This is a heuristic screen, not an automatic fix: moedict is not
// exhaustive of Taiwan colloquial readings (e.g. 什麼 shénme is common usage
// but may not be moedict's headline reading), and some mismatches are
// intentional project overrides. Every hit needs human review before
// touching source data — see the "how_to_apply" column notes.
//
// dict-revised.json is not vendored in this repo (license CC BY-ND 3.0 TW,
// not cleared for redistribution here — see README's 授權政策 section).
// Fetch it into tmp/ (gitignored) yourself:
//
//   git clone --depth 1 https://github.com/g0v/moedict-data.git tmp/moedict-data-src
//   xz -dk tmp/moedict-data-src/dict-revised.json.xz -c > tmp/moedict-data/dict-revised.json
//
// Usage:
//   cargo build --release
//   node scripts/audit/audit-moedict-readings.mjs [--out tmp/moedict-mismatches.tsv] [--limit 500]
//
// For each hit, inspect the phrase with:
//   node scripts/audit/explain-weight.mjs <phrase>

import fs from "node:fs";
import readline from "node:readline";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

function parseArgs(argv) {
  const args = {
    lexicon: path.join(ROOT, "normalized/smart-mandarin.tsv"),
    moedict: path.join(ROOT, "tmp/moedict-data/dict-revised.json"),
    out: path.join(ROOT, "tmp/moedict-mismatches.tsv"),
    bin: path.join(ROOT, "target/release/chiakey-lexicon"),
    limit: Infinity,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--lexicon") args.lexicon = argv[++i];
    else if (arg === "--moedict") args.moedict = argv[++i];
    else if (arg === "--out") args.out = argv[++i];
    else if (arg === "--bin") args.bin = argv[++i];
    else if (arg === "--limit") args.limit = Number(argv[++i]);
    else {
      console.error(`unknown argument: ${arg}`);
      process.exit(1);
    }
  }
  return args;
}

async function forEachLine(filePath, fn) {
  const rl = readline.createInterface({
    input: fs.createReadStream(filePath, "utf8"),
    crlfDelay: Infinity,
  });
  for await (const line of rl) {
    if (line.length === 0) continue;
    fn(line);
  }
}

// phrase -> { qstring, weight, sourceId, tags }, keeping the highest-weight
// row (ties keep the first-seen row). This mirrors the "engine-canonical
// qstring" logic bigram.rs's by_phrase uses: the row the walker actually
// defaults to for that phrase.
async function loadLexiconBest(lexiconPath) {
  const best = new Map();
  const allRowsByPhrase = new Map();
  await forEachLine(lexiconPath, (line) => {
    const [qstring, phrase, weightStr, sourceId, tags] = line.split("\t");
    const weight = Number(weightStr);
    const row = { qstring, weight, sourceId, tags };
    const current = best.get(phrase);
    if (!current || weight > current.weight) {
      best.set(phrase, row);
    }
    let rows = allRowsByPhrase.get(phrase);
    if (!rows) {
      rows = [];
      allRowsByPhrase.set(phrase, rows);
    }
    rows.push(row);
  });
  return { best, allRowsByPhrase };
}

// title -> Set(raw moedict bopomofo strings), aggregated across every
// dict-revised.json entry sharing that title (character entries and phrase
// entries can both exist for the same title).
function loadMoedictReadings(moedictPath) {
  const raw = fs.readFileSync(moedictPath, "utf8");
  const entries = JSON.parse(raw);
  const readingsByTitle = new Map();
  const firstDefByTitle = new Map();
  for (const entry of entries) {
    const title = entry.title;
    if (!title || title.includes("{[")) continue; // unencodable rare-char placeholder
    for (const heteronym of entry.heteronyms ?? []) {
      const bopomofo = heteronym.bopomofo;
      if (!bopomofo) continue;
      let set = readingsByTitle.get(title);
      if (!set) {
        set = new Set();
        readingsByTitle.set(title, set);
      }
      set.add(bopomofo);
      if (!firstDefByTitle.has(title)) {
        const def = heteronym.definitions?.[0]?.def;
        if (def) firstDefByTitle.set(title, def.slice(0, 40));
      }
    }
  }
  return { readingsByTitle, firstDefByTitle };
}

// Runs every unique bopomofo string through `chiakey-lexicon bpmf-to-qstring`
// in one process and returns a Map(bopomofo -> qstring|null).
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
    const line = lines[i] ?? "";
    const [qstring] = line.split("\t");
    map.set(list[i], qstring || null);
  }
  return map;
}

function decodeBopomofoBatch(bin, qstrings) {
  const list = Array.from(qstrings);
  const result = spawnSync(bin, ["qstring-to-bpmf"], {
    input: list.join("\n") + "\n",
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 512,
  });
  if (result.status !== 0) {
    console.error(result.stderr);
    throw new Error(`${bin} qstring-to-bpmf exited with ${result.status}`);
  }
  const lines = result.stdout.split("\n");
  const map = new Map();
  for (let i = 0; i < list.length; i += 1) {
    map.set(list[i], lines[i] || "");
  }
  return map;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));

  if (!fs.existsSync(args.lexicon)) {
    console.error(`lexicon tsv not found at ${args.lexicon}`);
    console.error("Run `cargo run --release -- prepare-release` first, or pass --lexicon.");
    process.exit(1);
  }
  if (!fs.existsSync(args.moedict)) {
    console.error(`moedict dict-revised.json not found at ${args.moedict}`);
    console.error("See the header of this script for how to fetch it into tmp/.");
    process.exit(1);
  }
  if (!fs.existsSync(args.bin)) {
    console.error(`binary not found at ${args.bin}; run \`cargo build --release\` first.`);
    process.exit(1);
  }

  console.error("loading lexicon...");
  const { best, allRowsByPhrase } = await loadLexiconBest(args.lexicon);
  console.error(`  ${best.size} distinct phrases`);

  console.error("loading moedict-data...");
  const { readingsByTitle, firstDefByTitle } = loadMoedictReadings(args.moedict);
  console.error(`  ${readingsByTitle.size} distinct titles with readings`);

  const candidatePhrases = [];
  for (const phrase of best.keys()) {
    if (readingsByTitle.has(phrase)) candidatePhrases.push(phrase);
  }
  console.error(`  ${candidatePhrases.length} phrases present in both`);

  const allBopomofo = new Set();
  for (const phrase of candidatePhrases) {
    for (const bopomofo of readingsByTitle.get(phrase)) allBopomofo.add(bopomofo);
  }
  console.error(`converting ${allBopomofo.size} unique moedict readings to qstring...`);
  const bopomofoToQstring = convertBopomofoBatch(args.bin, allBopomofo);

  const mismatches = [];
  for (const phrase of candidatePhrases) {
    const bestRow = best.get(phrase);
    const moedictQstrings = new Set();
    for (const bopomofo of readingsByTitle.get(phrase)) {
      const qstring = bopomofoToQstring.get(bopomofo);
      if (qstring) moedictQstrings.add(qstring);
    }
    if (moedictQstrings.size === 0) continue; // couldn't convert any reading, skip
    if (moedictQstrings.has(bestRow.qstring)) continue; // engine default matches moedict

    const altMatch = (allRowsByPhrase.get(phrase) ?? []).find(
      (row) => row.qstring !== bestRow.qstring && moedictQstrings.has(row.qstring),
    );

    mismatches.push({
      phrase,
      bestRow,
      moedictQstrings: Array.from(moedictQstrings),
      moedictBopomofo: Array.from(readingsByTitle.get(phrase)),
      def: firstDefByTitle.get(phrase) ?? "",
      altMatch,
    });
    if (mismatches.length >= args.limit) break;
  }
  console.error(`${mismatches.length} mismatches found`);

  console.error("decoding qstrings to bopomofo for display...");
  const qstringsToDecode = new Set();
  for (const m of mismatches) {
    qstringsToDecode.add(m.bestRow.qstring);
    if (m.altMatch) qstringsToDecode.add(m.altMatch.qstring);
  }
  const qstringToBopomofo = decodeBopomofoBatch(args.bin, qstringsToDecode);

  // Some sources (punctuation lists, _ctrl_ candidates) use the qstring
  // column as a non-phonetic marker rather than a real bopomofo-derived
  // reading; those never decode and are not genuine polyphone mismatches.
  const beforeFilter = mismatches.length;
  const filtered = mismatches.filter((m) => qstringToBopomofo.get(m.bestRow.qstring));
  mismatches.length = 0;
  mismatches.push(...filtered);
  console.error(`  dropped ${beforeFilter - mismatches.length} rows with non-phonetic qstrings`);

  const header = [
    "phrase",
    "current_bpmf",
    "current_qstring",
    "current_weight",
    "current_source",
    "current_tags",
    "qstring_basis",
    "moedict_bpmf_options",
    "class",
    "alt_bpmf_in_lexicon",
    "alt_weight",
    "alt_source",
    "moedict_def",
  ].join("\t");

  // "supplemental" rime-essay rows never had a per-phrase reading to begin
  // with: infer_overlay_qstrings (src/importers.rs) built their qstring by
  // concatenating each character's *already-decided single-char* reading,
  // because rime-essay's essay.txt is just "phrase, frequency" with no
  // zhuyin. A moedict mismatch there isn't a discovered bug, it's the known
  // failure mode of that heuristic surfacing again — still worth using
  // moedict to replace the guess, but don't read it as "this was wrong and
  // is now proven". Every other tag (including rime-essay's own
  // overlap-rerank/existing-rerank, which only reweight an already-real
  // qstring) keeps a reading some source actually asserted.
  const isInferred = (tags) => tags.includes(",supplemental");

  let inferredCount = 0;
  const rows = mismatches.map((m) => {
    const klass = m.altMatch ? "A_lexicon_has_matching_alt" : "B_no_matching_alt_in_lexicon";
    const basis = isInferred(m.bestRow.tags) ? "inferred_char_concat" : "authoritative";
    if (basis === "inferred_char_concat") inferredCount += 1;
    return [
      m.phrase,
      qstringToBopomofo.get(m.bestRow.qstring) ?? "",
      m.bestRow.qstring,
      m.bestRow.weight,
      m.bestRow.sourceId,
      m.bestRow.tags,
      basis,
      m.moedictBopomofo.join(" | "),
      klass,
      m.altMatch ? qstringToBopomofo.get(m.altMatch.qstring) ?? "" : "",
      m.altMatch ? m.altMatch.weight : "",
      m.altMatch ? m.altMatch.sourceId : "",
      m.def,
    ].join("\t");
  });

  fs.mkdirSync(path.dirname(args.out), { recursive: true });
  fs.writeFileSync(args.out, [header, ...rows].join("\n") + "\n", "utf8");
  console.error(`wrote ${rows.length} rows to ${args.out}`);
  console.error(
    `  ${rows.length - inferredCount} rows have qstring_basis=authoritative (some source asserted this reading directly)`,
  );
  console.error(
    `  ${inferredCount} rows have qstring_basis=inferred_char_concat (rime-essay supplemental guess via per-character concatenation — review separately, these are "known-unreliable" not "newly found wrong")`,
  );
  console.error(
    "class A = engine already has a row matching a moedict reading, but a different (mismatching) row currently wins — likely a rerank/weight fix.",
  );
  console.error(
    "class B = no row in the lexicon matches any moedict reading for this phrase — could be a missing reading, or a legitimate colloquial/proper-noun reading moedict doesn't record. Needs individual judgment.",
  );
}

main();
