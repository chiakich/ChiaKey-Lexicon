# 維護腳本使用說明

本文件列出 `scripts/` 下的維護工具，依用途分成五個子目錄：

| 目錄 | 用途 |
| --- | --- |
| `scripts/release/` | 本機詞庫安裝、取消與版號計算 |
| `scripts/lexicon/` | 詞條與 bigram 的增修、剪除、issue 處理 |
| `scripts/audit/` | 只讀不改的品質稽核與權重追查 |
| `scripts/corpus/` | 外部語料抓取、萃取與 bigram 產生 |
| `scripts/hotwords/` | 熱詞訊號蒐集與自動 overlay 更新 |

除 shell script 外，JavaScript 工具以 Node.js 執行；需要檢查或讀取建置結果的工具，請先在 repository 根目錄執行：

```sh
cargo run --release -- prepare-release
```

`prepare-release` 的 OpenCC 前置條件與正式發版流程，請見 [ReleaseFlow.zh-TW.md](ReleaseFlow.zh-TW.md)。所有指令都應由 repository 根目錄執行。

## 本機詞庫安裝與取消

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `install-dev-lexicon.sh` | 建置並讓輸入法載入本機 `dev` 詞庫。既有 slot 會先備份。 | `scripts/release/install-dev-lexicon.sh`；已有產物時用 `scripts/release/install-dev-lexicon.sh --no-build` |
| `uninstall-dev-lexicon.sh` | 停止使用本機 `dev` 詞庫，移除 `active -> versions/local-dev` 並刪除該 slot。它只會在 `active` 確實指向該 slot 時動作。 | `scripts/release/uninstall-dev-lexicon.sh` |

取消 dev 詞庫但保留檔案以便日後重新切換：

```sh
scripts/release/uninstall-dev-lexicon.sh --keep-slot
```

兩個腳本皆可用 `ACTIVE_ROOT` 覆寫 ChiaKey 詞庫根目錄，並用 `SLOT` 覆寫預設的 `local-dev` slot。例如：

```sh
ACTIVE_ROOT=/tmp/ChiaKey-Lexicons SLOT=test-dev scripts/release/uninstall-dev-lexicon.sh
```

取消後請重啟 ChiaKey（或重新切換輸入來源），並到「偏好設定 → 更新」下載／啟用正式 release 詞庫。

## 詞條與 bigram 維護

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `add-unigram.mjs` | 將指定詞條加入 `chiaki-modern-overlay/unigrams.tsv`。預設權重由腳本推算。 | `node scripts/lexicon/add-unigram.mjs "su3 cl3" 你好 --dry-run` |
| `add-bigram.mjs` | 將詞組轉換關係加入 bigram overlay。 | `node scripts/lexicon/add-bigram.mjs 天意 "tu0 u4" 難測 "s06hk4" --dry-run` |
| `prune-dead-bigrams.mjs` | 依走訪器可達性剪掉 A（不可達）與 B（永不勝出）的 bigram 列。預設試算，`--apply` 才寫檔。詳見 [BigramPruning.zh-TW.md](BigramPruning.zh-TW.md)。 | `node scripts/lexicon/prune-dead-bigrams.mjs`；確認後加 `--apply` |
| `process-missing-word-issue.mjs` | GitHub Actions 用：讀取 `ISSUE_BODY` 等環境變數，驗證缺詞回報、寫出回覆及 PR 資料。通常不需在本機直接執行。 | 由 `.github/workflows/add-unigram.yml` 呼叫 |
| `process-bigram-issue.mjs` | GitHub Actions 用：讀取 issue 環境變數，驗證並建立 bigram 變更資料。通常不需在本機直接執行。 | 由 `.github/workflows/add-bigram.yml` 呼叫 |

