#!/usr/bin/env node
//
// Enumerate動詞重疊式 (VV+O) 缺詞：詞庫裡有「洗洗」也有「洗澡」，但沒有「洗洗澡」，
// 於是 walker 在 ㄒㄧˇㄒㄧˇㄗㄠˇ 上走「喜+洗澡」。這個家族是構詞規則產生的，
// 逐詞回報補不完，本腳本改成從詞庫機械列舉 + 語料證據過濾。
//
// 流程：
//   1. 列舉：詞庫中 XX（同字同讀音重疊）× XY（X 開頭的二字詞），讀音一致 → 候選 XXY。
//   2. Walker check：算 XXY 讀音上目前的最佳路徑；已經走出 XXY 的候選不需要補。
//   3. 語料證據：在語料中數 XXY 的出現次數，並丟掉這三個字其實屬於別的詞的出現：
//      左邊界（「教育部|部長」→ 部部長）、被更長的詞整個蓋住（「多多指教」→ 多多指）、
//      Y 是下一個詞的詞頭（「多多|指正」→ 多多指）、以及 AABB 疊詞（斷斷續續 → 斷斷續）。
//   4. 產生權重：用有效分數反推，讓 XXY 贏過目前最佳路徑 SPLIT_MARGIN。
//
// Usage:
//   node scripts/audit/audit-reduplication-gaps.mjs [--min-count N] [--out FILE] CORPUS...
//
// 輸出可直接複核後併入 sources/chiaki-modern-overlay/unigrams.tsv。合併前仍須跑
// held-out 評估：三字節點帶 +2.0 長度加分，有搶走其他句子斷詞的風險。

import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const NORMALIZED_PATH =
  process.env.NORMALIZED_PATH ?? path.join(ROOT, "normalized/smart-mandarin.tsv");

// Manjusri Node.cpp c_phraseLengthBonus，log10 per extra syllable。
const PHRASE_LENGTH_BONUS = 1.0;
const SPLIT_MARGIN = 0.3;
// 左邊界檢查時往前看的最長詞長（字數）。
const MAX_LEFT_WORD = 4;
// 「這三個字其實是某個更長的詞的一部分」檢查時，考慮的最長詞長（字數）。
const MAX_ENCLOSING_WORD = 6;
// 右邊界檢查時往後看的最長詞長（字數）。
const MAX_RIGHT_WORD = 4;

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const lexicon = await loadLexicon(NORMALIZED_PATH);
  const candidates = enumerate(lexicon);
  console.error(`enumerated: ${candidates.length}`);

  const broken = candidates.filter((c) => {
    const best = bestPath(c.qstring, lexicon);
    if (!best) return false;
    c.currentPath = best.parts.map((row) => row.phrase).join("");
    c.weight = round6(best.score - lengthPrior(c.qstring.length / 2) + SPLIT_MARGIN);
    return c.currentPath !== c.phrase;
  });
  console.error(`walker already correct: ${candidates.length - broken.length}`);
  console.error(`needs a phrase: ${broken.length}`);

  if (options.corpora.length > 0) {
    await countInCorpora(broken, lexicon, options.corpora);
  }

  const kept = broken
    .filter((c) => (c.count ?? 0) >= options.minCount)
    .sort((a, b) => (b.count ?? 0) - (a.count ?? 0));
  console.error(`count >= ${options.minCount}: ${kept.length}`);

  const header = "# qstring\tphrase\tweight\tcount\taabb-count\tinside-longer-word\tsource-words\tcurrent-walker-path";
  const body = kept.map((c) =>
    [
      c.qstring,
      c.phrase,
      c.weight,
      c.count ?? "",
      c.aabbCount ?? "",
      c.swallowedCount ?? "",
      `${c.dup}+${c.vo}`,
      c.currentPath,
    ].join("\t"),
  );
  const text = `${[header, ...body].join("\n")}\n`;
  if (options.out) {
    fs.writeFileSync(options.out, text);
    console.error(`wrote ${relative(options.out)}`);
  } else {
    process.stdout.write(text);
  }
}

function parseArgs(argv) {
  const options = { minCount: 5, out: null, corpora: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--min-count") options.minCount = Number(argv[++index]);
    else if (arg === "--out") options.out = argv[++index];
    else if (arg.startsWith("--")) throw new Error(`unknown option ${arg}`);
    else options.corpora.push(arg);
  }
  return options;
}

async function loadLexicon(filePath) {
  const bestByQstring = new Map();
  const phrases = new Set();
  const known = new Set();
  // X + reading(X) -> reduplicated row / list of XY rows
  const dup = new Map();
  const verbObject = new Map();

  const rl = readline.createInterface({
    input: fs.createReadStream(filePath, { encoding: "utf8" }),
    crlfDelay: Infinity,
  });
  for await (const line of rl) {
    if (!line || line.startsWith("#")) continue;
    const [qstring, phrase, rawWeight] = line.split("\t");
    if (!qstring || !phrase) continue;
    const row = { qstring, phrase, weight: Number(rawWeight) };
    phrases.add(phrase);
    known.add(`${qstring}\t${phrase}`);
    const incumbent = bestByQstring.get(qstring);
    if (!incumbent || row.weight > incumbent.weight) bestByQstring.set(qstring, row);

    const chars = [...phrase];
    const readings = qstring.match(/../g) ?? [];
    if (chars.length !== 2 || readings.length !== 2) continue;
    const key = `${chars[0]}\t${readings[0]}`;
    if (chars[0] === chars[1] && readings[0] === readings[1]) {
      const held = dup.get(key);
      if (!held || row.weight > held.weight) dup.set(key, row);
    } else {
      if (!verbObject.has(key)) verbObject.set(key, []);
      verbObject.get(key).push(row);
    }
  }
  return { bestByQstring, phrases, known, dup, verbObject, pathMemo: new Map() };
}

