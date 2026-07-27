#!/usr/bin/env node
// 把 build-bigram-stats 的輸出整理成可進 sources/ 的 bigram 覆蓋層。
//
// 為什麼需要這一層：
//   1. build-bigram-stats 的 --probability 是常數（預設 -0.1），所有列拿到相同數值。
//      release 的 calibrate_bigram_boost 算的是 (raw - raw_max)，raw 全都一樣就等於
//      整批平權，出現 5000 次的搭配和出現 3 次的沒有區別。
//   2. 立法院語料的高頻搭配集中在議事語域（本席、審查會、條文+通過、邱+委員），
//      那是這份語料最強的訊號，卻是對一般使用者最沒用的部分。
//
// 機率怎麼給：
//   stats 檔只含 selected 的列，沒有 redundant／excluded，所以同一 previous 的
//   count 總和不是真正的邊際分佈，算不出 P(current|previous)。這裡改用 doc_count
//   做單調映射——doc_count 比 count 穩健（同一篇裡刷 50 次不等於 50 篇都出現）。
//   輸出值是「來源內部的強度序」，不是條件機率；calibration 只吃相對差距，夠用。
//
//   raw = CEIL - SPAN * log(maxDoc / doc) / log(maxDoc / minDoc)
//
//   SPAN 預設 1.5，對齊 SYNTHETIC/COMMONVOICE_BIGRAM_BOOST 的作用窗寬度，避免
//   任何列單純因為校準被擠到 unigram floor 以下（見 Docs/WalkerScoring.zh-TW.md
//   的 B 類）。
//
// 用法：
//   node scripts/corpus/postprocess-ly-bigrams.mjs
//   node scripts/corpus/postprocess-ly-bigrams.mjs --min-doc-count 20 --span 1.8

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";

const cfg = {
  stats: "tmp/ly-bigrams-stats.tsv",
  out: "tmp/ly-bigrams-overlay.tsv",
  review: "tmp/ly-bigrams-override-review.tsv",
  // build-bigram-stats --review 的輸出，用來把真實例句帶進複核表
  examples: "tmp/ly-bigrams-review.tsv",
  // 複核後的決定檔：把 review 表第一欄填 keep/drop 再傳回來即可
  decisions: null,
  db: "dist/dev/ChiaKeySource-dev.db",
  minDocCount: 10,
  span: 1.5,
  ceil: -0.2,
  // 單字 current 且 unigram 權重低於此值，多半是被偷走一個音節的碎片
  // （例：「謝謝召委」被切成 謝謝+召，召 = -1.56；而 條 -1.00、署 -0.93、
  // 部 -0.97 這些正常的單字用法都在門檻之上）。
  fragmentFloor: -1.2,
  // 語料只有文字沒有讀音。current 若有多個讀音且權重差在此值內，
  // build-bigram-stats 挑到哪個是任意的（一行 的兩個讀音權重完全相同）。
  // 開啟後直接丟棄這類列，而不是留到複核表。
  dropDubiousReading: false,
  dubiousMargin: 0.1,
};
for (let i = 2; i < process.argv.length; i += 1) {
  const a = process.argv[i];
  const v = () => process.argv[i + 1];
  if (a === "--stats") cfg.stats = v();
  else if (a === "--out") cfg.out = v();
  else if (a === "--review") cfg.review = v();
  else if (a === "--db") cfg.db = v();
  else if (a === "--min-doc-count") cfg.minDocCount = Number.parseInt(v(), 10);
  else if (a === "--span") cfg.span = Number.parseFloat(v());
  else if (a === "--ceil") cfg.ceil = Number.parseFloat(v());
  else if (a === "--fragment-floor") cfg.fragmentFloor = Number.parseFloat(v());
  else if (a === "--examples") cfg.examples = v();
  else if (a === "--decisions") cfg.decisions = v();
  else if (a === "--drop-dubious-reading") cfg.dropDubiousReading = true;
  else if (a === "--dubious-margin") cfg.dubiousMargin = Number.parseFloat(v());
  else if (a === "--help" || a === "-h") {
    console.log([
      "postprocess-ly-bigrams.mjs — 把 bigram 統計整理成 source 覆蓋層",
      "",
      "  --stats <file>          build-bigram-stats 的 --stats 輸出",
      "  --out <file>            覆蓋層輸出（qstring/previous/current/probability）",
      "  --review <file>         會改寫該讀音預設候選的列，需人工看過",
      "  --min-doc-count <n>     最少出現在幾份文件，預設 10",
      "  --span <f>              來源內部強度的 log 跨距，預設 1.5",
      "  --ceil <f>              最強一列的機率，預設 -0.2",
      "  --fragment-floor <f>    單字 current 的 unigram 權重下限，預設 -1.2",
      "  --examples <file>       build-bigram-stats --review 的輸出，供複核表附例句",
      "  --decisions <file>      複核決定檔（把 review 表第一欄填 keep/drop 後傳回）",
      "  --drop-dubious-reading  直接丟棄 current 有多個近乎同分讀音的列",
      "  --dubious-margin <f>    判定「近乎同分」的讀音權重差上限，預設 0.1",
    ].join("\n"));
    process.exit(0);
  }
}