`add-unigram.mjs` 可用 `--weight` 指定權重、`--tag` 重複新增 tag、或 `--tags` 指定 tag 字串；`add-bigram.mjs` 可用 `--probability` 指定機率。兩者先使用 `--dry-run` 檢查，遇到重複資料時只有在確認合理後才使用 `--force`。

## 資料品質稽核

`explain-weight.mjs` 原列於「詞條與 bigram 維護」，因其為純唯讀診斷工具，已移至 `scripts/audit/`。

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `audit-bigram-effectiveness.mjs` | 依走訪器可達性把 DB 中每筆 bigram 分成 A/B/C/D 四類，並輸出可剪除清單與並列讀音清單。判準見 [WalkerScoring.zh-TW.md](WalkerScoring.zh-TW.md)，操作流程見 [BigramPruning.zh-TW.md](BigramPruning.zh-TW.md)。 | `node scripts/audit/audit-bigram-effectiveness.mjs --prune-out tmp/prunable.tsv` |
| `audit-unigram-health.mjs` | 把 DB 中每筆多字 unigram 分成八類讀音／可達性狀態，並依來源層彙總。作法取自唯音輸入法先鋒語料庫的 `Collector_HealthCheck.swift`，判準換成本專案走訪器的有效分數（`weight + 1.0 ×(音節數−1)`）。 | `node scripts/audit/audit-unigram-health.mjs --out tmp/unigram-health.tsv` |
| `explain-weight.mjs` | 顯示詞條在正規化結果及各原始來源中的權重／頻率，協助判斷來源與勝出原因。 | `node scripts/audit/explain-weight.mjs 童音 同音` |
| `audit-boneyard-legacy-weights.mjs` | 找出 KeyKey boneyard 舊資料中，可能因舊權重而壓過現代語料同音候選的項目；結果僅供人工審查。 | `node scripts/audit/audit-boneyard-legacy-weights.mjs --top 50 --min-ratio 3` |
| `audit-rime-rerank-variants.mjs` | 找出 Rime 重排後值得以台灣用語檢視的候選差異。 | `node scripts/audit/audit-rime-rerank-variants.mjs --top 100 --max-gap 0.35 --min-tsi-ratio 1` |

`audit-rime-rerank-variants.mjs --help` 可列出完整選項，包括 `--min-shared-positions` 與 `--include-overlay-winners`。

### audit-unigram-health.mjs 的八個類別

前五類檢查「詞的注音對不對得上單字讀音表」，只在音節數等於字數時逐字比對；後三類檢查「這筆詞條在走訪器裡贏不贏得了完全退化的逐字路徑」。

| 類別 | 意義 | 該怎麼處理 |
| --- | --- | --- |
| `reading-mismatch` | 該位置的聲韻組合不在該字已知讀音裡 | 看下方「缺的是哪一邊」 |
| `tone-mismatch` | 聲韻相同但聲調不在該字已知讀音裡（非輕聲） | 同上 |
| `abbreviated-reading` | 該列含不可能單獨成音節的裸聲母 | KeyKey boneyard 的簡拼列，非真讀音，不可補進單字表 |
| `neutral-tone` | 該位置是輕聲，單字表只收本調 | 詞層輕聲的預期行為，通常不必處理 |
| `reading-unknown` | 某字在詞庫裡完全沒有單字列 | 補單字讀音，否則該字無法單獨輸入 |
| `length-mismatch` | 音節數與字數不符 | 合音、兒化（`那兒` = `ㄋㄦˋ`）等，無法逐字比對，僅列出供檢視 |
| `indifferent` | 詞等於逐字最佳串接，且贏不了逐字路徑 | 走訪器本來就會輸出同樣的字，這筆詞條只佔體積 |
| `insufficient` | 詞不等於逐字最佳串接，但贏不了逐字路徑 | 這是真的打不出來的詞，該升權或檢查來源 |
| `capped` | 同上兩類，但來源標記為刻意降權 | 贏不了逐字正是設計意圖，不必處理 |

#### 缺的是哪一邊

