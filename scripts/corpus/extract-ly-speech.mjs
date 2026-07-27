#!/usr/bin/env node
// 從 fetch-ly-gazette.mjs 下載的公報純文字檔中，只萃取「詢答口語」段落。
//
// 立法院公報一份文件通常混雜四種語域，其中只有一種對輸入法有用：
//   1. 議事錄       — 出席名單、時間地點，全形空格排版，無句子（丟棄）
//   2. 預算決議     — 「爰凍結該項預算50萬元，俟…始得動支」公文體（丟棄）
//   3. 專案書面報告 — 壹、貳、參編號的講稿本文（丟棄）
//   4. 詢答對話     — 委員與官員的即席一問一答，真正的口語（保留）
//
// 判準：只有「發言人：內容」這種行才是詢答。書面段落一律沒有發言人前綴，
// 所以這條規則同時擋掉 1~3，不需要另外做段落分類。
//
// 用法：
//   node scripts/corpus/extract-ly-speech.mjs --input tmp/ly-gazette/docs --output tmp/ly-speech.txt
//
// 輸出：一行一個發言輪次，可直接餵給
//   cargo run --bin chiakey-lexicon -- build-bigram-stats --input tmp/ly-speech.txt \
//     --lexicon normalized/smart-mandarin.tsv --document-boundary line

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";

const DEFAULTS = {
  input: "tmp/ly-gazette/docs",
  output: "tmp/ly-speech.txt",
  stats: "tmp/ly-speech-stats.json",
  sample: "tmp/ly-speech-sample.txt",
  minChars: 4,
  dedupeMinChars: 20,
  shortRepeatCap: 20,
  sampleSize: 300,
};

// 機關名不是發言人。「程序委員會意見：擬請院會將本案交X委員會審查。」是公報的
// 欄位標籤，格式上和發言完全一樣，但內容是套印文字。人名不會含「委員會」——
// 委員的寫法是「陳委員椒華」，主委是「陳主任委員吉仲」。
const NOT_A_PERSON = /委員會|黨團|辦公室|小組|議事處|公報處/;

// 職稱詞。發言人格式是「姓 + 職稱 + 名」，例如 顧部長立雄、沈副主任委員有忠。
const TITLES = [
  "副主任委員", "主任委員", "召集委員", "主任秘書", "專門委員", "政務委員",
  "副秘書長", "秘書長", "副署長", "副司長", "參謀長", "指揮官", "副局長",
  "副處長", "執行長", "總經理", "董事長", "理事長", "檢察長", "發言人",
  "委員", "部長", "次長", "署長", "司長", "處長", "局長", "院長", "廳長",
  "主委", "司令", "總長", "校長", "教授", "縣長", "市長", "科長", "專員",
  "編審", "技正", "參事", "顧問", "大使", "代表", "主席",
];

const SPEAKER = new RegExp(
  `^(主席|[\\u4e00-\\u9fff]{1,2}(?:${TITLES.join("|")})[\\u4e00-\\u9fffA-Za-z．\\s]{0,8})[：:]\\s*(.+)$`,
);

// 公文體 / 表格殘留的標記。命中即丟棄整行。
const FORMAL_MARKERS = [
  /【\d/, /\[image:/i, /照列/, /始得動支/, /爰凍結/, /提案人[：:]/,
  /^（?無）?$/, /決議[：:]/, /決定[：:]/, /審查結果/, /^（[一二三四五六七八九十]+）/,
  /^[壹貳參肆伍陸柒捌玖拾][、，]/,
];

function parseArgs(argv) {
  const cfg = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = () => {
      const value = argv[i + 1];
      if (value === undefined) throw new Error(`missing value for ${arg}`);
      i += 1;
      return value;
    };
    switch (arg) {
      case "--input": cfg.input = next(); break;
      case "--output": cfg.output = next(); break;
      case "--stats": cfg.stats = next(); break;
      case "--sample": cfg.sample = next(); break;
      case "--min-chars": cfg.minChars = Number.parseInt(next(), 10); break;
      case "--dedupe-min-chars": cfg.dedupeMinChars = Number.parseInt(next(), 10); break;
      case "--short-repeat-cap": cfg.shortRepeatCap = Number.parseInt(next(), 10); break;
      case "--help":
      case "-h":
        console.log(
          [
            "extract-ly-speech.mjs — 從公報文字檔萃取詢答口語",
            "",
            "  --input <dir>            公報 txt 目錄，預設 tmp/ly-gazette/docs",
            "  --output <file>          語料輸出，一行一輪發言",
            "  --stats <file>           統計輸出（JSON）",
            "  --sample <file>          抽樣輸出，供人工檢視語域",
            "  --min-chars <n>          發言最少字數，預設 4",
            "  --dedupe-min-chars <n>   超過此長度才做去重，預設 20",
            "",
            "去重只對長發言生效：公報會把上次會議議事錄整份複製進來，屬於真重複；",
            "但「謝謝主席」這類短語的高頻是真實的，去掉會扭曲 bigram 分布。",
          ].join("\n"),
        );
        process.exit(0);
        break;
      default:
        throw new Error(`unknown option: ${arg}`);
    }
  }
  return cfg;
}

