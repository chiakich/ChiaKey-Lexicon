#!/usr/bin/env node
// Step 1 of the pipeline: turn the raw corpora into the one-sentence-per-line
// text files `extract` consumes.
//
// Sentences are split on full-width 。！？； and newlines; a segment is kept when
// it holds at least 8 characters and contains Han. Each article contributes its
// title followed by its body, and sources are concatenated in the fixed order
// below so the output is byte-stable.
//
// Reconstruction status is recorded in ../README.md: govnews lands within 0.07%
// of the shipping corpus, ntpc does not yet reproduce. Not byte-exact.
//
// Usage:
//   node scripts/build-corpora.mjs --out DIR [--only govnews|ntpc|ptt|ly]
//   node scripts/build-corpora.mjs --out DIR --ptt-json path/to/ppt_pretrain.json
//   node scripts/build-corpora.mjs --out DIR --ly-corpus tmp/ly-speech.txt

import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");

function parseArgs(argv) {
  const args = { out: null, only: null, pttJson: null, lyCorpus: null };
  for (let i = 0; i < argv.length; i += 1) {
    if (argv[i] === "--out") args.out = argv[++i];
    else if (argv[i] === "--only") args.only = argv[++i];
    else if (argv[i] === "--ptt-json") args.pttJson = argv[++i];
    else if (argv[i] === "--ly-corpus") args.lyCorpus = argv[++i];
    else {
      console.error(`unknown argument: ${argv[i]}`);
      process.exit(1);
    }
  }
  if (!args.out) {
    console.error("--out DIR required");
    process.exit(1);
  }
  return args;
}

const HAN = /[㐀-䶿一-鿿豈-﫿]/;

// Sinica's 網頁內容 holds HTML. Tags are removed in place; character entities are
// deliberately left alone, matching the shipping corpus, which still carries 373
// lines containing `&nbsp;`.
function sentences(text) {
  const out = [];
  const stripped = String(text ?? "").replace(/<[^>]*>/g, "");
  for (const raw of stripped.split(/[。！？；\n\r]+/)) {
    const segment = raw.trim();
    if ([...segment].length >= 8 && HAN.test(segment)) out.push(segment);
  }
  return out;
}

const stripBom = (text) => text.replace(/^﻿/, "");
const readJson = (file) => JSON.parse(stripBom(fs.readFileSync(file, "utf8")));

// Minimal RFC4180 reader: the gov-news CSVs quote bodies that contain commas and
// newlines, so a line-based split would corrupt them.
function readCsv(file) {
  const text = stripBom(fs.readFileSync(file, "utf8"));
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i += 1;
        } else quoted = false;
      } else field += c;
      continue;
    }
    if (c === '"') quoted = true;
    else if (c === ",") {
      row.push(field);
      field = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i += 1;
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else field += c;
  }
  if (field !== "" || row.length) {
    row.push(field);
    rows.push(row);
  }
  const header = rows.shift() ?? [];
  return rows
    .filter((r) => r.length >= header.length)
    .map((r) => Object.fromEntries(header.map((h, idx) => [h, r[idx]])));
}

// Field names differ per publisher; the pipeline only needs title + body.
const GOVNEWS = [
  { file: "sources/taiwan-gov-news-ey/raw/news.json", kind: "json", title: "標題", body: "內容" },
  { file: "sources/taiwan-gov-news-mac/raw/news.csv", kind: "csv", title: "標題", body: "內文" },
  { file: "sources/taiwan-gov-news-sinica/raw/news.json", kind: "json", title: "標題", body: "網頁內容" },
  { file: "sources/taiwan-gov-news-hakka/raw/news.json", kind: "json", title: "name", body: "description" },
];
const NTPC = { file: "sources/taiwan-gov-news-ntpc/raw/news.csv", kind: "csv", title: "Subject_", body: "Content" };

function articleLines(spec) {
  const records = spec.kind === "json" ? readJson(path.join(ROOT, spec.file)) : readCsv(path.join(ROOT, spec.file));
  const lines = [];
  for (const record of records) {
    lines.push(...sentences(record[spec.title]));
    lines.push(...sentences(record[spec.body]));
  }
  return lines;
}

function write(outDir, name, lines) {
  const file = path.join(outDir, name);
  fs.writeFileSync(file, lines.length ? `${lines.join("\n")}\n` : "");
  console.log(`${name}\t${lines.length} lines`);
}

const args = parseArgs(process.argv.slice(2));
fs.mkdirSync(args.out, { recursive: true });
const want = (name) => !args.only || args.only === name;

if (want("govnews")) {
  const lines = GOVNEWS.flatMap(articleLines);
  write(args.out, "govnews-train.txt", lines);
}

if (want("ntpc")) {
  // The shipping run split ntpc in half by sentence: first half trains, second
  // half is the written-register held-out set.
  const lines = articleLines(NTPC);
  const half = Math.ceil(lines.length / 2);
  write(args.out, "ntpc-train.txt", lines.slice(0, half));
  write(args.out, "ntpc-test.txt", lines.slice(half));
}

if (want("ly")) {
  if (!args.lyCorpus) {
    console.log("ly\tskipped (pass --ly-corpus; build it with scripts/corpus/build-ly-corpus.sh)");
  } else {
    // build-ly-corpus.sh already strips speaker labels and timestamps and writes
    // one utterance per line; only the sentence split is left to do here.
    const lines = [];
    const rl = readline.createInterface({ input: fs.createReadStream(args.lyCorpus), crlfDelay: Infinity });
    for await (const line of rl) lines.push(...sentences(line));
    write(args.out, "ly-train.txt", lines);
  }
}

if (want("ptt")) {
  if (!args.pttJson) {
    console.log("ptt\tskipped (pass --ptt-json; see ../../README.md for the dataset id)");
  } else {
    // yuhuanstudio/PTT-pretrain-zhtw ships {"text": "..."} records holding whole
    // posts: 作者/看板/時間 headers, quoted lines and URLs are dropped, the body
    // after 標題/內文 is kept.
    const raw = readJson(args.pttJson);
    const records = Array.isArray(raw) ? raw : (raw.data ?? []);
    const lines = [];
    for (const record of records) {
      const text = typeof record === "string" ? record : (record.text ?? record.content ?? "");
      for (const line of String(text).split(/\n/)) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        if (/^(作者|看板|時間|發信站|文章網址|來自)[:：]/.test(trimmed)) continue;
        if (/^[:：>]/.test(trimmed)) continue;
        if (/^※/.test(trimmed)) continue;
        lines.push(...sentences(trimmed.replace(/https?:\/\/\S+/g, "")));
      }
    }
    write(args.out, "ptt-all.txt", lines);
  }
}