`reading-mismatch` 與 `tone-mismatch` 只說「詞層讀音與單字表不一致」，**不說哪一邊錯**，不要預設是詞標錯了。實測本專案的資料，多數是單字表漏收了台灣實際在用的讀音：`好萊塢` 的 `塢` 唸ㄨ、`咖哩` 的 `咖` 唸ㄍㄚ、`佣金` 的 `佣` 唸ㄩㄥ、`焢肉` 的 `焢` 唸ㄎㄨㄥˋ，這些詞的讀音都是對的，單字表卻只有另一個音。

缺單字讀音的影響不只在這些詞：該字無法單獨以這個音輸入，其他需要同一個音的詞也一樣打不出來。

因此 `--missing-out` 會把這兩類依 `(字, 缺的讀音)` 聚合，附上佐證詞數與例詞，直接對應 `sources/chiaki-modern-overlay/reading-supplements.tsv` 的補法。同一組被越多詞佐證，越可能是單字表漏收；只有一兩個詞佐證的才需要回頭確認是不是那些詞標錯。輕聲不列入這份清單——詞層輕聲不該回頭補成單字讀音。

```bash
node scripts/audit/audit-unigram-health.mjs --missing-out tmp/missing-readings.tsv
```

`abbreviated-reading` 的判準是「該列有 cell 是裸聲母（ㄅㄆㄇㄈ…）」——那種音節不存在，只會出現在簡拼列（`雅虎奇摩輸入法` 的 ㄏ ㄑ ㄇ ㄈ）。不能改用「單一注音符號」，因為ㄓㄔㄕㄖㄗㄘㄙ的空韻與單獨韻母都是完整音節（`試試` ㄕ˙、`二` ㄦˋ）；也不能只看「是已知讀音的截斷」，那會誤傷剛好是截斷形的常見誤讀（`鍥而不捨` ㄑㄧˋ）與台語借音（`哭枵` ㄧㄠ）。

`--missing-out` 的 `alt_reading_only` 欄標出「所有佐證詞都來自 `alt-reading,common-mistype` 列」的組。本專案的標準是**這些也要收**：詞層既然收了常見誤讀，單字層不收會讓打錯的人只能整詞打出、拆開就打不出來。

`capped` 是從 `indifferent` / `insufficient` 分出來的：來源 tag 含 `compositional-cap`（`669e85a` 把 essay 的組合式條目壓下去，好讓 bigram 層有機會介入）、`fragment-demote` 或 `demote` 的列，本來就是為了輸給其他路徑才存在的。不分出來的話，這幾百列會把真正需要處理的項目淹掉。新增降權機制時記得同步更新腳本裡的 `DEMOTION_TAGS`。

判準說明：

- **DB 必須比 `sources/` 新**。腳本會比對兩者的 mtime，DB 較舊時直接中止——否則報表反映的是上一版狀況，已經補好的東西會被重報（實際發生過：`丼` 的讀音已進 `reading-supplements`，舊 DB 仍說它缺）。確定要用舊 DB 時加 `--allow-stale`。
- 有效分數的換算依 [WalkerScoring.zh-TW.md](WalkerScoring.zh-TW.md)〈詞長加分〉。逐字路徑的每個單字節點加分為 0，所以整詞必須滿足 `weight + 1.0 ×(音節數−1) > Σ 各音節最佳單字 weight` 才贏得過逐字。
- 路徑比較會帶入 `unigrams.backoff`，且**不需要句子脈絡**。走訪器的 unigram path 是 `unigram[0] + backoff(previous)`，整句分數沿路累加，所以展開後：整詞是 `weight(W) + backoff(P) + 1.0×(N−1)`，逐字是 `Σ weight(ci) + backoff(P) + backoff(c1) + … + backoff(c_{N−1})`。`backoff(P)` 兩邊各一次而相消，span 外面接什麼不影響勝負；span 內部的 backoff 則從各單字自己那列取得。目前 backoff 欄全為 0，這幾項都消失。
- 逐字是**最弱的競爭者**。贏不了逐字的列一定也贏不了任何更好的拆詞路徑，所以 `indifferent` / `insufficient` 報出來的都是確定有問題的列；但沒被報出來不代表一定會贏，那要另外用完整 walker 檢查。
- `neutral-tone` 與 `tone-mismatch` 的細分需要把 qstring 解回注音，會呼叫 `target/release/chiakey-lexicon qstring-to-bpmf`。沒有先 `cargo build --release` 時，這兩類會全部併回 `reading-mismatch`，`--missing-out` 也會因此混入輕聲而失準。
- 來源層彙總讀 `normalized/smart-mandarin.tsv`（`prepare-release` 產生、不進版控）；沒有這個檔就只是來源欄留空。
- 本工具只讀不寫，不會改動 DB、來源檔或任何權重。

