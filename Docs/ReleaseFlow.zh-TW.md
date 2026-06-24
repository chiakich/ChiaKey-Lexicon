# 詞庫 Release 流程

這份文件記錄千秋輸入法詞庫（ChiaKey Lexicon）目前的 release 流程。原則是：日常開發走 `dev`，只有準備發版時才合併到 `main`，而 `main` 每次更新都會產生一版新的詞庫 release。

## Branch 角色

- `dev`：預設分支。日常調整詞庫來源、overlay、builder、文件與 CI 都先進這裡。
- `main`：release 分支。合併到 `main` 代表要發一版新的公開詞庫。
- feature branch：較大的實驗或調整可以先從 `dev` 分出，完成後開 PR 回 `dev`。

## 文件分工

- 專案架構與資料管線總覽放在 [README.md](../README.md)。
- 來源授權、redistribution decision、source inventory 說明放在 [SourceReview.md](SourceReview.md)。
- 這份文件只記錄 release branch、CI/CD、建置與發布流程。

## 本機建置

建 release package：

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
4. 執行 `cargo run --release -- prepare-release`。
5. 驗證 `SHA256SUMS`。
6. 用 SQLite smoke tests 確認 release DB 符合 app 端需要的基本合約。
7. 建立 GitHub Release 並上傳 DB、metadata、manifest、checksum。

`dist/` 是本機與 CI 的 staging 目錄，不 commit 進 git。公開 artifacts 以 GitHub Release 為準。

## 更新外部來源

外部來源以 pinned raw snapshot 形式 commit 在 `sources/*/raw/`。一般本機
build 和 CI release 不需要網路下載來源資料。

若要升級 libchewing、Rime essay 或 Mozc 顏文字等 pinned source，維護者先
更新 `src/config.rs` 裡的 URL / checksum，再執行：

```sh
cargo run --release -- fetch-modern-sources
```

這會重新下載 raw files、驗證 SHA-256，並更新對應的
`source-inventory.sha256`。更新後應把 raw source snapshot、inventory、
license 變更與 builder 變更一起 commit。

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
- `lexicon-manifest.json` 裡的 artifact URL 指向該 release tag，且 manifest version 與 release tag 一致。
- GitHub Actions 的 release job 完整通過，包括 Rust tests、artifact build、checksum validation、SQLite smoke tests。
- 若有 app 端相容性變更，使用新版 app 在乾淨 profile 裡確認 manifest 下載、DB 驗證、安裝與 fallback 行為。
