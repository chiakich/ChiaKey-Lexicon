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

import { mkdir, writeFile, readFile, access } from 'node:fs/promises'
import { join } from 'node:path'

const API = 'https://ly.govapi.tw/v2'
const DEFAULTS = {
  terms: [10, 11],
  out: 'tmp/ly-gazette',
  pageSize: 100,
  delayMs: 1200,
  maxRetries: 5,
  limit: 0,
  retryFailed: false,
  retryDeferred: false,
  decayAfter: 25,
  throttleAbort: 5,
}

function parseArgs(argv) {
  const cfg = { ...DEFAULTS }
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    const next = () => {
      const value = argv[i + 1]
      if (value === undefined) throw new Error(`missing value for ${arg}`)
      i += 1
      return value
    }
    switch (arg) {
      case '--terms':
        cfg.terms = next()
          .split(',')
          .map((t) => Number.parseInt(t.trim(), 10))
        break
      case '--out':
        cfg.out = next()
        break
      case '--page-size':
        cfg.pageSize = Number.parseInt(next(), 10)
        break
      case '--delay':
        cfg.delayMs = Number.parseInt(next(), 10)
        break
      case '--limit':
        cfg.limit = Number.parseInt(next(), 10)
        break
      case '--retry-failed':
        cfg.retryFailed = true
        break
      case '--decay-after':
        cfg.decayAfter = Number.parseInt(next(), 10)
        break
      case '--retry-deferred':
        cfg.retryDeferred = true
        break
      case '--throttle-abort':
        cfg.throttleAbort = Number.parseInt(next(), 10)
        break
      case '--help':
      case '-h':
        console.log(
          [
            'fetch-ly-gazette.mjs — 下載立法院公報議程純文字檔',
            '',
            '  --terms <n,n>     屆別，預設 10,11。建索引與下載都只處理這些屆',
            '  --out <dir>       輸出目錄，預設 tmp/ly-gazette',
            '  --page-size <n>   列表分頁大小，預設 100',
            '  --delay <ms>      每次請求間隔，預設 1200（API 有速率限制，勿調太低）',
            '  --limit <n>       最多下載幾份文件（試跑用），預設 0 表示不限',
            '  --retry-failed    重試 failures.json 裡記錄的永久失敗項目（上游轉檔壞掉）',
            '  --retry-deferred  重試 deferred.json 裡記錄的項目（上游持續 throttle）',
            '  --decay-after <n> 連續成功幾次就把 delay 減半回收，預設 25',
            '  --throttle-abort <n> 連續幾份被 throttle 就判定全域限流並中止，預設 5',
          ].join('\n'),
        )
        process.exit(0)
        break
      default:
        throw new Error(`unknown option: ${arg}`)
    }
  }
  return cfg
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}

// 永久失敗：上游那份文件的 txt 轉換壞了，重試不會好。
class PermanentFailure extends Error {}

// 「Too Many Requests」其實有兩種完全不同的情況，必須分開處理：
//
//   1. 真的全域限流 — 連續好幾份不同的文件都被擋，放慢或改天再跑會好。
//   2. 單一文件永遠回這個 — 上游那份的轉檔卡住或失敗，API 拿這個字串當萬用
//      錯誤回應。實測 1155206_00002（358 頁）從乾淨 IP 單一次冷請求就直接回
//      「Too Many Requests」，換網路、等待、重試都不會好。
//
// 舊版把兩者一律當成 (1)：對一份永遠不會成功的文件燒掉 5 次重試（約 37 秒），
// 然後把全域 delay 加倍，而且不記錄——於是每次重跑都在同一份文件上重蹈覆轍，
// 並讓後面幾千筆全部變慢。判準是「別的文件同時期有沒有成功」。
class ThrottledDocument extends Error {}

// Retry-After 可能是秒數或 HTTP date，兩種都要處理。回傳毫秒，沒有就回 null。
function retryAfterMs(response) {
  const raw = response.headers.get('retry-after')
  if (!raw) return null
  const seconds = Number.parseInt(raw, 10)
  if (Number.isFinite(seconds)) return seconds * 1000
  const at = Date.parse(raw)
  return Number.isFinite(at) ? Math.max(0, at - Date.now()) : null
}

