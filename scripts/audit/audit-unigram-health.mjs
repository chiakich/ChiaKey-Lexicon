#!/usr/bin/env node
// 檢查 release DB 裡每一筆多字 unigram 的「讀音正確性」與「路徑可達性」。
//
// 作法取自唯音輸入法先鋒語料庫的 Collector_HealthCheck.swift，但判準換成
// ChiaKey 走訪器的實際計分規則（見 Docs/WalkerScoring.zh-TW.md）。
//
// 先鋒語料庫的原始三類（Sources/LibVanguardChewingData/SubCodes/Collector/
// Collector_HealthCheck.swift，healthCheckPerMode）：
//
//   faulty       詞的注音對不上單字讀音表           → 讀音錯誤
//   indifferent  詞 == 逐字最佳串接，且贏不了逐字路徑 → 這筆詞條不產生任何效果
//   insufficient 詞 != 逐字最佳串接，但贏不了逐字路徑 → 該升權，否則永遠打不出來
//
// 它比較的是 `sum(單字最佳分) >= 詞分`。ChiaKey 的走訪器多了詞長加分
// （Manjusri/Node.cpp 的 c_phraseLengthBonus = 1.0，每多一個音節 +1.0），
// 所以這裡的比較必須換算成有效分數：
//
//   eff(詞)      = weight + 1.0 × (音節數 − 1)
//   eff(逐字路徑) = Σ 各音節最佳單字 weight        （每個單字節點加分為 0）
//
// 詞條只有在 eff(詞) > eff(逐字路徑) 時才贏得過完全退化的逐字路徑。
//
// 注意這是「最寬鬆的競爭者」：真正的對手是最佳斷詞，不一定是逐字。
// 一筆詞條連逐字路徑都贏不了，就一定贏不了任何更好的斷詞，所以本稽核
// 報出來的都是確定有問題的列，不會誤報；反過來說沒被報出來的列不代表
// 一定會贏（可能輸給某個更強的拆詞路徑），那要另外用完整 walker 檢查。
//
// faulty 這一類在本專案的資料上有九成是「詞層輕聲」（`但是` 的 `是` 讀 ㄕ˙、
// 單字表只收 ㄕˋ），混在一起會蓋掉其他不一致，所以再依注音細分成
// reading-mismatch（聲韻就不對）／tone-mismatch（只有聲調不對）／
// neutral-tone（輕聲變體）三類。完整類別說明見 Docs/Scripts.zh-TW.md。
//
// 要注意這幾類只說「詞層讀音與單字表不一致」，不說哪一邊錯。實測下來多數
// 是單字表缺了台灣實際在用的讀音（`好萊塢` 的 `塢` 唸 ㄨ、`咖哩` 的 `咖` 唸
// ㄍㄚ，單字表都只有另一個音），而不是詞標錯。所以本工具把這些列再依
// (字, 缺的讀音) 聚合成「單字表可能缺的讀音」清單，附上佐證詞數——那才是
// 可以直接拿去補 reading-supplements 的形式。缺單字讀音的影響也不只在這些
// 詞：該字會無法單獨以這個音輸入，其他需要同一個音的詞也一樣打不出來。
//
// 本工具只讀不寫，不會改動 DB、來源檔或任何權重。
//
// 用法：
//   node scripts/audit/audit-unigram-health.mjs
//   node scripts/audit/audit-unigram-health.mjs --out tmp/unigram-health.tsv
//   node scripts/audit/audit-unigram-health.mjs --only insufficient --top 40

import { DatabaseSync } from "node:sqlite";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const DEFAULTS = {
  db: "dist/dev/ChiaKeySource-dev.db",
  bin: "target/release/chiakey-lexicon",
  normalized: "normalized/smart-mandarin.tsv",
  sourcesDir: "sources",
  allowStale: false,
  out: null,
  missingOut: null,
  only: null,
  top: 20,
  // 走訪器每多一個音節加 1.0（Manjusri/Node.cpp c_phraseLengthBonus）。
  lengthBonus: 1.0,
  // 低於此權重的列視為地板值，先鋒語料庫同樣會先濾掉（其閾值為 -14）。
  minWeight: -90,
};