function normalise(text) {
  return text
    .replace(/　/g, " ")            // 全形空格是排版用的，不是詞界
    .replace(/（\s*\d+\s*時\s*\d+\s*分\s*）/g, "") // 發言起始時間戳
    .replace(/〔[^〕]*〕/g, "")
    .replace(/【[^】]*】/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

// 名單行：幾乎都是 2~4 字的人名，沒有句末標點。
function looksLikeNameList(text) {
  if (/[。？！]/.test(text)) return false;
  const chunks = text.split(/\s+/).filter(Boolean);
  if (chunks.length < 4) return false;
  const nameish = chunks.filter((c) => /^[一-鿿]{2,4}$/.test(c)).length;
  return nameish / chunks.length > 0.7;
}

async function main() {
  const cfg = parseArgs(process.argv.slice(2));
  const files = (await readdir(cfg.input)).filter((f) => f.endsWith(".txt"));
  if (files.length === 0) throw new Error(`${cfg.input} 裡沒有 .txt，請先跑 fetch-ly-gazette.mjs`);

  const seen = new Map();
  const out = [];
  const speakerCounts = new Map();
  const stats = {
    documents: files.length,
    rawChars: 0,
    rawLines: 0,
    speakerLines: 0,
    droppedFormal: 0,
    droppedNameList: 0,
    droppedShort: 0,
    droppedDuplicate: 0,
    droppedRepeatCap: 0,
    droppedNotPerson: 0,
    keptTurns: 0,
    keptChars: 0,
  };

  for (const file of files) {
    const text = await readFile(join(cfg.input, file), "utf8");
    stats.rawChars += text.length;
    for (const rawLine of text.split(/\r?\n/)) {
      const line = rawLine.trim();
      if (!line) continue;
      stats.rawLines += 1;

      // 議事錄的標籤行用全形空格撐開（「主　　席：」），不是發言。
      const colon = line.search(/[：:]/);
      if (colon > 0 && line.slice(0, colon).includes("　")) continue;

      const match = SPEAKER.exec(line);
      if (!match) continue;
      stats.speakerLines += 1;

      const speaker = match[1].replace(/\s+/g, "");
      if (NOT_A_PERSON.test(speaker)) { stats.droppedNotPerson += 1; continue; }
      const body = normalise(match[2]);

      if (FORMAL_MARKERS.some((re) => re.test(body))) { stats.droppedFormal += 1; continue; }
      if (looksLikeNameList(body)) { stats.droppedNameList += 1; continue; }
      if ([...body].length < cfg.minChars) { stats.droppedShort += 1; continue; }

      // 長發言完全去重（公報會整份複製上次會議議事錄）；短發言保留到上限，
      // 讓「好，謝謝。」這類真實高頻語留下頻率訊號但不至於淹沒語料。
      const key = createHash("sha1").update(body).digest("hex");
      if ([...body].length >= cfg.dedupeMinChars) {
        if (seen.has(key)) { stats.droppedDuplicate += 1; continue; }
        seen.set(key, 1);
      } else {
        const n = seen.get(key) ?? 0;
        if (n >= cfg.shortRepeatCap) { stats.droppedRepeatCap += 1; continue; }
        seen.set(key, n + 1);
      }

      out.push(body);
      stats.keptTurns += 1;
      stats.keptChars += [...body].length;
      speakerCounts.set(speaker, (speakerCounts.get(speaker) ?? 0) + 1);
    }
  }

  stats.retentionByChars = stats.rawChars ? +(stats.keptChars / stats.rawChars).toFixed(4) : 0;
  stats.meanTurnChars = stats.keptTurns ? +(stats.keptChars / stats.keptTurns).toFixed(1) : 0;
  stats.topSpeakers = [...speakerCounts.entries()]
    .sort((a, b) => b[1] - a[1])
    .slice(0, 30)
    .map(([name, count]) => ({ name, count }));

  await writeFile(cfg.output, `${out.join("\n")}\n`, "utf8");
  await writeFile(cfg.stats, JSON.stringify(stats, null, 2), "utf8");

  const step = Math.max(1, Math.floor(out.length / cfg.sampleSize));
  const sample = out.filter((_, i) => i % step === 0).slice(0, cfg.sampleSize);
  await writeFile(cfg.sample, `${sample.join("\n")}\n`, "utf8");

  console.error(
    [
      `文件 ${stats.documents} 份，原始 ${stats.rawChars.toLocaleString()} 字`,
      `保留 ${stats.keptTurns.toLocaleString()} 輪發言、${stats.keptChars.toLocaleString()} 字`,
      `字元保留率 ${(stats.retentionByChars * 100).toFixed(1)}%，平均每輪 ${stats.meanTurnChars} 字`,
      `丟棄：機關標籤 ${stats.droppedNotPerson}、公文體 ${stats.droppedFormal}、名單 ${stats.droppedNameList}、過短 ${stats.droppedShort}、長句重複 ${stats.droppedDuplicate}、短句超額 ${stats.droppedRepeatCap}`,
      `語料 → ${cfg.output}｜統計 → ${cfg.stats}｜抽樣 → ${cfg.sample}`,
    ].join("\n"),
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