// 429（限流）值得退避等待；5xx / 空回應等也沒用，快速放棄並記錄。
async function fetchWithRetry(url, cfg, asText = false) {
  let wait = cfg.delayMs
  let serverErrors = 0
  for (let attempt = 0; attempt < cfg.maxRetries; attempt += 1) {
    const response = await fetch(url, {
      headers: { 'user-agent': 'ChiaKey-Lexicon/gazette-fetch (+https://github.com/akira02/ChiaKey-Lexicon)' },
    })
    if (response.ok) {
      const body = asText ? await response.text() : await response.json()
      // 純文字端點在被限流時會回 200 + "Too Many Requests" 字串。
      // 這條路徑沒有標頭可讀，只能盲目退避。
      if (asText && body.trim() === 'Too Many Requests') {
        await sleep(wait)
        wait *= 2
        continue
      }
      // 空回應代表上游轉檔失敗；不要存成看似成功的空檔案。
      if (asText && body.trim().length === 0) {
        throw new PermanentFailure(`empty document body: ${url}`)
      }
      return body
    }
    if (response.status === 429) {
      // 伺服器有講就聽它的，不要自己猜。
      const hinted = retryAfterMs(response)
      await sleep(hinted ?? wait)
      wait = hinted ? hinted * 2 : wait * 2
      continue
    }
    if (response.status >= 500) {
      serverErrors += 1
      if (serverErrors >= 2) throw new PermanentFailure(`HTTP ${response.status} (twice): ${url}`)
      await sleep(cfg.delayMs)
      continue
    }
    throw new PermanentFailure(`HTTP ${response.status} for ${url}`)
  }
  throw new ThrottledDocument(`throttled after ${cfg.maxRetries} attempts: ${url}`)
}

async function listAgendas(term, cfg) {
  const agendas = []
  let page = 1
  let totalPage = 1
  do {
    const url = `${API}/gazette_agendas?limit=${cfg.pageSize}&page=${page}&${encodeURIComponent('屆')}=${term}`
    const body = await fetchWithRetry(url, cfg)
    totalPage = body.total_page ?? 1
    for (const agenda of body.gazetteagendas ?? []) {
      const txt = (agenda['處理後公報網址'] ?? []).find((entry) => entry.type === 'txt')
      if (!txt) continue
      agendas.push({
        id: agenda['公報議程編號'],
        term: agenda['屆'],
        session: agenda['會期'],
        date: (agenda['會議日期'] ?? [])[0] ?? null,
        category: agenda['類別代碼'],
        pageStart: agenda['起始頁碼'],
        pageEnd: agenda['結束頁碼'],
        subject: agenda['案由'],
        txtUrl: txt.url,
      })
    }
    process.stderr.write(`\r屆 ${term}: 列表 ${page}/${totalPage}，累積 ${agendas.length} 筆`)
    page += 1
    await sleep(cfg.delayMs)
  } while (page <= totalPage)
  process.stderr.write('\n')
  return agendas
}