const CATEGORIES = [
  ["reading-mismatch", "mismatch", "讀音不一致：該位置的聲韻組合不在該字已知讀音裡"],
  ["tone-mismatch", "tone", "讀音不一致：聲韻相同但聲調不在該字已知讀音裡（非輕聲）"],
  ["abbreviated-reading", "abbrev", "簡拼列：該列含不可能單獨成音節的裸聲母（ㄅㄆㄇㄈ…），非真讀音"],
  ["neutral-tone", "neutral", "輕聲變體：該位置是輕聲，單字表只收了本調（詞層輕聲的常見情形）"],
  ["reading-unknown", "unknown", "讀音無從查證：某字在詞庫裡沒有任何單字列"],
  ["length-mismatch", "lenDiff", "音節數與字數不符：無法逐字比對讀音（合音、兒化、符號等）"],
  ["indifferent", "indiff", "無作用：等同逐字最佳串接，且贏不了逐字路徑"],
  ["insufficient", "insuff", "權重不足：寫法與逐字最佳串接不同，但贏不了逐字路徑"],
  ["capped", "capped", "刻意降權：來源標記為 cap／demote，贏不了逐字路徑正是預期結果"],
];

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
      case "--normalized": cfg.normalized = next(); break;
      case "--sources-dir": cfg.sourcesDir = next(); break;
      case "--allow-stale": cfg.allowStale = true; break;
      case "--out": cfg.out = next(); break;
      case "--missing-out": cfg.missingOut = next(); break;
      case "--only": cfg.only = next(); break;
      case "--top": cfg.top = Number.parseInt(next(), 10); break;
      case "--length-bonus": cfg.lengthBonus = Number.parseFloat(next()); break;
      case "--min-weight": cfg.minWeight = Number.parseFloat(next()); break;
      case "--help":
      case "-h":
        console.log([
          "audit-unigram-health.mjs — 依 ChiaKey 走訪器檢查多字 unigram 的讀音與可達性",
          "",
          "  --db <path>          release DB，預設 dist/dev/ChiaKeySource-dev.db",
          "  --bin <path>         Rust CLI（用來把 qstring 解回注音），預設 target/release/chiakey-lexicon",
          "  --normalized <path>  回溯來源層，預設 normalized/smart-mandarin.tsv",
          "  --sources-dir <dir>   檢查 DB 是否過期的比對對象，預設 sources",
          "  --allow-stale        DB 比來源檔舊時仍繼續（預設直接中止）",
          "  --out <file>         輸出完整清單 TSV",
          "  --missing-out <file> 輸出「單字表可能缺的讀音」清單 TSV（補 reading-supplements 用）",
          `  --only <category>    只列出單一類別：${CATEGORIES.map(([key]) => key).join(" / ")}`,
          "  --top <n>            每類在終端機列出前 n 筆，預設 20",
          "  --length-bonus <x>   詞長加分，預設 1.0（對應 c_phraseLengthBonus）",
          "  --min-weight <x>     低於此權重的列視為地板值而略過，預設 -90",
        ].join("\n"));
        process.exit(0);
        break;
      default: throw new Error(`unknown option: ${arg}`);
    }
  }
  if (cfg.only && !CATEGORIES.some(([key]) => key === cfg.only)) {
    throw new Error(`unknown category: ${cfg.only}`);
  }
  return cfg;
}

const pct = (a, b) => (b === 0 ? "0.0" : ((a / b) * 100).toFixed(1));
const fmt = (x) => (Number.isFinite(x) ? x.toFixed(3) : "");

// `_` 本身是合法的 qstring 字元（48 + order，見 src/phonetics.rs），所以不能用
// 「開頭是底線」來濾掉非讀音列。真正的保留命名空間只有這兩個，判準與 src/db.rs
// 的 load_primary_character_readings 一致。
const isReading = (qstring) =>
  !qstring.startsWith("_punctuation_") && !qstring.startsWith("_ctrl_");

const UNATTRIBUTED = "(未對應到來源層)";

