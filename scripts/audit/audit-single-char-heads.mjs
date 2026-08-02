#!/usr/bin/env node
// 檢查單音節讀音上「走訪器會拿到哪個單字」，找出語氣詞／結構助詞輸給同音實詞的位置。
//
// 為什麼要單獨做這件事：audit-unigram-health.mjs 只看多字詞條，而
// Docs/WalkerBaseline.zh-TW.md 的錯誤結構分析指出，最大的幾群字級錯誤全部是
// 單字之間的競爭（`啊`→`阿`、`啦`→`拉`、`嘛`→`嗎`），完全落在那支工具的視野外。
//
// 走訪器怎麼決定排頭（Manjusri/Node.h findHighestScorePair）：
//
//   result = m_unigramCurrents[0];
//
// 而 m_unigramCurrents 由 LanguageModel::findUnigrams() 撈出後跑 stable_sort，
// 依 probability 由高到低。stable_sort 保留輸入順序，輸入順序就是 SQLite 沒有
// ORDER BY 時的 rowid 順序，所以排頭 = `ORDER BY probability DESC, rowid ASC`
// 的第一列。`db::reorder_unigrams` 正是靠重寫實體順序來決定並列時誰在前。
//
// 判準：某個語氣詞／結構助詞在該讀音上存在，但不是排頭。輸出翻轉所需的權重差
// （margin），由小到大排序——並列與微差最該優先看，因為翻轉成本最低。
//
// 這裡刻意不引用 gold set 或任何語料來挑候選：語料只用來事後量測改動效果
// （見 Docs/WalkerBaseline.zh-TW.md），拿它挑候選再拿它驗收就是 in-sample。
// 本工具的輸出是**假設**，要不要調整必須用 gold set 前後比對來決定。
//
// 本工具只讀不寫，不會改動 DB、來源檔或任何權重。
//
// 用法：
//   node scripts/audit/audit-single-char-heads.mjs
//   node scripts/audit/audit-single-char-heads.mjs --out tmp/single-char-heads.tsv
//   node scripts/audit/audit-single-char-heads.mjs --class structural --top 40
//   node scripts/audit/audit-single-char-heads.mjs --ties --top 30

import { DatabaseSync } from "node:sqlite";
import { existsSync, statSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const DEFAULTS = {
  db: "dist/dev/ChiaKeySource-dev.db",
  bin: "target/release/chiakey-lexicon",
  sourcesDir: "sources",
  out: null,
  tiesOut: null,
  cls: null,
  top: 25,
  ties: false,
  // 並列視為 0：權重是 log10 機率，兩列差到小數點後六位以下時，實務上由
  // 實體順序決定勝負（`嘛`／`嗎` 差 8e-6 就是這種情形）。
  tieEpsilon: 1e-5,
  allowStale: false,
};

// 語氣詞是封閉類，位置訊息強（幾乎只出現在句末或句中停頓），unigram 權重壓不住
// 同音實詞時就會整批出錯。結構助詞另外分一類：`的`／`地`／`得` 依 WalkerBaseline
// 的說明要等輸入法本體以聲調區分，bigram 層刻意不處理，不該混進來一起看。
const PARTICLES = new Map([
  ...[..."啊呀哇哪啦嘞咧唄嘛嗎呢吧喔噢哦唷喲耶欸誒囉嘍咯嗯哼哎唉呦嘿"].map((c) => [c, "語氣詞"]),
  ...[..."的地得了著過們"].map((c) => [c, "結構助詞"]),
]);

const CLASSES = ["語氣詞", "結構助詞"];

function parseArgs(argv) {
  const cfg = { ...DEFAULTS };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = () => {
      const v = argv[i + 1];
      if (v === undefined) throw new Error(`missing value for ${arg}`);
      i += 1;
      return v;
    };
    switch (arg) {
      case "--db": cfg.db = next(); break;
      case "--bin": cfg.bin = next(); break;
      case "--sources-dir": cfg.sourcesDir = next(); break;
      case "--out": cfg.out = next(); break;
      case "--ties-out": cfg.tiesOut = next(); break;
      case "--class": cfg.cls = next(); break;
      case "--top": cfg.top = Number.parseInt(next(), 10); break;
      case "--tie-epsilon": cfg.tieEpsilon = Number.parseFloat(next()); break;
      case "--ties": cfg.ties = true; break;
      case "--allow-stale": cfg.allowStale = true; break;
      case "--help":
      case "-h":
        console.log([
          "audit-single-char-heads.mjs — 找出語氣詞／結構助詞輸給同音實詞的讀音",
          "",
          "  --db <path>          release DB，預設 dist/dev/ChiaKeySource-dev.db",
          "  --bin <path>         Rust CLI（qstring 解回注音），預設 target/release/chiakey-lexicon",
          "  --out <file>         輸出完整清單 TSV",
          "  --ties <n>           另外列出所有前二名並列的讀音（排頭由實體順序決定）",
          "  --ties-out <file>    並列清單輸出 TSV",
          `  --class <name>       只看單一類別：${CLASSES.join(" / ")}`,
          "  --top <n>            每類在終端機列出前 n 筆，預設 25",
          "  --tie-epsilon <x>    視為並列的權重差上限，預設 1e-5",
          "  --allow-stale        DB 比來源檔舊時仍繼續（預設直接中止）",
        ].join("\n"));
        process.exit(0);
        break;
      default: throw new Error(`unknown option: ${arg}`);
    }
  }
  if (cfg.cls && !CLASSES.includes(cfg.cls)) throw new Error(`unknown class: ${cfg.cls}`);
  return cfg;
}