async function main() {
  const cfg = parseArgs(process.argv.slice(2))
  const docsDir = join(cfg.out, 'docs')
  await mkdir(docsDir, { recursive: true })

  let index = []
  const indexPath = join(cfg.out, 'index.json')
  if (await exists(indexPath)) {
    index = JSON.parse(await readFile(indexPath, 'utf8'))
    console.error(`沿用既有索引 ${index.length} 筆（刪除 ${indexPath} 可重建）`)
  } else {
    for (const term of cfg.terms) {
      index.push(...(await listAgendas(term, cfg)))
    }
    await writeFile(indexPath, JSON.stringify(index, null, 2), 'utf8')
    console.error(`索引寫入 ${indexPath}，共 ${index.length} 筆`)
  }

  // 已知永久失敗的文件：重跑時直接跳過，不再浪費重試。--retry-failed 可清空重試。
  const failuresPath = join(cfg.out, 'failures.json')
  let permanent = {}
  if (!cfg.retryFailed && (await exists(failuresPath))) {
    permanent = JSON.parse(await readFile(failuresPath, 'utf8'))
    const n = Object.keys(permanent).length
    if (n > 0) console.error(`跳過 ${n} 筆已知永久失敗（--retry-failed 可重試）`)
  }

  // 單一文件層級的 throttle。跟 failures.json 分開放，因為它有機會隨上游修復而
  // 恢復，只是不該在每次例行重跑時重試。
  const deferredPath = join(cfg.out, 'deferred.json')
  let deferred = {}
  if (await exists(deferredPath)) {
    deferred = JSON.parse(await readFile(deferredPath, 'utf8'))
    const n = Object.keys(deferred).length
    if (n > 0) {
      if (cfg.retryDeferred) console.error(`重試 ${n} 筆先前被上游 throttle 的文件`)
      else console.error(`跳過 ${n} 筆上游持續 throttle 的文件（--retry-deferred 可重試）`)
    }
  }

  let downloaded = 0
  let skipped = 0
  let failed = 0
  let rateLimited = 0
  let streak = 0
  let consecutiveThrottles = 0
  const baseDelayMs = cfg.delayMs

  // 沿用既有索引時，索引裡可能含有這次不想處理的屆別（例如索引建過 10+11，
  // 但這輪只想補 11）。--terms 之前只作用在建索引階段，這裡補上下載端的過濾。
  const wanted = new Set(cfg.terms)
  const scoped = index.filter((a) => wanted.has(a.term))
  if (scoped.length !== index.length) {
    console.error(`索引 ${index.length} 筆，本輪只處理屆 ${cfg.terms.join('/')} 共 ${scoped.length} 筆`)
  }
  // 由舊到新處理。上游是「請求時才轉檔並快取」，越新的公報越可能還沒轉好——
  // 實測 2026-06 那幾冊（300+ 頁）一律回 Too Many Requests，而 2024-02 的同屆
  // 文件秒回。索引本身是新到舊，照原順序跑等於一開場就連撞好幾筆最難的，
  // 會把下面的全域限流判斷誤觸發。日期缺漏者排最後。
  const byDate = [...scoped].sort((a, b) => (a.date ?? '9999').localeCompare(b.date ?? '9999'))
  const target = cfg.limit > 0 ? byDate.slice(0, cfg.limit) : byDate

  for (const [i, agenda] of target.entries()) {
    const path = join(docsDir, `${agenda.id}.txt`)
    const skipDeferred = deferred[agenda.id] && !cfg.retryDeferred
    if ((await exists(path)) || permanent[agenda.id] || skipDeferred) {
      skipped += 1
      continue
    }
    try {
      const text = await fetchWithRetry(agenda.txtUrl, cfg, true)
      await writeFile(path, text, 'utf8')
      downloaded += 1
      streak += 1
      consecutiveThrottles = 0
      // 順利跑了一段就把之前因限流加上去的 delay 收回來，但不低於起始值。
      if (streak >= cfg.decayAfter && cfg.delayMs > baseDelayMs) {
        cfg.delayMs = Math.max(baseDelayMs, Math.floor(cfg.delayMs / 2))
        console.error(`\n連續成功 ${streak} 筆，delay 回降為 ${cfg.delayMs}ms`)
        streak = 0
      }
    } catch (error) {
      failed += 1
      streak = 0
      if (error instanceof PermanentFailure) {
        permanent[agenda.id] = error.message
        await writeFile(failuresPath, JSON.stringify(permanent, null, 2), 'utf8')
      } else {
        rateLimited += 1
        consecutiveThrottles += 1
        // 「連續失敗」只有在本輪已經成功過的前提下才代表全域限流；否則可能只是
        // 佇列開頭剛好排了一串上游沒轉好的文件，不該讓整輪停擺。
        if (downloaded > 0 && consecutiveThrottles >= cfg.throttleAbort) {
          console.error(`\n本輪已成功 ${downloaded} 筆後，連續 ${consecutiveThrottles} 份被 throttle，判定為全域限流。`)
          console.error(`已中止。稍後再跑（可續傳），或用 --delay ${cfg.delayMs * 2} 放慢。`)
          break
        }
        // 一筆都還沒成功就連續失敗到這個地步，比較可能是 API 整體不可用。
        if (downloaded === 0 && consecutiveThrottles >= cfg.throttleAbort * 4) {
          console.error(`\n開場連續 ${consecutiveThrottles} 份都失敗且無任何成功，API 可能整體不可用。已中止。`)
          break
        }
        // 別的文件跑得動，只有這份不行 → 上游那份的問題，記錄後跳過，
        // 不要因此拖慢全域速度。
        deferred[agenda.id] = error.message
        await writeFile(deferredPath, JSON.stringify(deferred, null, 2), 'utf8')
        console.error(`\n${agenda.id} 上游持續 throttle，記入 deferred.json 後跳過`)
      }
    }
    if (i % 10 === 0) {
      process.stderr.write(`\r下載中 ${i + 1}/${target.length}（新增 ${downloaded}、跳過 ${skipped}、失敗 ${failed}）`)
    }
    await sleep(cfg.delayMs)
  }
  process.stderr.write('\n')
  const perm = Object.keys(permanent).length
  const defer = Object.keys(deferred).length
  console.error(`完成：新增 ${downloaded}、跳過 ${skipped}、失敗 ${failed}，輸出於 ${docsDir}`)
  if (perm > 0) console.error(`永久失敗 ${perm} 筆（上游轉檔壞掉），已記錄於 ${failuresPath}`)
  if (defer > 0) {
    console.error(`上游 throttle ${defer} 筆，已記錄於 ${deferredPath}`)
    console.error(`  這類文件換 IP 或重試都不會好，是上游那份轉檔卡住。過陣子可用 --retry-deferred 再試。`)
  }
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