// normalized/smart-mandarin.tsv 是 prepare-release 產生的稽核視圖，不進版控；
// 沒有它也能跑，只是報表的來源欄會全部落在 UNATTRIBUTED。
function loadSourceMap(path) {
  const map = new Map();
  if (!existsSync(path)) return map;
  for (const line of readFileSync(path, "utf8").split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const f = line.split("\t");
    if (f.length < 5) continue;
    // tags 形如 `unigram,reading-supplements,supplemental-reading,...`，
    // 第一個非 `unigram` 的片段就是資料層名稱。
    const layer = f[4].split(",").find((tag) => tag && tag !== "unigram") ?? f[3];
    map.set(`${f[0]}\t${f[1]}`, { layer, tags: f[4] });
  }
  return map;
}

// 這些 tag 代表該列是被刻意壓下去的（`compositional-cap` 來自 669e85a，把 essay
// 的組合式條目壓到讓 bigram 層有機會介入；`fragment-demote` 來自碎片降權層）。
// 這種列贏不了逐字路徑正是設計意圖，不該和「不小心權重不足」混在一起看。
const DEMOTION_TAGS = new Set(["compositional-cap", "fragment-demote", "demote"]);
const isDemoted = (tags) => tags.split(",").some((tag) => DEMOTION_TAGS.has(tag));

// `alt-reading` / `common-mistype` 是刻意收的常見誤讀，讓打錯的人也找得到詞
// （`絢麗` 收 ㄒㄩㄣˋ、`蛤蜊` 收 ㄍㄜˇ）。它們對不上單字讀音表是預期結果。
// 更要緊的是：誤讀值得收成詞條，不代表值得提升成單字讀音——那會讓該讀音的
// 候選清單跳出這個字。所以 --missing-out 會把佐證來源一併帶出來，全部佐證
// 都來自誤讀列的那幾組要另外決定，不能跟真正漏收的讀音混在一起。
const ALT_READING_TAGS = new Set(["alt-reading", "common-mistype"]);
const isAltReading = (tags) => tags.split(",").some((tag) => ALT_READING_TAGS.has(tag));

// KeyKey boneyard 夾帶了一批以注音縮寫當 qstring 的列（`雅虎奇摩輸入法` 整串都是
// 單一注音符號、`弭棹` 的 `弭` 寫成 ㄇˇ）。判準是「去聲調後是某個已知讀音的前綴
// 或後綴、且更短」——那是截斷而不是另一個音。這種列不該補進單字表：它會讓該
// 縮寫讀音的候選清單跳出這個字。
// 簡拼的判準是「這個 cell 根本不可能是一個音節」。ㄅㄆㄇㄈ… 這組聲母不能單獨成
// 音節，只會出現在簡拼列（`雅虎奇摩輸入法` 的 ㄏ ㄑ ㄇ ㄈ）。
//
// 不能改用「單一注音符號」當判準：ㄓㄔㄕㄖㄗㄘㄙ 的空韻（`試試` 的 ㄕ˙、`姊姊`
// 的 ㄗ˙）與單獨的韻母（`二` ㄦˋ、`五` ㄨˇ）都是完整音節。也不能只看「是某個
// 已知讀音的截斷」，那會誤傷剛好是截斷形的常見誤讀（`鍥而不捨` 的 ㄑㄧˋ）與台語
// 借音（`哭枵` 的 ㄧㄠ）——那些是該收的。
const BARE_INITIALS = new Set([..."ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒ"]);
const isImpossibleSyllable = (bpmf) => BARE_INITIALS.has(stripTone(bpmf));

// 去掉注音末尾的聲調記號（一聲本來就沒有記號），用來比對聲韻是否相同。
const stripTone = (bpmf) => bpmf.replace(/[ˊˇˋ˙]$/, "");

// qstring 是每個音節固定兩個字元（見 src/phonetics.rs absolute_order_string）。
function syllables(qstring) {
  const out = [];
  for (let i = 0; i < qstring.length; i += 2) out.push(qstring.slice(i, i + 2));
  return out;
}

// 讀音只在報表裡當人類可讀欄位用，拿不到 Rust CLI 就留空，不影響判定。
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
    maxBuffer: 1024 * 1024 * 512,
  });
  if (result.status !== 0) {
    console.error(result.stderr);
    console.error("  （qstring-to-bpmf 失敗，注音欄位留空。）");
    return map;
  }
  const lines = result.stdout.split("\n");
  for (let i = 0; i < list.length; i += 1) map.set(list[i], lines[i] ?? "");
  return map;
}