// 議場專用詞。出現在 previous 或 current 就整列丟棄——一般使用者不會打這些，
// 但它們在這份語料裡是最高頻的搭配，留著會蓋掉正常用法。
const REGISTER_DENY = [
  "本席", "本院", "大院", "貴委員會", "逕付", "二讀", "三讀", "審查會",
  "朝野協商", "議事錄", "召委", "備詢", "宣讀", "修正動議", "散會",
  "公報", "報告事項", "討論事項", "程序委員會", "黨團", "院會",
  "如協商條文", "審查完竣", "提案人", "連署",
  // 「議」系會議詞：同音的「意」系在日常用法遠更常見，語料的偏好會打壞
  // 沒有意義／意識 這些高頻日常詞。無異議、有異議都由 includes 一併命中。
  "異議", "議事", "動議", "宣告", "裁示",
  // 「本案及第X條」的誤切產物。及第 是真詞（權重 -0.93），碎片門檻抓不到，
  // 但現代日常幾乎不用。
  "及第",
  // 「委員」系：逐字稿裡是稱謂與職銜，一般輸入用不到，而且會把 姓+委員、
  // 委員+名 這類搭配灌進來。委員會／召集委員／主任委員 都由 includes 一併命中。
  "委員",
];

// 逐字稿的系統性偏誤，格式為「詞庫預設字→語料選字」。只點名確認過的，
// 其餘同音近形的替換（紋→文、屬→署、按→案、人→仁、佈→布、昇→升）都是
// 正確的修正，要留下來。在→再 與 再→在 兩個方向都保留，那正是 bigram 該做的事。
const BIASED_SUBSTITUTIONS = new Set([
  "台→臺",   // 異體字：校正層的 t2tw 政策已選定 台
  "了→瞭",   // 異體字：了解／瞭解
  "覆→復",   // 異體字：台灣慣用 答覆
  "他→它",   // 議場談法案機關多用「它」，日常輸入以「他」為主
  "嗎→嘛",   // 逐字稿是口語，「嘛」偏高；打字時疑問句的「嗎」遠更常見
  "照→召",   // 「謝謝召委」的碎片
]);

// 數字與序數：公報充滿「第八十七條」「十七條」「第二次會議」，這些搭配對
// 一般輸入沒有價值，而且量大。整個 token 只由數字字元構成就丟棄。
// 千萬／萬一 是由數字字元組成的常用詞，例外放行。
const NUMERAL_CHARS = /^[一二三四五六七八九十百千萬億兆零兩第廿卅0-9０-９]+$/;
const NUMERAL_ALLOW = new Set(["千萬", "萬一", "一一"]);
const isNumeralToken = (s) => NUMERAL_CHARS.test(s) && !NUMERAL_ALLOW.has(s);

// 讀音錯掛的修正。詞庫原本給「數」的 ㄕㄨˇ（|O，數落／數一數）-0.930、
// ㄕㄨˋ（\\_，數字／人數）-1.803，排序與實際用法相反，於是 build-bigram-stats
// 取最高權重時把 預算數／決算數／案件數 全掛到 ㄕㄨˇ 去。
//
// 根本修正已寫進 sources/chiaki-modern-overlay/explicit.tsv（把兩個讀音的權重
// 對調）。這裡的 remap 是給「詞庫已修、但 stats 檔是舊 DB 算的」這段過渡期用的：
// 重新 build 之後 stats 會直接給正確讀音，這條規則就自動失效（比對不到 from）。
const READING_REMAP = new Map([
  ["數", { from: "|O", to: "\\_" }],
]);

