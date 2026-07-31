#!/usr/bin/env node
// 從公報口語語料中找出「詞庫還沒收錄」的詞，輸出可人工複核的候選清單。
//
// 為什麼需要這支腳本：
//   build-bigram-stats 是拿 --lexicon 做最大匹配，只看得見已收錄的詞。實測 ly
//   候選檔涉及 16,560 個詞形，不在 DB 的只有 3 個——不是門檻設太嚴，是那條路徑
//   結構上就封閉。要補新詞必須另外做未知詞發現。
//
// 判準為什麼是兩條而不是一條：
//   單用內聚度（PMI）會撈到一堆切窗碎片。「人事行政總處」在固定寬度窗口下會產生
//   「事行政總」，它的內部字組共現極強，PMI 很高，但它不是詞。
//   單用左右鄰接熵會撈到自由組合。「相關的」「所有的」左右都能接各種字，熵很高，
//   但「的」黏誰都行，它同樣不是詞。
//   真正的詞要同時滿足「內部黏得緊」與「外部接得自由」，所以兩條判準必須並用。
//   實測 12M 字樣本：只用 PMI 得到的前 40 名幾乎全是碎片；只用熵得到的前 40 名
//   幾乎全是「的」系組合；兩者並用才篩得出 區間測速／疾管署／急難救助 這類。
//
// 讀音欄位是「提案」不是結論：
//   語料只有文字沒有讀音，這裡用每個字的最高權重讀音串起來當初稿。多音字一定會
//   錯（重、行、長、得），複核時必須逐筆確認，不能直接進 sources/。
//
// 用法：
//   node scripts/corpus/discover-ly-words.mjs
//   node scripts/corpus/discover-ly-words.mjs --limit-chars 12000000 --min-count 150

import { writeFileSync, existsSync } from "node:fs";
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import { DatabaseSync } from "node:sqlite";

const cfg = {
  input: "tmp/ly-speech.txt",
  out: "tmp/ly-words-candidates.tsv",
  review: "tmp/ly-words-review.tsv",
  db: "dist/dev/ChiaKeySource-dev.db",
  // 0 = 讀完整份語料
  limitChars: 0,
  minLen: 2,
  maxLen: 4,
  // 出現次數下限。67M 字全量建議 150 以上，取樣時可下修
  minCount: 100,
  // 內聚度下限。log((c*N)/min(左半count*右半count))
  minPmi: 3.0,
  // 左右鄰接熵下限，取兩側較小者
  minEntropy: 1.5,
  // n-gram 表超過這個大小就剪掉低頻項，避免整份語料爆記憶體
  maxEntries: 6_000_000,
  examples: 1,
};
for (let i = 2; i < process.argv.length; i += 1) {
  const a = process.argv[i];
  const v = () => process.argv[i + 1];
  if (a === "--input") cfg.input = v();
  else if (a === "--out") cfg.out = v();
  else if (a === "--review") cfg.review = v();
  else if (a === "--db") cfg.db = v();
  else if (a === "--limit-chars") cfg.limitChars = Number.parseInt(v(), 10);
  else if (a === "--min-count") cfg.minCount = Number.parseInt(v(), 10);
  else if (a === "--min-pmi") cfg.minPmi = Number.parseFloat(v());
  else if (a === "--min-entropy") cfg.minEntropy = Number.parseFloat(v());
  else if (a === "--max-len") cfg.maxLen = Number.parseInt(v(), 10);
  else if (a === "--max-entries") cfg.maxEntries = Number.parseInt(v(), 10);
  else if (a === "--help" || a === "-h") {
    console.log([
      "discover-ly-words.mjs — 從公報語料找出未收錄的詞",
      "",
      "  --input <file>        口語語料，一行一個發言輪次，預設 tmp/ly-speech.txt",
      "  --out <file>          候選清單（含統計欄位）",
      "  --review <file>       人工複核表，第一欄填 keep/drop",
      "  --db <file>           release DB，用來判斷哪些詞已收錄",
      "  --limit-chars <n>     只讀前 n 字，0 為全量，預設 0",
      "  --min-count <n>       出現次數下限，預設 100",
      "  --min-pmi <f>         內聚度下限，預設 3.0",
      "  --min-entropy <f>     左右鄰接熵下限（取較小側），預設 1.5",
      "  --max-len <n>         候選最長字數，預設 4",
      "  --max-entries <n>     n-gram 表剪枝門檻，預設 6000000",
    ].join("\n"));
    process.exit(0);
  }
}

