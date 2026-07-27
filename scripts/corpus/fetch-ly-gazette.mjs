#!/usr/bin/env node
// 抓取立法院公報議程的處理後純文字檔（ly.govapi.tw v2）。
//
// 授權：立法院網站資料開放宣告（無償、非專屬、可再授權，需標示來源）。
// 本腳本只負責下載原始文字到本機快取；語料萃取見 extract-ly-speech.mjs。
//
// 用法：
//   node scripts/corpus/fetch-ly-gazette.mjs --terms 10,11 --out tmp/ly-gazette
//   node scripts/corpus/fetch-ly-gazette.mjs --terms 11 --out tmp/ly-gazette --limit 50   # 試跑
//
// 可重複執行：已下載的檔案會跳過，中斷後直接再跑一次即可續傳。

import { mkdir, writeFile, readFile, access } from "node:fs/promises";
import { join } from "node:path";

const API = "https://ly.govapi.tw/v2";
const DEFAULTS = {
  terms: [10, 11],
  out: "tmp/ly-gazette",
  pageSize: 100,
  delayMs: 1200,
  maxRetries: 5,
  limit: 0, // 0 = 不限
  retryFailed: false,
};

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
      case "--terms":
        cfg.terms = next()
          .split(",")
          .map((t) => Number.parseInt(t.trim(), 10));
        break;
      case "--out":
        cfg.out = next();
        break;
      case "--page-size":
        cfg.pageSize = Number.parseInt(next(), 10);
        break;
      case "--delay":
        cfg.delayMs = Number.parseInt(next(), 10);
        break;
      case "--limit":
        cfg.limit = Number.parseInt(next(), 10);
        break;
      case "--retry-failed":
        cfg.retryFailed = true;
        break;
      case "--help":
      case "-h":
        console.log(
          [
            "fetch-ly-gazette.mjs — 下載立法院公報議程純文字檔",
            "",
            "  --terms <n,n>     屆別，預設 10,11",
            "  --out <dir>       輸出目錄，預設 tmp/ly-gazette",
            "  --page-size <n>   列表分頁大小，預設 100",
            "  --delay <ms>      每次請求間隔，預設 1200（API 有速率限制，勿調太低）",
            "  --limit <n>       最多下載幾份文件（試跑用），預設 0 表示不限",
            "  --retry-failed    重試 failures.json 裡記錄的永久失敗項目",
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

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

// 永久失敗：上游那份文件的 txt 轉換壞了，重試不會好。
class PermanentFailure extends Error {}

// 429（限流）值得退避等待；5xx / 空回應等也沒用，快速放棄並記錄。
async function fetchWithRetry(url, cfg, asText = false) {
  let wait = cfg.delayMs;
  let serverErrors = 0;
  for (let attempt = 0; attempt < cfg.maxRetries; attempt += 1) {
    const response = await fetch(url, {
      headers: { "user-agent": "ChiaKey-Lexicon/gazette-fetch (+https://github.com/akira02/ChiaKey-Lexicon)" },
    });
    if (response.ok) {
      const body = asText ? await response.text() : await response.json();
      // 純文字端點在被限流時會回 200 + "Too Many Requests" 字串。
      if (asText && body.trim() === "Too Many Requests") {
        await sleep(wait);
        wait *= 2;
        continue;
      }
      // 空回應代表上游轉檔失敗；不要存成看似成功的空檔案。
      if (asText && body.trim().length === 0) {
        throw new PermanentFailure(`empty document body: ${url}`);
      }
      return body;
    }
    if (response.status === 429) {
      await sleep(wait);
      wait *= 2;
      continue;
    }
    if (response.status >= 500) {
      serverErrors += 1;
      if (serverErrors >= 2) throw new PermanentFailure(`HTTP ${response.status} (twice): ${url}`);
      await sleep(cfg.delayMs);
      continue;
    }
    throw new PermanentFailure(`HTTP ${response.status} for ${url}`);
  }
  throw new Error(`rate-limited after ${cfg.maxRetries} attempts: ${url}`);
}

async function listAgendas(term, cfg) {
  const agendas = [];
  let page = 1;
  let totalPage = 1;
  do {
    const url = `${API}/gazette_agendas?limit=${cfg.pageSize}&page=${page}&${encodeURIComponent("屆")}=${term}`;
    const body = await fetchWithRetry(url, cfg);
    totalPage = body.total_page ?? 1;
    for (const agenda of body.gazetteagendas ?? []) {
      const txt = (agenda["處理後公報網址"] ?? []).find((entry) => entry.type === "txt");
      if (!txt) continue;
      agendas.push({
        id: agenda["公報議程編號"],
        term: agenda["屆"],
        session: agenda["會期"],
        date: (agenda["會議日期"] ?? [])[0] ?? null,
        category: agenda["類別代碼"],
        pageStart: agenda["起始頁碼"],
        pageEnd: agenda["結束頁碼"],
        subject: agenda["案由"],
        txtUrl: txt.url,
      });
    }
    process.stderr.write(`\r屆 ${term}: 列表 ${page}/${totalPage}，累積 ${agendas.length} 筆`);
    page += 1;
    await sleep(cfg.delayMs);
  } while (page <= totalPage);
  process.stderr.write("\n");
  return agendas;
}

async function main() {
  const cfg = parseArgs(process.argv.slice(2));
  const docsDir = join(cfg.out, "docs");
  await mkdir(docsDir, { recursive: true });

  let index = [];
  const indexPath = join(cfg.out, "index.json");
  if (await exists(indexPath)) {
    index = JSON.parse(await readFile(indexPath, "utf8"));
    console.error(`沿用既有索引 ${index.length} 筆（刪除 ${indexPath} 可重建）`);
  } else {
    for (const term of cfg.terms) {
      index.push(...(await listAgendas(term, cfg)));
    }
    await writeFile(indexPath, JSON.stringify(index, null, 2), "utf8");
    console.error(`索引寫入 ${indexPath}，共 ${index.length} 筆`);
  }

  // 已知永久失敗的文件：重跑時直接跳過，不再浪費重試。--retry-failed 可清空重試。
  const failuresPath = join(cfg.out, "failures.json");
  let permanent = {};
  if (!cfg.retryFailed && (await exists(failuresPath))) {
    permanent = JSON.parse(await readFile(failuresPath, "utf8"));
    const n = Object.keys(permanent).length;
    if (n > 0) console.error(`跳過 ${n} 筆已知永久失敗（--retry-failed 可重試）`);
  }

  let downloaded = 0;
  let skipped = 0;
  let failed = 0;
  let rateLimited = 0;
  const target = cfg.limit > 0 ? index.slice(0, cfg.limit) : index;

  for (const [i, agenda] of target.entries()) {
    const path = join(docsDir, `${agenda.id}.txt`);
    if (await exists(path) || permanent[agenda.id]) {
      skipped += 1;
      continue;
    }
    try {
      const text = await fetchWithRetry(agenda.txtUrl, cfg, true);
      await writeFile(path, text, "utf8");
      downloaded += 1;
    } catch (error) {
      failed += 1;
      if (error instanceof PermanentFailure) {
        permanent[agenda.id] = error.message;
        await writeFile(failuresPath, JSON.stringify(permanent, null, 2), "utf8");
      } else {
        rateLimited += 1;
        // 連續限流代表整體太快，放慢後續請求。
        cfg.delayMs = Math.min(cfg.delayMs * 2, 10000);
        console.error(`\n${agenda.id} 限流放棄，delay 調整為 ${cfg.delayMs}ms`);
      }
    }
    if (i % 10 === 0) {
      process.stderr.write(
        `\r下載中 ${i + 1}/${target.length}（新增 ${downloaded}、跳過 ${skipped}、失敗 ${failed}）`,
      );
    }
    await sleep(cfg.delayMs);
  }
  process.stderr.write("\n");
  const perm = Object.keys(permanent).length;
  console.error(`完成：新增 ${downloaded}、跳過 ${skipped}、失敗 ${failed}，輸出於 ${docsDir}`);
  if (perm > 0) console.error(`永久失敗 ${perm} 筆（上游轉檔壞掉），已記錄於 ${failuresPath}`);
  if (rateLimited > 0) console.error(`因限流放棄 ${rateLimited} 筆，重跑本腳本會續傳。`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