// 稱呼句的殘骸。主席說「謝謝鄭委員」「剛才李委員提到」，「委員」被前面的規則
// 濾掉之後，剩下「前詞 + 姓」這種沒有意義的搭配。單字姓氏當 current 一律丟棄；
// 代價是少數真詞（周轉+金）也會被誤傷，但 135 列裡只有 2 列屬於這種。
const SURNAMES = new Set([..."王李張劉陳楊黃趙吳周徐孫馬朱胡郭何高林羅鄭梁謝宋唐許鄧馮韓曹曾彭蕭蔡潘田董袁于余葉蔣杜蘇魏程呂丁沈任姚盧傅鍾姜崔譚廖范汪陸金石戴賈韋夏邱方侯鄒熊孟秦白江閻薛尹段雷黎史龍陶賀顧毛郝龔邵萬錢嚴賴洪武莫孔柯游童溫"]);

// 「謝謝」後面接的一律是稱呼（謝謝吳委員／謝謝副主委／謝謝人事長），
// 沒有一筆是一般語言用法。
const ADDRESS_PREVIOUS = new Set(["謝謝", "感謝"]);

// 「姓 + 職稱」是主席程序性發言的產物（「請邱委員議瑩發言」）。
// 只擋 previous 是單字的情況，這樣「委員+說」之類的正常搭配不受影響。
const TITLES_AS_CURRENT = new Set([
  "委員", "部長", "次長", "署長", "主委", "司長", "處長", "局長",
  "院長", "主任委員", "委員長", "秘書長", "總統", "副總統",
]);

// 讀複核決定。格式就是本腳本產出的 review 表：第一欄填 keep / drop。
function loadDecisions(path) {
  const map = new Map();
  if (!path || !existsSync(path)) return map;
  for (const line of readFileSync(path, "utf8").split(/\r?\n/)) {
    if (!line || line.startsWith("#")) continue;
    const f = line.split("\t");
    if (f.length < 3) continue;
    const d = f[0].trim().toLowerCase();
    if (d === "keep" || d === "drop") map.set(`${f[1]}\t${f[2]}`, d);
  }
  return map;
}

function loadExamples(path) {
  const map = new Map();
  if (!path || !existsSync(path)) return map;
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  const hdr = lines[0].split("\t");
  const ix = Object.fromEntries(hdr.map((h, i) => [h, i]));
  for (let i = 1; i < lines.length; i += 1) {
    const f = lines[i].split("\t");
    if (f.length < hdr.length) continue;
    map.set(`${f[ix.previous]}\t${f[ix.current]}`, f[ix.examples] ?? "");
  }
  return map;
}

