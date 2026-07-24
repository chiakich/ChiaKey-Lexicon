# 維護腳本使用說明

本文件列出 `scripts/` 下的維護工具。除 shell script 外，JavaScript 工具以 Node.js 執行；需要檢查或讀取建置結果的工具，請先在 repository 根目錄執行：

```sh
cargo run --release -- prepare-release
```

`prepare-release` 的 OpenCC 前置條件與正式發版流程，請見 [ReleaseFlow.zh-TW.md](ReleaseFlow.zh-TW.md)。所有指令都應由 repository 根目錄執行。

## 本機詞庫安裝與取消

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `install-dev-lexicon.sh` | 建置並讓輸入法載入本機 `dev` 詞庫。既有 slot 會先備份。 | `scripts/install-dev-lexicon.sh`；已有產物時用 `scripts/install-dev-lexicon.sh --no-build` |
| `uninstall-dev-lexicon.sh` | 停止使用本機 `dev` 詞庫，移除 `active -> versions/local-dev` 並刪除該 slot。它只會在 `active` 確實指向該 slot 時動作。 | `scripts/uninstall-dev-lexicon.sh` |

取消 dev 詞庫但保留檔案以便日後重新切換：

```sh
scripts/uninstall-dev-lexicon.sh --keep-slot
```

兩個腳本皆可用 `ACTIVE_ROOT` 覆寫 ChiaKey 詞庫根目錄，並用 `SLOT` 覆寫預設的 `local-dev` slot。例如：

```sh
ACTIVE_ROOT=/tmp/ChiaKey-Lexicons SLOT=test-dev scripts/uninstall-dev-lexicon.sh
```

取消後請重啟 ChiaKey（或重新切換輸入來源），並到「偏好設定 → 更新」下載／啟用正式 release 詞庫。

## 詞條與 bigram 維護

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `add-explicit.mjs` | 將指定詞條加入 `chiaki-modern-overlay/explicit.tsv`。預設權重由腳本推算。 | `node scripts/add-explicit.mjs "su3 cl3" 你好 --dry-run` |
| `add-bigram.mjs` | 將詞組轉換關係加入 bigram overlay。 | `node scripts/add-bigram.mjs 天意 "tu0 u4" 難測 "s06hk4" --dry-run` |
| `explain-weight.mjs` | 顯示詞條在正規化結果及各原始來源中的權重／頻率，協助判斷來源與勝出原因。 | `node scripts/explain-weight.mjs 童音 同音` |
| `process-missing-word-issue.mjs` | GitHub Actions 用：讀取 `ISSUE_BODY` 等環境變數，驗證缺詞回報、寫出回覆及 PR 資料。通常不需在本機直接執行。 | 由 `.github/workflows/add-unigram.yml` 呼叫 |
| `process-bigram-issue.mjs` | GitHub Actions 用：讀取 issue 環境變數，驗證並建立 bigram 變更資料。通常不需在本機直接執行。 | 由 `.github/workflows/add-bigram.yml` 呼叫 |

`add-explicit.mjs` 可用 `--weight` 指定權重、`--tag` 重複新增 tag、或 `--tags` 指定 tag 字串；`add-bigram.mjs` 可用 `--probability` 指定機率。兩者先使用 `--dry-run` 檢查，遇到重複資料時只有在確認合理後才使用 `--force`。

## 資料品質稽核

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `audit-boneyard-legacy-weights.mjs` | 找出 KeyKey boneyard 舊資料中，可能因舊權重而壓過現代語料同音候選的項目；結果僅供人工審查。 | `node scripts/audit-boneyard-legacy-weights.mjs --top 50 --min-ratio 3` |
| `audit-rime-rerank-variants.mjs` | 找出 Rime 重排後值得以台灣用語檢視的候選差異。 | `node scripts/audit-rime-rerank-variants.mjs --top 100 --max-gap 0.35 --min-tsi-ratio 1` |

`audit-rime-rerank-variants.mjs --help` 可列出完整選項，包括 `--min-shared-positions` 與 `--include-overlay-winners`。

## 熱詞與 release 輔助

| 腳本 | 用途 | 常用指令 |
| --- | --- | --- |
| `hotwords.mjs` | 蒐集 Google Trends 觀測值，或依歷史觀測值更新自動熱詞 overlay。主要由 CI 排程執行。 | `node scripts/hotwords.mjs collect --output tmp/hotwords-observations/DATE.json` |
| `compute-release-version.sh` | 依 Asia/Taipei 當月既有 Git tag 算出下一個 `YYYY.MM.N` 版號；傳入參數則原樣輸出。 | `scripts/compute-release-version.sh`；`scripts/compute-release-version.sh 2026.07.1` |

更新熱詞的完整範例：

```sh
node scripts/hotwords.mjs refresh \
  --observations-dir tmp/hotwords-observations \
  --state sources/chiaki-auto-hotwords-overlay/state.json \
  --output sources/chiaki-auto-hotwords-overlay/phrases.tsv \
  --summary tmp/hotwords-summary.md
```

熱詞腳本可使用 `--date`（collect）或 `--today`（refresh）讓流程可重現；其餘行為與 CI 排程請見 [ReleaseFlow.zh-TW.md](ReleaseFlow.zh-TW.md)。