if (!existsSync(cfg.db)) {
  console.error(`找不到 release DB：${cfg.db}\n請先執行 cargo run --release -- prepare-release`);
  process.exit(1);
}
if (!existsSync(cfg.input)) {
  console.error(`找不到語料：${cfg.input}\n請先執行 scripts/corpus/extract-ly-speech.mjs`);
  process.exit(1);
}

// 首尾虛詞。候選的第一個或最後一個字落在這裡就丟棄——「的垃圾」「有注意到」
// 「最嚴重的」這類在雙判準下仍會存活，但它們是句法組合不是詞。
const BOUNDARY_FUNCTION = new Set([
  ..."的了是在有會要所都最很就也把被和與及對於之而但因如把讓給向從到跟並還沒不無再又更已將可能該其此這那哪什麼們個很太也才只呢嗎吧啊喔耶嘛",
]);

// 議場專用詞。與 postprocess-ly-bigrams.mjs 的 REGISTER_DENY 同一個理由：
// 它們在這份語料裡頻率極高，但一般使用者不會打。
const REGISTER_DENY = [
  "本席", "本院", "大院", "逕付", "二讀", "三讀", "審查會", "朝野協商",
  "議事錄", "召委", "備詢", "宣讀", "修正動議", "散會", "公報", "院會",
  "報告事項", "討論事項", "程序委員會", "黨團", "提案人", "連署", "委員",
  "附帶決議", "國是論壇", "審查完竣", "primary",
];

const db = new DatabaseSync(cfg.db, { readOnly: true });
const known = new Set();
for (const r of db.prepare("select distinct current from unigrams").all()) known.add(r.current);
// 每個字的最高權重讀音，用來組讀音初稿。
// 權重是對數值，越接近 0 越常用（postprocess-ly-bigrams.mjs 取的也是 max），
// 所以要 desc 排序取第一筆；用 asc 會挑到該字最罕見的讀音。
const charReading = new Map();
for (const r of db
  .prepare("select qstring, current, probability from unigrams where length(current) = 1 order by probability desc")
  .all()) {
  if (!charReading.has(r.current)) charReading.set(r.current, r.qstring);
}
db.close();
console.log(`已收錄詞形 ${known.size.toLocaleString()}，單字讀音 ${charReading.size.toLocaleString()}`);

const HAN = /[一-鿿]+/g;

async function* runs() {
  let seen = 0;
  const rl = createInterface({ input: createReadStream(cfg.input, "utf8"), crlfDelay: Infinity });
  for await (const line of rl) {
    seen += line.length;
    if (cfg.limitChars && seen > cfg.limitChars) break;
    const m = line.match(HAN);
    if (m) yield m;
  }
}

// ── pass 1：統計 n-gram 次數（含已收錄詞，PMI 的分母需要）────────────────
const count = new Map();
let charTotal = 0;
let pruneFloor = 1;
function bump(k) {
  count.set(k, (count.get(k) ?? 0) + 1);
}
function prune() {
  if (count.size <= cfg.maxEntries) return;
  pruneFloor += 1;
  for (const [k, c] of count) if (c <= pruneFloor && k.length > 1) count.delete(k);
  console.log(`  剪枝：門檻 ≤${pruneFloor}，剩餘 ${count.size.toLocaleString()} 項`);
}

let processed = 0;
for await (const segs of runs()) {
  for (const s of segs) {
    charTotal += s.length;
    for (let i = 0; i < s.length; i += 1) {
      bump(s[i]);
      for (let n = cfg.minLen; n <= cfg.maxLen; n += 1) {
        if (i + n > s.length) break;
        bump(s.slice(i, i + n));
      }
    }
  }
  processed += 1;
  if (processed % 200000 === 0) prune();
}
prune();
console.log(`pass 1 完成：${charTotal.toLocaleString()} 字，n-gram 表 ${count.size.toLocaleString()} 項`);