function main() {
  if (!existsSync(cfg.stats)) {
    console.error(`找不到 ${cfg.stats}。先跑 build-bigram-stats --stats ${cfg.stats}`);
    process.exit(1);
  }
  const lines = readFileSync(cfg.stats, "utf8").split(/\r?\n/);
  const hdr = lines[0].split("\t");
  const ix = Object.fromEntries(hdr.map((h, i) => [h, i]));

  const rows = [];
  const drop = { notSelected: 0, thin: 0, register: 0, titleAfterSurname: 0, badQstring: 0, reviewedDrop: 0, numeral: 0, address: 0, readingRemapped: 0 };
  const decisions = loadDecisions(cfg.decisions);
  const examples = loadExamples(cfg.examples);
  if (decisions.size) console.log(`套用複核決定 ${decisions.size} 筆（來自 ${cfg.decisions}）`);
  let burst = 0;

  for (let i = 1; i < lines.length; i += 1) {
    if (!lines[i]) continue;
    const f = lines[i].split("\t");
    if (f.length < hdr.length) continue;
    if (f[ix.selected] !== "true") { drop.notSelected += 1; continue; }

    const previous = f[ix.previous];
    const current = f[ix.current];
    const count = Number.parseInt(f[ix.count], 10);
    const docCount = Number.parseInt(f[ix.doc_count], 10);
    const pq = f[ix.previous_qstring];
    const cq = f[ix.current_qstring];

    if (!pq || !cq) { drop.badQstring += 1; continue; }
    if (docCount < cfg.minDocCount) { drop.thin += 1; continue; }
    if (REGISTER_DENY.some((t) => previous.includes(t) || current.includes(t))) {
      drop.register += 1;
      continue;
    }
    if ([...previous].length === 1 && TITLES_AS_CURRENT.has(current)) {
      drop.titleAfterSurname += 1;
      continue;
    }
    if (isNumeralToken(previous) || isNumeralToken(current)) {
      drop.numeral += 1;
      continue;
    }
    if (ADDRESS_PREVIOUS.has(previous) || ([...current].length === 1 && SURNAMES.has(current))) {
      drop.address += 1;
      continue;
    }
    let currentQstring = cq;
    const remap = READING_REMAP.get(current);
    if (remap && cq === remap.from) {
      currentQstring = remap.to;
      drop.readingRemapped += 1;
    }
    const decided = decisions.get(`${previous}\t${current}`);
    if (decided === "drop") { drop.reviewedDrop += 1; continue; }
    if (decided === "keep") {
      // 人工放行：跳過後續所有自動過濾
      rows.push({ qstring: `${pq} ${currentQstring}`, previous, current, count, docCount, approved: true });
      continue;
    }
    if (count > docCount * 3) burst += 1; // 單篇內刷高的訊號，僅計數觀察
    rows.push({ qstring: `${pq} ${currentQstring}`, previous, current, count, docCount });
  }

  if (rows.length === 0) {
    console.error("門檻過嚴，沒有列留下。");
    process.exit(1);
  }

  // doc_count → 強度序。log 比例映射，最高者落在 ceil，最低者落在 ceil - span。
  const maxDoc = Math.max(...rows.map((r) => r.docCount));
  const minDoc = Math.min(...rows.map((r) => r.docCount));
  const denom = Math.log(maxDoc / minDoc) || 1;
  for (const r of rows) {
    r.probability = Number(
      (cfg.ceil - cfg.span * (Math.log(maxDoc / r.docCount) / denom)).toFixed(6),
    );
  }

  writeFileSync(
    cfg.out,
    `# qstring\tprevious\tcurrent\tprobability\n${rows
      .map((r) => `${r.qstring}\t${r.previous}\t${r.current}\t${r.probability}`)
      .join("\n")}\n`,
    "utf8",
  );

  console.log(`來源列 ${lines.length - 1} → 初篩後 ${rows.length}`);
  console.log(`  丟棄：出現文件數不足 ${drop.thin}、議事語域 ${drop.register}、` +
    `姓+職稱 ${drop.titleAfterSurname}、稱呼殘骸 ${drop.address}、讀音改掛 ${drop.readingRemapped}、數字序數 ${drop.numeral}、qstring 缺漏 ${drop.badQstring}、複核判定移除 ${drop.reviewedDrop}`);
  console.log(`  doc_count 範圍 ${minDoc}~${maxDoc}，機率範圍 ${(cfg.ceil - cfg.span).toFixed(2)}~${cfg.ceil}`);
  console.log(`  單篇突刷（count > 3×doc_count）${burst} 列，僅觀察未剔除`);
  console.log(`  覆蓋層 → ${cfg.out}`);

  // ---- 對照 release DB 做 A/B/C/D 分類，並挑出會改寫預設候選的列 ----
  if (!existsSync(cfg.db)) {
    console.log(`\n（找不到 ${cfg.db}，跳過分類）`);
    return;
  }
  const db = new DatabaseSync(cfg.db, { readOnly: true });
  const node = new Map();
  for (const r of db.prepare(
    "select qstring, count(*) n, min(probability) lo, max(probability) hi from unigrams group by qstring",
  ).all()) node.set(r.qstring, r);
  const topPhrase = new Map();
  for (const r of db.prepare("select qstring, current, probability from unigrams").all()) {
    const cur = topPhrase.get(r.qstring);
    if (!cur || r.probability > cur.p) topPhrase.set(r.qstring, { phrase: r.current, p: r.probability });
  }
  const bestByPhrase = new Map();
  for (const r of db.prepare("select current, max(probability) p from unigrams group by current").all()) {
    bestByPhrase.set(r.current, r.p);
  }
  // 破音詞偵測：語料只有文字沒有讀音，build-bigram-stats 得自己挑一個 qstring。
  // 同一個詞若有多個讀音且權重接近，挑到哪個是任意的（例：一行 的 ㄧˋㄏㄤˊ 與
  // ㄧˋㄒㄧㄥˊ 權重完全相同）。掛錯讀音時，輕則永不觸發，重則搶走另一個詞的
  // 預設位置（最後+一行 掛到 ㄧˋㄒㄧㄥˊ，會蓋掉 異形）。
  const readingSpread = new Map();
  for (const r of db.prepare(
    "select current, count(distinct qstring) n, max(probability) hi, min(probability) lo from unigrams group by current",
  ).all()) {
    if (r.n > 1) readingSpread.set(r.current, r.hi - r.lo);
  }
  db.close();

  // 單字碎片過濾：需要 unigram 權重，所以擺在讀完 DB 之後。
  const before = rows.length;
  const kept = rows.filter((r) => {
    if (r.approved) return true;
    if ([...r.current].length !== 1) return true;
    const u = bestByPhrase.get(r.current);
    return u === undefined || u >= cfg.fragmentFloor;
  });
  const droppedFragment = before - kept.length;
  // 讀音存疑：語料提供不了讀音資訊，掛錯時會搶走別的詞的預設位置。
  let droppedDubious = 0;
  if (cfg.dropDubiousReading) {
    const beforeDub = kept.length;
    const k = kept.filter((r) => {
      if (r.approved) return true;
      const sp = readingSpread.get(r.current);
      return sp === undefined || sp > cfg.dubiousMargin;
    });
    droppedDubious = beforeDub - k.length;
    kept.length = 0;
    kept.push(...k);
  }
  // 逐字稿相對於一般輸入的系統性偏誤，按「預設字→語料選字」點名。
  // 用點名而不是權重門檻：實測落差對這件事沒有區辨力（議事/意識 落差 0.76 該砍，
  // 邱泰源/太原 落差 0.87 卻是對的），只有語義能分。
  const beforeSub = kept.length;
  const kept2 = kept.filter((r) => {
    if (r.approved) return true;
    const cq = r.qstring.split(" ").pop();
    const top = topPhrase.get(cq);
    if (!top || top.phrase === r.current) return true;
    const a = [...r.current];
    const b = [...top.phrase];
    if (a.length !== b.length) return true;
    const diffs = [];
    a.forEach((ch, j) => { if (ch !== b[j]) diffs.push(`${b[j]}→${ch}`); });
    return !(diffs.length === 1 && BIASED_SUBSTITUTIONS.has(diffs[0]));
  });
  const droppedSub = beforeSub - kept2.length;
  kept.length = 0;
  kept.push(...kept2);
  rows.length = 0;
  rows.push(...kept);
  rows.sort((a, b) => b.probability - a.probability || a.qstring.localeCompare(b.qstring));
  writeFileSync(
    cfg.out,
    `# qstring\tprevious\tcurrent\tprobability\n${rows
      .map((r) => `${r.qstring}\t${r.previous}\t${r.current}\t${r.probability}`)
      .join("\n")}\n`,
    "utf8",
  );
  console.log(`  再丟棄單字碎片（權重 < ${cfg.fragmentFloor}）${droppedFragment} 列、` +
    `偏誤字元替換 ${droppedSub} 列` +
    (cfg.dropDubiousReading ? `、讀音存疑 ${droppedDubious} 列` : "") +
    ` → 最終 ${rows.length} 列`);

  // 群組第一名（A 類判定）
  const head = new Map();
  for (const r of rows) {
    const k = `${r.qstring} ${r.previous}`;
    const c = head.get(k);
    if (!c || r.probability > c.probability) head.set(k, r);
  }

  const BOOST = 1.5;
  const rawMax = Math.max(...rows.map((r) => r.probability));
  const bucket = { A: 0, B: 0, C: 0, D: 0, noNode: 0 };
  const overrides = [];
  for (const r of rows) {
    const h = head.get(`${r.qstring} ${r.previous}`);
    if (h.current !== r.current) { bucket.A += 1; continue; }
    const cq = r.qstring.split(" ").pop();
    const n = node.get(cq);
    const u = bestByPhrase.get(r.current);
    if (!n || u === undefined) { bucket.noNode += 1; continue; }
    const stored = Math.min(u + BOOST + (r.probability - rawMax), -0.05);
    if (stored <= n.lo) bucket.B += 1;
    else if (stored > n.hi) bucket.C += 1;
    else bucket.D += 1;

    const top = topPhrase.get(cq);
    if (top && top.phrase !== r.current && stored > n.hi) {
      // 分桶讓人工複核好掃：同長度且只差一個字的，多半是異體字或人稱代詞
      // 這類「同音近形」問題（臺灣/台灣、瞭解/了解、它/他），性質和真正的
      // 同音異詞消歧不同，應該回到校正層處理而不是靠 bigram。
      const a = [...r.current];
      const b = [...top.phrase];
      let bucket = "其他";
      if (a.length === b.length) {
        const diff = a.reduce((n2, ch, i) => n2 + (ch === b[i] ? 0 : 1), 0);
        if (diff === 1) bucket = "單字差異";
      }
      // 讀音存疑優先於其他分類：這類要先確認 current 掛的讀音對不對，
      // 判斷「語料選擇 vs 預設候選」在此之前沒有意義。
      const spread = readingSpread.get(r.current);
      if (spread !== undefined && spread <= 0.1) bucket = "讀音存疑";
      overrides.push([bucket, r.previous, r.current, top.phrase, r.docCount, cq,
        (examples.get(`${r.previous}\t${r.current}`) ?? "").slice(0, 160)]);
    }
  }
  const tot = rows.length;
  const pc = (x) => `${((x / tot) * 100).toFixed(1)}%`;
  console.log(`\n分類（對照 ${cfg.db}）：`);
  console.log(`  A 不可達 ${bucket.A} ${pc(bucket.A)}｜B 永不勝出 ${bucket.B} ${pc(bucket.B)}｜` +
    `C 恆生效 ${bucket.C} ${pc(bucket.C)}｜D 條件生效 ${bucket.D} ${pc(bucket.D)}｜查無讀音 ${bucket.noNode}`);

  overrides.sort((a, b) => b[4] - a[4]);
  writeFileSync(
    cfg.review,
    [
      "# 複核表：第一欄留白＝沿用自動判定；填 keep 保留、drop 移除。",
      "# 填完直接以 --decisions 傳回本檔即可。",
      "# decision\tprevious\tcurrent(語料選擇)\t目前預設\tdoc_count\tbucket\t例句",
      ...overrides.map((r) => ["", r[1], r[2], r[3], r[4], r[0], r[6]].join("\t")),
    ].join("\n") + "\n",
    "utf8",
  );
  const dubious = overrides.filter((r) => r[0] === "讀音存疑");
  const oneChar = overrides.filter((r) => r[0] === "單字差異");
  const other = overrides.filter((r) => r[0] === "其他");
  console.log(`\n會改寫該讀音預設候選的列：${overrides.length} 筆 → ${cfg.review}`);
  console.log(`  讀音存疑（current 有多個近乎同分的讀音，配對可能錯誤）：${dubious.length} 筆 ← 先看這批`);
  for (const r of dubious.slice(0, 8)) console.log(`    ${r[1]} + ${r[2]}   （搶了 ${r[3]} 的位置，${r[4]} 篇）`);
  console.log(`  單字差異（異體字／人稱代詞，建議回校正層處理）：${oneChar.length} 筆`);
  for (const r of oneChar.slice(0, 6)) console.log(`    ${r[1]} + ${r[2]}   （原本 ${r[3]}，${r[4]} 篇）`);
  console.log(`  其他（真正的同音異詞消歧候選）：${other.length} 筆`);
  for (const r of other.slice(0, 10)) console.log(`    ${r[1]} + ${r[2]}   （原本 ${r[3]}，${r[4]} 篇）`);
}

main();
