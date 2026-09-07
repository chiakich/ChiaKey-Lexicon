#!/usr/bin/env node
//
// 「不」的讀音覆蓋缺口：詞庫裡「吃不下」只有 ㄅㄨˊ（=@）與 ㄅㄨ˙（{n）兩個讀音，
// 沒有 ㄅㄨˋ（L_），於是使用者照字面打 ㄔㄅㄨˋㄒㄧㄚˋ 時 walker 只能走
// 「吃+部下」。ㄅㄨˊ／ㄅㄨ˙ 是變調與輕聲的結果，實際輸入時使用者幾乎一律打
// 本調 ㄅㄨˋ，所以每個含「不」的詞都應該同時能用 ㄅㄨˋ 打出來。
//
// 這裡只列舉「詞庫已有此詞、但缺 ㄅㄨˋ 讀音」的列，輸出成
// reading-supplements.tsv 的格式；權重由 release 端的 reading_supplement_records
// 決定（無撞碼時沿用該詞自己的最高權重，撞碼時壓到既有詞下方）。
//
// Usage:
//   node scripts/audit/audit-bu-tone-readings.mjs [--out FILE]

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const NORMALIZED_PATH =
  process.env.NORMALIZED_PATH ?? path.join(ROOT, "normalized/smart-mandarin.tsv");

// ㄅㄨˋ / ㄅㄨˊ / ㄅㄨ˙ 的 qstring 音節（src/phonetics.rs 的編碼，每音節 2 bytes）。
const BU_FALLING = "L_";
const BU_RISING = "=@";
const BU_NEUTRAL = "{n";
const TAGS = "bu-sandhi-reading,bu-tone-coverage";

// 「不派」是語料切詞碎片，補了 ㄅㄨˋ 之後 eff(不派)+eff(佛教) 會壓過權重極低的
// 「部派佛教」(-4.837)。這是唯一在全詞庫 walker 複驗中出現的回歸，直接排除。
const EXCLUDE = new Set(["不派"]);

// 他／她／它／牠 同音，補讀音等於在「不管他」的 qstring 上多開一列「不管她」。
// 84e5fe7 已就上一批 不 batch 做過這個決定：這種列一律不補，改由 explicit.tsv
// 的 pronoun-order 逐對排序處理。撞碼上限擋不住這件事——它是以補讀音當時的權重
// 計算，之後 naer-word-frequency 重算權重就失效了。
const PRONOUNS = /[她它牠]/u;

function main() {
  const args = process.argv.slice(2);
  let out = path.join(ROOT, "tmp/bu-tone-readings.tsv");
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === "--out") out = args[++i];
  }

  const exact = new Set();
  const rows = [];
  for (const line of fs.readFileSync(NORMALIZED_PATH, "utf8").split("\n")) {
    if (!line) continue;
    const [qstring, phrase] = line.split("\t");
    if (!qstring || !phrase) continue;
    exact.add(`${qstring}\t${phrase}`);
    if (phrase.includes("不")) rows.push([qstring, phrase]);
  }

  const supplements = new Set();
  for (const [qstring, phrase] of rows) {
    const syllables = qstring.match(/../g) ?? [];
    // 兒化韻的列（「小不點兒」）音節數少於字數，位置對不上，跳過。
    if (syllables.length !== [...phrase].length) continue;
    if (EXCLUDE.has(phrase) || PRONOUNS.test(phrase)) continue;
    let changed = false;
    const target = [...phrase].map((character, index) => {
      const syllable = syllables[index];
      if (character !== "不") return syllable;
      if (syllable === BU_RISING || syllable === BU_NEUTRAL) {
        changed = true;
        return BU_FALLING;
      }
      return syllable;
    });
    if (!changed) continue;
    const key = `${target.join("")}\t${phrase}`;
    if (exact.has(key)) continue;
    supplements.add(key);
  }

  const body = [...supplements].sort().map((key) => `${key}\t${TAGS}`);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, body.join("\n") + "\n", "utf8");
  console.error(`wrote ${body.length} rows to ${out}`);
}

main();