// ── 初選：未收錄且達到次數門檻 ─────────────────────────────────────────
const shortlist = new Set();
for (const [g, c] of count) {
  if (g.length < cfg.minLen || c < cfg.minCount || known.has(g)) continue;
  if (BOUNDARY_FUNCTION.has(g[0]) || BOUNDARY_FUNCTION.has(g[g.length - 1])) continue;
  if (REGISTER_DENY.some((d) => g.includes(d))) continue;
  shortlist.add(g);
}
console.log(`初選（未收錄 + 次數 ≥${cfg.minCount} + 首尾非虛詞 + 非議場詞）：${shortlist.size.toLocaleString()}`);

// ── pass 2：只為初選項收集左右鄰字與例句 ───────────────────────────────
const left = new Map();
const right = new Map();
const example = new Map();
function note(map, g, ch) {
  let m = map.get(g);
  if (!m) map.set(g, (m = new Map()));
  m.set(ch, (m.get(ch) ?? 0) + 1);
}
for await (const segs of runs()) {
  const joined = segs.join("");
  for (const s of segs) {
    for (let i = 0; i < s.length; i += 1) {
      for (let n = cfg.minLen; n <= cfg.maxLen; n += 1) {
        if (i + n > s.length) break;
        const g = s.slice(i, i + n);
        if (!shortlist.has(g)) continue;
        note(left, g, i > 0 ? s[i - 1] : "^");
        note(right, g, i + n < s.length ? s[i + n] : "$");
        if (!example.has(g)) example.set(g, joined.slice(0, 60));
      }
    }
  }
}

function entropy(m) {
  if (!m) return 0;
  let total = 0;
  for (const v of m.values()) total += v;
  let e = 0;
  for (const v of m.values()) {
    const p = v / total;
    e -= p * Math.log(p);
  }
  return e;
}

// ── 評分與輸出 ─────────────────────────────────────────────────────────
const rows = [];
for (const g of shortlist) {
  const c = count.get(g);
  let worst = Infinity;
  for (let i = 1; i < g.length; i += 1) {
    const a = count.get(g.slice(0, i)) ?? 0;
    const b = count.get(g.slice(i)) ?? 0;
    if (a === 0 || b === 0) { worst = 0; break; }
    worst = Math.min(worst, a * b);
  }
  if (!worst || !Number.isFinite(worst)) continue;
  const pmi = Math.log((c * charTotal) / worst);
  const be = Math.min(entropy(left.get(g)), entropy(right.get(g)));
  if (pmi < cfg.minPmi || be < cfg.minEntropy) continue;
  const reading = [...g].map((ch) => charReading.get(ch) ?? "?").join("");
  rows.push({ g, c, pmi, be, reading, missing: [...g].some((ch) => !charReading.has(ch)), ex: example.get(g) ?? "" });
}
rows.sort((a, b) => b.pmi + b.be - (a.pmi + a.be));
console.log(`通過雙判準（PMI ≥${cfg.minPmi}、鄰接熵 ≥${cfg.minEntropy}）：${rows.length.toLocaleString()}`);

writeFileSync(
  cfg.out,
  ["# phrase\tcount\tpmi\tbranching_entropy\treading_draft\treading_complete"]
    .concat(rows.map((r) => `${r.g}\t${r.c}\t${r.pmi.toFixed(3)}\t${r.be.toFixed(3)}\t${r.reading}\t${r.missing ? "false" : "true"}`))
    .join("\n") + "\n",
  "utf8",
);
writeFileSync(
  cfg.review,
  ["# decision\tphrase\tcount\treading_draft\texample", "# 第一欄填 keep 或 drop；reading_draft 由單字最高權重讀音組成，多音字必須逐筆確認"]
    .concat(rows.map((r) => `\t${r.g}\t${r.c}\t${r.reading}\t${r.ex}`))
    .join("\n") + "\n",
  "utf8",
);
console.log(`候選清單 → ${cfg.out}`);
console.log(`複核表   → ${cfg.review}`);
console.log("");
console.log("下一步：複核表第一欄填 keep/drop，逐筆確認 reading_draft 的多音字，");
console.log("        再依 sources/<id>/unigrams.tsv 的 qstring/phrase/weight/tags 格式建立覆蓋層。");