function enumerate(lexicon) {
  const out = [];
  for (const [key, dup] of lexicon.dup) {
    for (const vo of lexicon.verbObject.get(key) ?? []) {
      const phrase = dup.phrase + [...vo.phrase][1];
      const qstring = dup.qstring + vo.qstring.slice(2);
      if (lexicon.known.has(`${qstring}\t${phrase}`)) continue;
      out.push({ phrase, qstring, dup: dup.phrase, vo: vo.phrase });
    }
  }
  return out;
}

function lengthPrior(syllableCount) {
  return PHRASE_LENGTH_BONUS * (syllableCount - 1);
}

function nodeScore(row) {
  return row.weight + lengthPrior(row.qstring.length / 2);
}

// Walker 的最佳路徑分數（有效分數，含長度加分），含整個 qstring 被別的詞佔用的單節點路徑。
function bestPath(qstring, lexicon) {
  const cached = lexicon.pathMemo.get(qstring);
  if (cached !== undefined) return cached;
  const syllableCount = qstring.length / 2;

  let best = null;
  const whole = lexicon.bestByQstring.get(qstring);
  if (whole) best = { score: nodeScore(whole), parts: [whole] };
  for (let at = 1; at < syllableCount; at += 1) {
    const left = lexicon.bestByQstring.get(qstring.slice(0, at * 2));
    const right = bestPath(qstring.slice(at * 2), lexicon);
    if (!left || !right) continue;
    const score = nodeScore(left) + right.score;
    if (!best || score > best.score) best = { score, parts: [left, ...right.parts] };
  }

  lexicon.pathMemo.set(qstring, best);
  return best;
}

async function countInCorpora(candidates, lexicon, corpora) {
  const counts = new Map();
  const aabb = new Map();
  const swallowed = new Map();
  for (const candidate of candidates) counts.set(candidate.phrase, 0);

  for (const file of corpora) {
    const rl = readline.createInterface({
      input: fs.createReadStream(file, { encoding: "utf8" }),
      crlfDelay: Infinity,
    });
    for await (const line of rl) {
      const chars = [...line];
      for (let at = 0; at + 3 <= chars.length; at += 1) {
        const trigram = chars[at] + chars[at + 1] + chars[at + 2];
        const seen = counts.get(trigram);
        if (seen === undefined) continue;
        if (coveredOnTheLeft(chars, at, lexicon.phrases)) continue;
        if (
          insideLongerWord(chars, at, lexicon.phrases) ||
          startsAnotherWordOnTheRight(chars, at, lexicon.phrases)
        ) {
          swallowed.set(trigram, (swallowed.get(trigram) ?? 0) + 1);
          continue;
        }
        // XXYY 是 AABB 疊詞（斷斷續續、清清楚楚），該補的是四字詞不是這三個字。
        if (chars[at + 3] === chars[at + 2]) {
          aabb.set(trigram, (aabb.get(trigram) ?? 0) + 1);
          continue;
        }
        counts.set(trigram, seen + 1);
      }
    }
    console.error(`scanned ${relative(file)}`);
  }

  for (const candidate of candidates) {
    candidate.count = counts.get(candidate.phrase);
    candidate.aabbCount = aabb.get(candidate.phrase) ?? 0;
    candidate.swallowedCount = swallowed.get(candidate.phrase) ?? 0;
  }
}

// 「教育部|部長」「大學|學院」這類假陽性：第一個 X 其實是前一個詞的詞尾。
// 若有長度 2..MAX_LEFT_WORD 的詞剛好結束在 at，這次出現不算數。
function coveredOnTheLeft(chars, at, phrases) {
  for (let length = 2; length <= MAX_LEFT_WORD; length += 1) {
    const start = at - length + 1;
    if (start < 0) break;
    if (phrases.has(chars.slice(start, at + 1).join(""))) return true;
  }
  return false;
}

// 「多多指正」「多多指導」：Y 是下一個詞的詞頭，這三個字只是前綴。
function startsAnotherWordOnTheRight(chars, at, phrases) {
  for (let length = 2; length <= MAX_RIGHT_WORD; length += 1) {
    const end = at + 2 + length;
    if (end > chars.length) break;
    if (phrases.has(chars.slice(at + 2, end).join(""))) return true;
  }
  return false;
}

// 「多多指」其實只是「多多指教」的前三個字，「面面相」來自「面面相覷」。
// 若有更長的既有詞完整蓋住這三個字，該補的是那個長詞，這次出現不算數。
function insideLongerWord(chars, at, phrases) {
  for (let length = 4; length <= MAX_ENCLOSING_WORD; length += 1) {
    for (let start = at - (length - 3); start <= at; start += 1) {
      if (start < 0 || start + length > chars.length) continue;
      if (phrases.has(chars.slice(start, start + length).join(""))) return true;
    }
  }
  return false;
}

function round6(value) {
  return Number(value.toFixed(6));
}

function relative(filePath) {
  return path.relative(ROOT, filePath) || filePath;
}

await main();