const fmt = (x) => (Number.isFinite(x) ? x.toFixed(6) : "");
const isReading = (qstring) =>
  !qstring.startsWith("_punctuation_") && !qstring.startsWith("_ctrl_");

// 與 audit-unigram-health.mjs 同一套判準：只看版控追蹤的檔案，語料暫存不算。
function newestSourceMtime(dir) {
  const listed = spawnSync("git", ["ls-files", "-z", dir], { encoding: "utf8" });
  const files = listed.status === 0 ? listed.stdout.split("\0").filter(Boolean) : [];
  let newest = 0;
  for (const file of files) {
    try {
      newest = Math.max(newest, statSync(file).mtimeMs);
    } catch {
      // 已刪除但尚未 commit 的檔案，略過。
    }
  }
  return newest;
}

function decodeBopomofo(bin, qstrings) {
  const list = Array.from(qstrings);
  const map = new Map();
  if (!list.length) return map;
  if (!existsSync(bin)) {
    console.error(`  （找不到 ${bin}，注音欄位留空。先跑 cargo build --release 可補上。）`);
    return map;
  }
  const result = spawnSync(bin, ["qstring-to-bpmf"], {
    input: list.join("\n") + "\n",
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 128,
  });
  if (result.status !== 0) {
    console.error(result.stderr);
    return map;
  }
  const lines = result.stdout.split("\n");
  for (let i = 0; i < list.length; i += 1) map.set(list[i], lines[i] ?? "");
  return map;
}

