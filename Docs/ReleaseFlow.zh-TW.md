# 詞庫 Release 流程

這份文件記錄 Chiaki KeyKey Lexicon 目前的 release 流程。原則是：日常開發走 `dev`，只有準備發版時才合併到 `main`，而 `main` 每次更新都會產生一版新的詞庫 release。

## Branch 角色

- `dev`：預設分支。日常調整詞庫來源、overlay、builder、文件與 CI 都先進這裡。
- `main`：release 分支。合併到 `main` 代表要發一版新的公開詞庫。
- feature branch：較大的實驗或調整可以先從 `dev` 分出，完成後開 PR 回 `dev`。

## 來源策略

目前詞庫由八層組成：

1. KeyKey Boneyard bootstrap：repo 內 vendored 一份 cooked DB，路徑是 `sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db`。這讓 CI 和 release build 不需要依賴本機的 `../KeyKey-Boneyard` checkout。
2. KeyKey BPMF punctuation table：repo 內 vendored 原始 KeyKey 標點 CIN，路徑是 `sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin`。release builder 只匯入 `%chardef` 裡 `_punctuation_` / `_ctrl_` 開頭的 rows，供 Smart Mandarin runtime 查標點候選。
3. KeyKey prepopulated service data：repo 內 vendored 原始 KeyKey `CannedMessages.plist`，路徑是 `sources/keykey-prepopulated-service-data/vendor/`。release builder 會把完整 plist 內容寫進 `prepopulated_service_data`，並補上正值 timestamp。OneKey 是 Yahoo 時代的 URL launcher，不屬於輸入詞庫資料，現代 Chiaki KeyKey 不再載入，因此 release 不產生 `onekey_services` 或 `onekey_services_timestamp`。
4. KeyKey module CIN tables：repo 內 vendored 原始 KeyKey 泛用輸入法、標點與注音文校正 CIN，路徑是 `sources/keykey-module-cin/vendor/`。release builder 會建立 module SQLite tables，例如 `Generic-cj-cin`、`Generic-simplex-cin`、`BopomofoCorrection-bopomofo-correction-cin`，不併入 Smart Mandarin language model。
5. libchewing-data：維持 upstream pinned source，不把完整 upstream repo 複製進來。用 `cargo run --release -- fetch-modern-sources` 下載固定版本與 SHA-256。
6. bpmf-ext-cin：repo 內 vendored 一份 Public Domain extended BPMF 單字表，路徑是 `sources/bpmf-ext-cin/vendor/bpmf-ext.cin`，只用來補缺的 CJK BMP 單字讀音。
7. Rime essay：維持 upstream pinned source，只抓固定 commit 的 `essay.txt` 與 license。
8. Chiaki modern overlay：repo 直接維護的小型人工補詞，路徑是 `sources/chiaki-modern-overlay/phrases.tsv`。

`sources/keykey-boneyard-bootstrap/source-inventory.sha256` 是 bootstrap DB 的 provenance，記錄當初 cooked DB 來自哪些 KeyKey Boneyard 檔案與 SHA-256。release builder 實際讀取的是 vendored cooked DB。

## 本機建置

先下載現代詞庫來源：

```sh
cargo run --release -- fetch-modern-sources
```

再建 release package：

```sh
cargo run --release -- prepare-release
```

預設會輸出：

```text
normalized/smart-mandarin.tsv
manifests/lexicon-manifest.json
dist/<version>/KeyKeySource-<version>.db
dist/<version>/KeyKeySource-<version>.json
dist/<version>/lexicon-manifest.json
dist/<version>/SHA256SUMS
```

若要指定版本：

```sh
LEXICON_VERSION=2026.06.4 cargo run --release -- prepare-release
```

若要測試不同 bootstrap DB：

```sh
BONEYARD_DB=/path/to/KeyKeySource.db cargo run --release -- prepare-release
```

## 版本規則

公開 release tag 使用：

```text
YYYY.MM.N
```

例如 `2026.06.4`。`YYYY.MM` 以 Asia/Taipei 日期為準，`N` 是當月流水號。CI 在 `main` 自動 release 時會讀取既有 tag，取同月份最大流水號再加一。

## CI/CD

GitHub Actions 的 release workflow 觸發條件：

- push 到 `main`
- 手動 `workflow_dispatch`

workflow 會做：

1. 安裝 Rust 與 SQLite 依賴。
2. 計算 release 版本。
3. 跑 `cargo test`。
4. 執行 `cargo run --release -- fetch-modern-sources`。
5. 執行 `cargo run --release -- prepare-release`。
6. 驗證 `SHA256SUMS`。
7. 用 SQLite smoke test 確認基本候選詞存在。
8. 建立 GitHub Release 並上傳 DB、metadata、manifest、checksum。

`dist/` 是本機與 CI 的 staging 目錄，不 commit 進 git。公開 artifacts 以 GitHub Release 為準。

若只是調整 release workflow 本身、而且會另外手動發版，可以在 commit message 放 `[skip release]`。這只應用在少數維護流程的情境；一般 `main` merge 應該讓 CI 自動 release。

## 發版流程

日常開發：

```text
feature branch -> dev
```

準備發版：

```text
dev -> main
```

合併到 `main` 後 CI 會自動建立新版 GitHub Release。若需要指定版本，可以手動執行 release workflow 並填入 `version`。

## Release 後檢查

發版完成後至少確認：

- GitHub Release 有四個 artifacts：`KeyKeySource-<version>.db`、`KeyKeySource-<version>.json`、`lexicon-manifest.json`、`SHA256SUMS`。
- `SHA256SUMS` 驗證通過。
- `lexicon-manifest.json` 裡的 artifact URL 指向該 release tag。
- 常見測試詞如 `測試輸入`、`輸入法`、`台灣`、`人工智慧`、`小紅書` 存在於 `unigrams.current`。
- Smart Mandarin 標點查表 key 存在於 `unigrams` 與 `Mandarin-bpmf-cin`，至少包含 `_punctuation_< -> ，`、`_punctuation_Standard_< -> ，`，且 `_punctuation_list` 至少 50 筆。
- `prepopulated_service_data` 存在 `canned_messages`、`canned_messages_timestamp`，plist payload 長度大於 1000，timestamp 是正整數；且不得包含 obsolete OneKey keys：`onekey_services`、`onekey_services_timestamp`。
- Module CIN tables 存在且有合理 row count：`Generic-cj-cin`、`Generic-simplex-cin`、`Punctuations-cj-halfwidth-cin`、`Punctuations-cj-mixedwidth-cin`、`BopomofoCorrection-bopomofo-correction-cin`。
