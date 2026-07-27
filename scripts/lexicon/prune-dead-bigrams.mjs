#!/usr/bin/env node
// 依 ChiaKey 走訪器的可達性，把來源 bigrams.tsv 中永遠不會生效的列剪掉。
//
// 剪除條件（分類依據見 Docs/WalkerScoring.zh-TW.md）：
//   A 不是 (qstring, previous) 群組中機率最高者 → at(0) 取不到，永不執行
//   B 是群組第一名，但機率 <= 該讀音最弱候選    → 任何學習狀態都贏不了
//
// 不剪 C（恆生效）與 D（使用者學過較弱候選才生效）。D 是學習翻轉排序後的救援路徑。
//
// 分類是對「當前 release DB」成立的。之後若在某讀音下新增了更弱的候選，該讀音的
// 下界會下降，部分 B 會轉成 D；所以每次大幅改動詞庫後應重跑 audit 再決定。
//
// 用法：
//   node scripts/lexicon/prune-dead-bigrams.mjs              # 試算，不寫檔
//   node scripts/lexicon/prune-dead-bigrams.mjs --apply      # 實際寫回

import { DatabaseSync } from "node:sqlite";
import { readFileSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const cfg = {
  db: "dist/dev/ChiaKeySource-dev.db",
  sourcesDir: "sources",
  apply: process.argv.includes("--apply"),
  // 可剪的 key 同時存在多個來源檔時，預設整組略過（無法判斷該剪哪一份）。
  // 指定 --prune-shared-from <source-id> 就只從該來源剪掉，其餘來源保留。
  pruneSharedFrom: null,
};
for (let i = 2; i < process.argv.length; i += 1) {
  if (process.argv[i] === "--db") cfg.db = process.argv[i + 1];
  if (process.argv[i] === "--sources-dir") cfg.sourcesDir = process.argv[i + 1];
  if (process.argv[i] === "--prune-shared-from") cfg.pruneSharedFrom = process.argv[i + 1];
}

if (!existsSync(cfg.db)) {
  console.error(`找不到 ${cfg.db}。先跑：cargo run --release -- prepare-release`);
  process.exit(1);
}
const db = new DatabaseSync(cfg.db, { readOnly: true });

// 每個讀音的候選分數區間；學習只重排不改分，排頭會落在 [lo, hi] 之間。
const node = new Map();
for (const r of db.prepare("select qstring, min(probability) lo, max(probability) hi from unigrams group by qstring").all()) {
  node.set(r.qstring, r);
}

// (qstring, previous) 群組中只有機率最高那筆會被 at(0) 讀到。
const head = new Map();
const all = db.prepare("select qstring, previous, current, probability from bigrams").all();
for (const r of all) {
  const k = `${r.qstring} ${r.previous}`;
  const cur = head.get(k);
  if (!cur || r.probability > cur.probability) head.set(k, r);
}

const prunable = new Map(); // key -> reason
for (const r of all) {
  const key = `${r.qstring}\t${r.previous}\t${r.current}`;
  const h = head.get(`${r.qstring} ${r.previous}`);
  if (h.current !== r.current || Math.abs(h.probability - r.probability) > 1e-12) {
    prunable.set(key, "A");
    continue;
  }
  const n = node.get(r.qstring.split(" ").pop());
  if (n && r.probability <= n.lo) prunable.set(key, "B");
}
db.close();

// 同一 key 出現在多個來源檔時，DB 只留下最後匯入的那一筆，無法判斷剪掉哪一份
// 才正確，所以整組跳過。
const owners = new Map();
const files = [];
for (const entry of readdirSync(cfg.sourcesDir)) {
  const file = join(cfg.sourcesDir, entry, "bigrams.tsv");
  if (!existsSync(file)) continue;
  files.push([entry, file]);
  for (const line of readFileSync(file, "utf8").split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const f = line.split("\t");
    if (f.length < 4) continue;
    const key = `${f[0]}\t${f[1]}\t${f[2]}`;
    if (!owners.has(key)) owners.set(key, new Set());
    owners.get(key).add(entry);
  }
}

let totalRemoved = 0;
let totalShared = 0;
console.log(`${"來源".padEnd(38)}${"原始".padStart(8)}${"剪除".padStart(8)}${"保留".padStart(8)}${"跨來源略過".padStart(12)}`);
for (const [name, file] of files) {
  const lines = readFileSync(file, "utf8").split(/\r?\n/);
  const kept = [];
  let before = 0;
  let removed = 0;
  let shared = 0;
  for (const line of lines) {
    if (!line) continue;
    if (line.startsWith("#")) { kept.push(line); continue; }
    const f = line.split("\t");
    if (f.length < 4) { kept.push(line); continue; }
    before += 1;
    const key = `${f[0]}\t${f[1]}\t${f[2]}`;
    if (prunable.has(key)) {
      if ((owners.get(key)?.size ?? 1) > 1 && cfg.pruneSharedFrom !== name) {
        shared += 1;
        kept.push(line);
        continue;
      }
      removed += 1;
      continue;
    }
    kept.push(line);
  }
  totalRemoved += removed;
  totalShared += shared;
  console.log(`${name.padEnd(38)}${String(before).padStart(8)}${String(removed).padStart(8)}${String(before - removed).padStart(8)}${String(shared).padStart(12)}`);
  if (cfg.apply && removed > 0) writeFileSync(file, `${kept.join("\n")}\n`, "utf8");
}

console.log(`\n合計剪除 ${totalRemoved} 列，跨來源重複而略過 ${totalShared} 列。`);
console.log(cfg.apply ? "已寫回來源檔。" : "試算模式，未寫檔。加 --apply 實際執行。");