function main() {
  const cfg = parseArgs(process.argv.slice(2));
  if (!existsSync(cfg.db)) {
    console.error(`找不到 ${cfg.db}。先跑：cargo run --release -- prepare-release`);
    process.exit(1);
  }
  const dbMtime = statSync(cfg.db).mtimeMs;
  const sourceMtime = newestSourceMtime(cfg.sourcesDir);
  if (sourceMtime > dbMtime) {
    const day = (ms) => new Date(ms).toISOString().slice(0, 16).replace("T", " ");
    console.error(`⚠ ${cfg.db}（${day(dbMtime)}）比 ${cfg.sourcesDir}/ 的最新異動（${day(sourceMtime)}）舊。`);
    console.error("  先跑：cargo run --release -- prepare-release");
    if (!cfg.allowStale) process.exit(1);
    console.error("  （--allow-stale 已指定，繼續。）\n");
  }

  const db = new DatabaseSync(cfg.db, { readOnly: true });
  // 這個排序就是走訪器看到的候選順序，見檔頭說明。
  const rows = db
    .prepare(
      `select qstring, current, probability
       from unigrams
       where current <> '' and length(current) = 1 and length(qstring) = 2
       order by qstring, probability desc, rowid asc`,
    )
    .all();

  const nodes = new Map();
  for (const r of rows) {
    if (!isReading(r.qstring)) continue;
    if (!nodes.has(r.qstring)) nodes.set(r.qstring, []);
    nodes.get(r.qstring).push({ char: r.current, weight: r.probability });
  }

  const findings = [];
  const ties = [];
  for (const [qstring, candidates] of nodes) {
    if (candidates.length < 2) continue;
    const [head, runnerUp] = candidates;

    if (head.weight - runnerUp.weight <= cfg.tieEpsilon) {
      ties.push({ qstring, head, runnerUp, candidates });
    }

    for (let rank = 1; rank < candidates.length; rank += 1) {
      const cand = candidates[rank];
      const cls = PARTICLES.get(cand.char);
      if (!cls) continue;
      findings.push({
        qstring,
        particle: cand.char,
        particleWeight: cand.weight,
        rank: rank + 1,
        head: head.char,
        headWeight: head.weight,
        // 翻轉所需的最小提升。並列時走訪器靠實體順序決勝，需要的是「嚴格大於」，
        // 所以這裡的 0 代表「只要有任何正向差距就會翻轉」。
        margin: head.weight - cand.weight,
        cls,
        candidates,
      });
    }
  }

  const bpmf = decodeBopomofo(
    cfg.bin,
    new Set([...findings.map((f) => f.qstring), ...ties.map((t) => t.qstring)]),
  );

  findings.sort((a, b) => a.margin - b.margin || a.qstring.localeCompare(b.qstring));

  console.log(`=== 單音節讀音的排頭（DB: ${cfg.db}）===`);
  console.log(`  受檢讀音：${nodes.size.toLocaleString()} 個（有兩個以上單字候選者）`);
  console.log(`  語氣詞／結構助詞不是排頭的位置：${findings.length}\n`);

  for (const cls of CLASSES) {
    if (cfg.cls && cfg.cls !== cls) continue;
    const list = findings.filter((f) => f.cls === cls);
    if (!list.length) continue;
    console.log(`=== ${cls}（${list.length} 個位置，依翻轉成本由低到高）===`);
    for (const f of list.slice(0, cfg.top)) {
      const reading = bpmf.get(f.qstring) || f.qstring;
      const tie = f.margin <= cfg.tieEpsilon ? " ← 並列，排頭由實體順序決定" : "";
      const others = f.candidates.slice(0, 3).map((c) => c.char).join("");
      console.log(
        `  ${reading.padEnd(8)} ${f.particle} ${fmt(f.particleWeight)}  輸給 ${f.head} ${fmt(f.headWeight)}` +
        `  差 ${fmt(f.margin)}  第 ${f.rank} 名／${f.candidates.length}  前三:${others}${tie}`,
      );
    }
    if (list.length > cfg.top) console.log(`  …其餘 ${list.length - cfg.top} 個見 --out 輸出。`);
    console.log();
  }

  if (cfg.ties || cfg.tiesOut) {
    console.log(`=== 前二名並列的讀音（${ties.length} 個）===`);
    console.log("  這些位置的排頭完全由 db::reorder_unigrams 寫出的實體順序決定，");
    console.log("  只要有任何正向權重差就能穩定翻轉，是成本最低的調整目標。");
    if (cfg.ties) {
      for (const t of ties.slice(0, cfg.top)) {
        const reading = bpmf.get(t.qstring) || t.qstring;
        console.log(`  ${reading.padEnd(8)} ${t.head.char} / ${t.runnerUp.char}  ${fmt(t.head.weight)}`);
      }
      if (ties.length > cfg.top) console.log(`  …其餘 ${ties.length - cfg.top} 個見 --ties-out 輸出。`);
    }
    console.log();
  }

  console.log("  本工具只稽核、不改任何權重。輸出是假設，要用 gold set 前後比對驗收：");
  console.log("  見 Docs/WalkerBaseline.zh-TW.md。");

  if (cfg.out) {
    const header = "# class\treading\tqstring\tparticle\tparticle_weight\trank\tcandidate_count\thead\thead_weight\tmargin\tcandidates";
    const body = findings
      .filter((f) => !cfg.cls || f.cls === cfg.cls)
      .map((f) => [
        f.cls,
        bpmf.get(f.qstring) ?? "",
        f.qstring,
        f.particle,
        fmt(f.particleWeight),
        f.rank,
        f.candidates.length,
        f.head,
        fmt(f.headWeight),
        fmt(f.margin),
        f.candidates.slice(0, 8).map((c) => c.char).join(""),
      ].join("\t"))
      .join("\n");
    writeFileSync(cfg.out, `${header}\n${body}\n`, "utf8");
    console.log(`\n  完整清單 → ${cfg.out}`);
  }

  if (cfg.tiesOut) {
    const header = "# reading\tqstring\thead\trunner_up\tweight\tcandidate_count";
    const body = ties
      .map((t) => [
        bpmf.get(t.qstring) ?? "",
        t.qstring,
        t.head.char,
        t.runnerUp.char,
        fmt(t.head.weight),
        t.candidates.length,
      ].join("\t"))
      .join("\n");
    writeFileSync(cfg.tiesOut, `${header}\n${body}\n`, "utf8");
    console.log(`  並列讀音清單 → ${cfg.tiesOut}（${ties.length} 個）`);
  }

  db.close();
}

main();