## 外部語料

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `fetch-ly-gazette.mjs` | 下載立法院公報議程的處理後純文字檔。可中斷續傳，永久失敗記錄於 `failures.json`。 | `node scripts/corpus/fetch-ly-gazette.mjs --terms 10,11 --out tmp/ly-gazette` |
| `extract-ly-speech.mjs` | 從公報文字檔只萃取詢答口語段落，丟棄議事錄、預算決議與書面報告。 | `node scripts/corpus/extract-ly-speech.mjs` |
| `discover-ly-words.mjs` | 從口語語料找出詞庫尚未收錄的詞。以內聚度（PMI）與左右鄰接熵兩條判準並用，避開切窗碎片與「的」系自由組合。輸出的讀音欄位是提案，多音字必須逐筆複核後才能進 `sources/`。 | `node scripts/corpus/discover-ly-words.mjs --limit-chars 12000000 --min-count 150` |
| `postprocess-ly-bigrams.mjs` | 把 `build-bigram-stats` 的輸出整理成 source 格式：換算機率、過濾語域偏誤、產生人工複核表。 | `node scripts/corpus/postprocess-ly-bigrams.mjs --min-doc-count 17 --drop-dubious-reading` |
| `build-ly-corpus.sh` | 上述三步加上 `build-bigram-stats` 的一鍵管線。 | `LIMIT=50 scripts/corpus/build-ly-corpus.sh` |

過濾判準與語域分析見 [`sources/tw-ly-transcript/README.md`](../sources/tw-ly-transcript/README.md) 的研究附錄。

## 熱詞與 release 輔助

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `collect-ptt-gossiping.py` | 蒐集 PTT 八卦板熱門標題與推文彙總訊號，供 hotwords 流程使用。由 CI 呼叫。 | 由 `.github/workflows/hotwords.yml` 呼叫 |
| `hotwords.mjs` | 蒐集 Google Trends 觀測值，或依歷史觀測值更新自動熱詞 overlay。主要由 CI 排程執行。 | `node scripts/hotwords/hotwords.mjs collect --output tmp/hotwords-observations/DATE.json` |
| `compute-release-version.sh` | 依 Asia/Taipei 當月既有 Git tag 算出下一個 `YYYY.MM.N` 版號；傳入參數則原樣輸出。 | `scripts/release/compute-release-version.sh`；`scripts/release/compute-release-version.sh 2026.07.1` |

更新熱詞的完整範例：

```sh
node scripts/hotwords/hotwords.mjs refresh \
  --observations-dir tmp/hotwords-observations \
  --state sources/chiaki-auto-hotwords-overlay/state.json \
  --output sources/chiaki-auto-hotwords-overlay/phrases.tsv \
  --summary tmp/hotwords-summary.md
```

熱詞腳本可使用 `--date`（collect）或 `--today`（refresh）讓流程可重現；其餘行為與 CI 排程請見 [ReleaseFlow.zh-TW.md](ReleaseFlow.zh-TW.md)。
