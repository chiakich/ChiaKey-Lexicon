# 千秋輸入法綜合詞庫

[English](README.en.md)

千秋輸入法綜合詞庫（ChiaKey Lexicon）是[千秋輸入法（ChiaKey）](https://github.com/chiakich/ChiaKey)衍生的詞庫專案。

主輸入法專案將專注在輸入法本體，這個 repository 則負責持續演進的詞庫資料、來源 manifest、授權等。

## 分工

`ChiaKey` 負責：

1. macOS IMK runtime。
2. 輸入引擎整合。
3. 資料庫 schema 與 reader。
4. 可消費此 repo release artifacts 的 builder 或安裝 script。
5. bundled fallback `ChiaKeySource.db`。

`ChiaKey-Lexicon` 負責：

1. source manifests。
2. source license 與 attribution records。
3. vendored raw lexicon sources。
4. release-ready `ChiaKeySource` database artifacts。
5. checksums 或 signatures。
6. lexicon release changelog。

合併到 `main` 後，會透過 GitHub Actions 建置並發布版本化詞庫 release。

更多說明請見：

- [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)
- [Docs/SourceReview.md](Docs/SourceReview.md)
- [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md)

若要建立本機檢查用 package 請執行：

```sh
cargo run --release -- prepare-release
```

`prepare-release` 需要本機可執行 OpenCC CLI，Rime essay 匯入時會先套 `t2tw`，再套專案保留的少量例外規則。可用 `OPENCC_BINARY` 與 `OPENCC_T2TW_CONFIG` 覆寫預設的 `opencc` / `t2tw.json`。

合併到 `main` 後，會透過 GitHub Actions 建置並發布版本化詞庫 release。

## 架構

這個 repository 以可重現的資料 pipeline 為核心：

1. `sources/<source-id>/` 放每個已審查 input source 與本地 README；`source-inventory.sha256` 只在「相容性基底詞庫」與「外部詞庫」中維護，用於 vendored/pinned upstream 檔案的 provenance。
2. 授權檔放在各自 `sources/<source-id>/`，以 source-local 方式管理每個可公開 release source 所需的 license text 或 license notes。
3. `src/` 是 Rust release toolchain，負責驗證 inputs、將資料層匯入 KeyKey database shape、寫出 generated audit artifacts、更新 release metadata、產生 manifests。
4. `normalized/smart-mandarin.tsv` 是 Smart Mandarin language-model rows 的 generated normalized audit view，不 commit。
5. `manifests/lexicon-manifest.json` 是輸入法端消費的 generated update contract，不 commit；發版時會複製到 `dist/`。
6. `dist/dev/` 或 `dist/<version>/` 是本機 release artifacts staging 目錄，不 commit。

資料層大致分成四類：

1. 相容性基底詞庫：原輸入法既有的 database reader 與 input modules 需要的 KeyKey-origin data。
2. 外部詞庫：現代繁中 / 注音詞彙，以及補充字詞 coverage。
3. 專案詞庫：小型 overlay，用來修已知輸入缺漏、指定讀音、調整候選排序。
4. 校正層：小型已審查規則，讓預設繁中 release 符合輸入法的語言與地區期待。

## 資料層

這個 repository 的資料不是以「單一 source 清單」來看，而是分成四個資料層。release builder 會按固定順序疊加，避免互相覆蓋造成不可追蹤。

### 相容性基底詞庫

目標：維持 ChiaKey runtime、既有 schema 與模組表的相容性。

- `keykey-boneyard-bootstrap`：release DB 初始基底（cooked `KeyKeySource.db`）。
- `keykey-punctuations-cin`：BPMF 標點與 `_ctrl_*` 相容資料。
- `keykey-module-cin`：`Generic-cj-cin`、`Generic-simplex-cin`、倉頡標點、`BopomofoCorrection-bopomofo-correction-cin`。
- `keykey-prepopulated-service-data`：`canned_messages` 與 timestamp。
- `bpmf-ext-cin`：補單字 `(reading, character)` coverage。

### 外部詞庫

目標：提供可審查、可再散布的外部詞彙與讀音覆蓋。

- `libchewing-data`：主要現代繁中/注音詞庫層。
- `rime-essay`：低優先補充詞與 rerank 證據層。
- `mozc-emoticon-data`：補 `顏文字` 預載分類。

### 專案詞庫

目標：由專案維護、直接反映 ChiaKey 使用情境的詞庫資料。

- `chiakey-modern-overlay`：專案自有修正詞與 explicit 讀音/排序調整。
- `chiaki-web-overlay`：人工審核後的網路用語 unigram/bigram 補充。
- `chiaki-synthetic-overlay`：合成語料提煉的 unigram/bigram 補充。
- `openformosa-common-voice-25-zh-tw`：從 Common Voice 句料挑選的 bigram rows。
- `chiakey-auto-hotwords-overlay`：自動刷新 hotwords overlay（僅保留專案輸出 rows）。
- `chiakey-symbols-overlay`：補 `_punctuation_list` 缺漏符號與 runtime 標點候選。

### 校正層

目標：把外部證據轉成預設繁中（zh-TW）輸出期待，並抑制已知斷詞風險。

- `chiakey-rime-conversion-policy`：OpenCC `t2tw` 後的 Rime 例外規則，只保留地名 `里`、食物詞 `里肌` 等 `t2tw` 無法安全判斷的專案偏好。
- `chiakey-fragment-denylist`：句段碎片權重上限（降低偷字造成的錯誤斷詞）。


## 整合方式

release builder 的整合流程是 deterministic 的：

1. 先驗證每個必要 source file 存在，並為「相容性基底詞庫」與「外部詞庫」中有 vendored/pinned upstream 檔案的 source 產生 `source-inventory.sha256`。
2. 複製 `keykey-boneyard-bootstrap` 的 cooked `KeyKeySource.db` 作為基底。
3. 匯入 `libchewing-data`，以明確注音資料補強現代詞彙；libchewing phrase 會替換 bootstrap 中同詞的舊推導資料。
4. 匯入 `bpmf-ext-cin`，只補缺少的單字讀音，不覆蓋既有資料。
5. 將 Rime essay phrase 批次套用 OpenCC `t2tw`，再讀取 `chiakey-rime-conversion-policy` 套用少量後處理例外；normalized 結果會在 Rime rerank 與 supplemental 匯入之間共用。
6. 套用 `rime-essay` rerank：同音候選只允許有限幅度提升，既有弱詞可用 Rime 分數與切分證據有限度升權；單字同音群會在 Rime 單字頻率有足夠優勢時小幅重排；接著只加入目前 DB 尚無、且能安全推得注音的補充詞。
7. 匯入 `chiakey-modern-overlay/phrases.tsv`，讓專案自有修正可以替換已知問題詞。
8. 匯入 `chiakey-modern-overlay/explicit.tsv`，處理需要指定 qstring 或排序的精準修正。
9. 匯入 `chiaki-web-overlay/explicit.tsv` 與 `chiaki-synthetic-overlay/unigrams.tsv`。
10. 由 OpenCC `t2tw` 產生同 qstring variant 權重上限，降低不符合預設繁中期待、且已有台灣標準 counterpart 的候選；再套用 `chiakey-fragment-denylist`，把偷字的非詞彙碎片壓到安全界。
11. 匯入 `chiaki-synthetic-overlay/bigrams.tsv`、`openformosa-common-voice-25-zh-tw/bigrams.tsv`，再匯入 `chiaki-web-overlay/bigrams.tsv`，讓 reviewed web bigrams 可以覆蓋重疊的統計來源 rows。
12. 補入 runtime compatibility data：BPMF 標點、ChiaKey supplemental symbol list、canned messages、Mozc 顏文字、module CIN tables。
13. 從最終 `unigrams` 派生 `associated_phrases`，供聯想詞提示使用。
14. 執行 runtime-required validations，寫出 normalized TSV、release metadata、manifest 與 checksums。

另外，release builder 會從整合完成的 `unigrams` 派生 `associated_phrases` runtime table。這張表不是獨立詞源，而是提供「聯想詞提示」使用的 head-character -> phrase-tail 候選，例如輸出 `我` 後可提示 `們`、`的` 等詞尾。

整合後，每筆可追蹤的詞庫 row 會帶有 source path、source kind、checksum 與 tags；輸入法端消費的是最後生成的 `ChiaKeySource.db` 和 `lexicon-manifest.json`，維護端可在本機 build 後從 generated `normalized/smart-mandarin.tsv` 和 metadata 回查來源。

各來源的授權、redistribution decision 與風險紀錄放在 [Docs/SourceReview.md](Docs/SourceReview.md)。日常 release 操作放在 [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)。

## Repository 目錄

```text
Docs/
  ReleaseFlow.zh-TW.md
  SourceReview.md
src/
  main.rs
manifests/
  lexicon-manifest.example.json
normalized/
  .gitkeep
schemas/
  lexicon-manifest.schema.json
sources/
  .gitkeep
```

建置完成的 release artifacts 不會 commit 進 git。請用 `dist/` 之類的本機 staging 目錄，再由 GitHub Releases 發布 artifacts。

若要更新 pinned 外部來源，可由維護者手動執行：

```sh
cargo run --release -- fetch-modern-sources
```

這個指令會更新 vendored raw source snapshots 與 source inventories；一般 CI release build 不需要網路下載來源資料。

## Release 內容

GitHub Release 應發布：

```text
ChiaKeySource-YYYY.MM.N.db
ChiaKeySource-YYYY.MM.N.json
lexicon-manifest.json
SHA256SUMS
```

輸入法端應下載並驗證 `lexicon-manifest.json`，再把相容的 `ChiaKeySource` database 安裝到：

```text
~/Library/Application Support/ChiaKey/Lexicons/
```

runtime 載入資料庫時，若 active external database 不存在、無效或不相容，應 fallback 到 bundled database。

## 授權政策

Rust release tooling 與 repository scripts 使用 MIT License；見 [LICENSE](LICENSE)。

詞庫資料沒有單一 repository-wide license。

每個 source 都必須在公開 release 前宣告自己的 license。未知授權資料只能做本機實驗，不可包含在 public release artifacts。

本專案研究製作的 `chiaki` 系列詞庫與清單，預設採用 CC BY-NC 4.0 授權條款釋出。

歡迎學術研究與個人非營利專案自由使用，使用時請標示原作者姓名。

商業用途（Commercial Use）：
若您的專案涉及商業營利行為（例如：整合至付費產品、商業應用 API、企業內部使用等），則不在上述授權範圍內。如需商用，請透過以下方式與我聯繫，討論商業授權事宜。

聯絡信箱：maid@chiaki.ch