// DB 比來源檔舊的話，稽核報的是上一版的狀況——已經修好的東西會被重報一次。
// 這在實務上真的發生過（`丼` 的讀音已補進 reading-supplements，稽核仍說它缺），
// 所以這裡直接擋在最前面提醒。
function newestSourceMtime(dir) {
  // 只看版控追蹤的檔案。`sources/*/raw/` 與 `pipeline/data/` 底下是語料暫存
  // （動輒數百 MB、不進版控、重跑就變動），拿它們當基準會一直誤報；而真正的
  // 輸入檔（含 `sources/rime-essay/raw/essay.txt` 這種也放在 raw/ 的）都有進版控。
  const listed = spawnSync("git", ["ls-files", "-z", dir], { encoding: "utf8" });
  const files = listed.status === 0
    ? listed.stdout.split("\0").filter(Boolean)
    : [];
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
    console.error("  報表會是上一版的狀況，已經修好的項目仍會被列出。");
    console.error("  先跑：cargo run --release -- prepare-release");
    if (!cfg.allowStale) process.exit(1);
    console.error("  （--allow-stale 已指定，繼續。）\n");
  }
  const db = new DatabaseSync(cfg.db, { readOnly: true });

  const rows = db
    .prepare("select qstring, current, probability, backoff from unigrams where current <> ''")
    .all();

  // 走訪器的 unigram path 是 `unigram[0] + backoff(previous)`，整句分數沿路累加
  // （Graph.h `ssp.second + node.lengthPrior() + nextPath[0].score`），所以一條
  // 路徑有幾個節點就累加幾次 backoff。展開整詞與逐字兩條路徑：
  //
  //   整詞 W 前接 P：  weight(W) + backoff(P) + 1.0×(N−1)
  //   逐字 c1..cN 前接 P：Σ weight(ci) + backoff(P) + backoff(c1) + … + backoff(c_{N−1})
  //
  // `backoff(P)` 兩邊各出現剛好一次而相消——span 外面接什麼不影響勝負，所以
  // 這個比較不需要句子脈絡也能精確判定。span 內部的 backoff(c1..c_{N−1}) 則直接
  // 查得到：Graph.h `addUnigramBackoffs(findUnigrams(pnode.queryString()))` 是拿
  // 前一個節點的候選來建表、以詞面為 key，取的正是那個詞自己那列的 backoff。
  //
  // 目前 backoff 欄全為 0，這幾項都消失，結果與不考慮 backoff 時相同。
  // ---- 單字表：每個音節的最佳單字、每個字的已知讀音集合 ------------------
  const bestChar = new Map(); // 音節 -> { phrase, weight }
  const charReadings = new Map(); // 字 -> Set(音節)
  for (const r of rows) {
    if (!isReading(r.qstring)) continue;
    if ([...r.current].length !== 1 || r.qstring.length !== 2) continue;
    if (!charReadings.has(r.current)) charReadings.set(r.current, new Set());
    charReadings.get(r.current).add(r.qstring);
    const cur = bestChar.get(r.qstring);
    // 同分時取字面較小者，讓報表輸出穩定；走訪器實際的並列排頭是未定義的
    // （見 Docs/WalkerScoring.zh-TW.md〈排序前提〉）。
    if (!cur || r.probability > cur.weight || (r.probability === cur.weight && r.current < cur.phrase)) {
      bestChar.set(r.qstring, {
        phrase: r.current,
        weight: r.probability,
        backoff: Number(r.backoff) || 0,
      });
    }
  }

  // ---- 同一 (qstring, 詞) 的最佳權重，用來標出「同讀音的競爭寫法」--------
  const bestAt = new Map();
  for (const r of rows) {
    const k = `${r.qstring}\t${r.current}`;
    const cur = bestAt.get(k);
    if (cur === undefined || r.probability > cur) bestAt.set(k, r.probability);
  }

  const findings = [];
  let audited = 0;
  let healthy = 0;
  let skippedFloor = 0;
  let skippedNoPath = 0;

  for (const r of rows) {
    if (!isReading(r.qstring)) continue;
    const chars = [...r.current];
    if (chars.length < 2) continue;
    if (r.qstring.length % 2 !== 0) continue;
    if (r.probability <= cfg.minWeight) { skippedFloor += 1; continue; }

    audited += 1;
    const cells = syllables(r.qstring);

    // ---- 讀音檢查：只有音節數 == 字數時才能逐字對位 ---------------------
    if (cells.length !== chars.length) {
      findings.push({
        category: "length-mismatch",
        qstring: r.qstring,
        phrase: r.current,
        weight: r.probability,
        effective: r.probability + cfg.lengthBonus * (cells.length - 1),
        detail: `${cells.length} 音節 / ${chars.length} 字`,
      });
      continue;
    }

    let readingFault = null;
    for (let i = 0; i < chars.length; i += 1) {
      const known = charReadings.get(chars[i]);
      if (!known) {
        readingFault = { category: "reading-unknown", detail: `第 ${i + 1} 字「${chars[i]}」在詞庫中沒有單字列` };
        break;
      }
      if (!known.has(cells[i])) {
        readingFault = {
          category: "reading-mismatch",
          position: i + 1,
          char: chars[i],
          cell: cells[i],
          cells,
          known: [...known],
        };
        break;
      }
    }
    if (readingFault) {
      findings.push({
        ...readingFault,
        qstring: r.qstring,
        phrase: r.current,
        weight: r.probability,
        effective: r.probability + cfg.lengthBonus * (cells.length - 1),
      });
      // 讀音本身有問題時，路徑比較的基準不可信，跟先鋒語料庫一樣跳過。
      continue;
    }

    // ---- 路徑競爭：整詞 vs 完全退化的逐字路徑 ---------------------------
    const perChar = cells.map((cell) => bestChar.get(cell));
    if (perChar.some((x) => x === undefined)) { skippedNoPath += 1; continue; }

    const effective = r.probability + cfg.lengthBonus * (cells.length - 1);
    // 最後一個字不貢獻 backoff：它沒有後續節點會把它當 previous 查。
    const rivalWeight = perChar.reduce(
      (sum, x, i) => sum + x.weight + (i < perChar.length - 1 ? x.backoff : 0),
      0,
    );
    if (effective > rivalWeight) { healthy += 1; continue; }

    const joined = perChar.map((x) => x.phrase).join("");
    const category = joined === r.current ? "indifferent" : "insufficient";
    findings.push({
      category,
      qstring: r.qstring,
      phrase: r.current,
      weight: r.probability,
      effective,
      rival: joined,
      rivalWeight,
      deficit: rivalWeight - effective,
      // 同讀音下若逐字串接本身也是詞、且權重更高，代表這是兩種寫法在打架。
      detail: (() => {
        const alt = bestAt.get(`${r.qstring}\t${joined}`);
        return alt !== undefined && alt > r.probability
          ? `同讀音競爭寫法「${joined}」weight=${fmt(alt)} 高於本列`
          : "";
      })(),
    });
  }

  // ---- 注音解碼：報表欄位，同時用來把輕聲變體從讀音錯誤裡分出來 ----------
  const wanted = new Set();
  for (const f of findings) {
    wanted.add(f.qstring);
    if (f.cell) {
      for (const cell of f.cells) wanted.add(cell);
      wanted.add(f.cell);
      for (const cell of f.known) wanted.add(cell);
    }
  }
  const bpmf = decodeBopomofo(cfg.bin, wanted);

  // 讀音對不上的原因分三層，嚴重度差很多，混在一起看不出該修哪些：
  //   輕聲變體 「但是 ㄉㄢˋ ㄕ˙」— 詞層輕聲，單字表通常只收本調，屬預期行為。
  //   聲調不符 「油脂 ㄧㄡˊ ㄓˇ」— 聲韻對得上，不是詞讀音錯就是單字缺該調。
  //   聲韻不符 「雅虎 ㄚ ㄏ」    — 完全不同的音，多半是簡拼列或借音寫法。
  // 這個細分需要注音，拿不到 Rust CLI 時全部維持在 reading-mismatch。
  for (const f of findings) {
    if (f.category !== "reading-mismatch") continue;
    const cell = bpmf.get(f.cell);
    if (cell) {
      const known = f.known.map((k) => bpmf.get(k)).filter(Boolean);
      const bare = stripTone(cell);
      if (f.cells.some((c) => {
        const decoded = bpmf.get(c);
        return decoded && isImpossibleSyllable(decoded);
      })) {
        f.category = "abbreviated-reading";
      } else if (known.some((k) => stripTone(k) === bare)) {
        f.category = cell.endsWith("˙") ? "neutral-tone" : "tone-mismatch";
      }
    }
    f.detail = `第 ${f.position} 字「${f.char}」的讀音 ${cell || f.cell} 不在其已知讀音（${f.known.map((k) => bpmf.get(k) || k).join(" ")}）`;
  }

  // 來源層要在統計之前先掛上：capped 的判定靠來源 tag，會改掉分類。
  const sourceMap = loadSourceMap(cfg.normalized);
  for (const f of findings) {
    const entry = sourceMap.get(`${f.qstring}\t${f.phrase}`);
    f.source = entry?.layer ?? UNATTRIBUTED;
    f.isAltReading = entry ? isAltReading(entry.tags) : false;
    if (entry && isDemoted(entry.tags)
      && (f.category === "indifferent" || f.category === "insufficient")) {
      f.category = "capped";
    }
  }

  const buckets = Object.fromEntries(CATEGORIES.map(([key]) => [key, 0]));
  for (const f of findings) buckets[f.category] += 1;
  const reported = findings.length;
  console.log(`=== 多字 unigram 健康度（DB: ${cfg.db}）===`);
  console.log(`  受檢列數（>= 2 字、非保留命名空間、weight > ${cfg.minWeight}）：${audited.toLocaleString()}`);
  console.log(`  略過：地板權重 ${skippedFloor.toLocaleString()} 列、逐字路徑不存在 ${skippedNoPath.toLocaleString()} 列`);
  console.log(`  通過：${healthy.toLocaleString()} 列（${pct(healthy, audited)}%）\n`);

  const width = Math.max(...CATEGORIES.map(([key]) => key.length));
  for (const [key, , label] of CATEGORIES) {
    console.log(`  ${key.padEnd(width)}  ${String(buckets[key]).padStart(8)}  ${pct(buckets[key], audited)}%  ${label}`);
  }
  console.log(`\n  合計報出 ${reported.toLocaleString()} 列（${pct(reported, audited)}%）。本工具只稽核、不改任何權重。`);

  // ---- 各來源層：哪一層該負責處理這些列 --------------------------------
  if (!sourceMap.size) {
    console.log(`  （找不到 ${cfg.normalized}，來源欄留空。跑一次 prepare-release 可補上。）`);
  } else {
    const perSource = new Map();
    for (const f of findings) {
      if (!perSource.has(f.source)) {
        perSource.set(f.source, Object.fromEntries([...CATEGORIES.map(([key]) => [key, 0]), ["total", 0]]));
      }
      const s = perSource.get(f.source);
      s[f.category] += 1;
      s.total += 1;
    }
    const nameWidth = Math.max(12, ...[...perSource.keys()].map((k) => k.length));
    console.log("\n=== 各來源層 ===");
    console.log(`  ${"來源層".padEnd(nameWidth)}${"總數".padStart(8)}${CATEGORIES.map(([, short]) => short.padStart(10)).join("")}`);
    for (const [name, s] of [...perSource.entries()].sort((a, b) => b[1].total - a[1].total)) {
      console.log(`  ${name.padEnd(nameWidth)}${String(s.total).padStart(8)}${CATEGORIES.map(([key]) => String(s[key]).padStart(10)).join("")}`);
    }
  }

  for (const [key, , label] of CATEGORIES) {
    if (cfg.only && cfg.only !== key) continue;
    const list = findings.filter((f) => f.category === key);
    if (!list.length) continue;
    // 路徑類依落後幅度排序（最該處理的在前），讀音類依權重由高到低。
    list.sort((a, b) =>
      a.deficit !== undefined && b.deficit !== undefined ? b.deficit - a.deficit : b.weight - a.weight);
    console.log(`\n=== ${key}（${list.length.toLocaleString()} 列）${label} ===`);
    for (const f of list.slice(0, cfg.top)) {
      const reading = bpmf.get(f.qstring) || f.qstring;
      const tail = f.rival !== undefined
        ? `輸給逐字「${f.rival}」${fmt(f.rivalWeight)}，差 ${fmt(f.deficit)}${f.detail ? ` — ${f.detail}` : ""}`
        : f.detail;
      console.log(`  ${f.phrase}  ${reading}  weight=${fmt(f.weight)} eff=${fmt(f.effective)}  [${f.source}]  ${tail}`);
    }
    if (list.length > cfg.top) console.log(`  …其餘 ${(list.length - cfg.top).toLocaleString()} 列見 --out 輸出。`);
  }

  // ---- 單字表可能缺的讀音：把讀音不一致的列依 (字, 缺的讀音) 聚合 ---------
  // 這幾類只說詞層與單字表不一致、不說哪邊錯，但同一個 (字, 讀音) 被越多詞
  // 佐證，就越可能是單字表漏收而不是那些詞全標錯。輕聲不列入：詞層輕聲本來
  // 就不該回頭補成單字讀音。
  const missing = new Map();
  for (const f of findings) {
    if (f.category !== "reading-mismatch" && f.category !== "tone-mismatch") continue;
    const key = `${f.char}\t${f.cell}`;
    if (!missing.has(key)) {
      missing.set(key, {
        char: f.char,
        cell: f.cell,
        reading: bpmf.get(f.cell) || f.cell,
        known: f.known.map((k) => bpmf.get(k) || k),
        phrases: [],
        sources: new Set(),
        altOnly: true,
      });
    }
    const group = missing.get(key);
    group.phrases.push(f.phrase);
    group.sources.add(f.source);
    if (!f.isAltReading) group.altOnly = false;
  }
  const missingList = [...missing.values()].sort(
    (a, b) => b.phrases.length - a.phrases.length || a.char.localeCompare(b.char),
  );
  if (missingList.length) {
    console.log(`\n=== 單字表可能缺的讀音（${missingList.length} 組）===`);
    console.log("  同一組被越多詞佐證，越可能是單字表漏收台灣實際讀音，而不是那些詞標錯。");
    for (const m of missingList.slice(0, cfg.top)) {
      const examples = [...new Set(m.phrases)].slice(0, 4).join("、");
      const flag = m.altOnly ? " ⚑僅誤讀列佐證" : "";
      console.log(`  ${String(m.phrases.length).padStart(3)} 詞  ${m.char} ${m.reading}  (已知 ${m.known.join(" ")})  例：${examples}${flag}`);
    }
    if (missingList.length > cfg.top) {
      console.log(`  …其餘 ${missingList.length - cfg.top} 組見 --missing-out 輸出。`);
    }
  }

  if (cfg.missingOut) {
    const header = "# char\tmissing_reading\tmissing_qstring\tknown_readings\tphrase_count\talt_reading_only\tsources\tphrases";
    const body = missingList
      .map((m) => [
        m.char,
        m.reading,
        m.cell,
        m.known.join(" "),
        m.phrases.length,
        m.altOnly ? "1" : "0",
        [...m.sources].sort().join(" "),
        [...new Set(m.phrases)].join(" "),
      ].join("\t"))
      .join("\n");
    writeFileSync(cfg.missingOut, `${header}\n${body}\n`, "utf8");
    console.log(`\n  可能缺的單字讀音 → ${cfg.missingOut}（${missingList.length} 組）`);
  }

  if (cfg.out) {
    const header = "# category\tsource\tqstring\tbopomofo\tphrase\tweight\teffective\trival\trival_weight\tdeficit\tdetail";
    const body = findings
      .filter((f) => !cfg.only || f.category === cfg.only)
      .sort((a, b) =>
        a.category === b.category
          ? (b.deficit ?? -Infinity) - (a.deficit ?? -Infinity) || b.weight - a.weight
          : a.category.localeCompare(b.category))
      .map((f) => [
        f.category,
        f.source,
        f.qstring,
        bpmf.get(f.qstring) ?? "",
        f.phrase,
        fmt(f.weight),
        fmt(f.effective),
        f.rival ?? "",
        f.rivalWeight === undefined ? "" : fmt(f.rivalWeight),
        f.deficit === undefined ? "" : fmt(f.deficit),
        f.detail ?? "",
      ].join("\t"))
      .join("\n");
    writeFileSync(cfg.out, `${header}\n${body}\n`, "utf8");
    console.log(`\n  完整清單 → ${cfg.out}`);
  }

  db.close();
}

main();
