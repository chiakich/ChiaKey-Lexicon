# 千秋輸入法詞庫

[English](README.en.md)

千秋輸入法詞庫（ChiaKey Lexicon）是千秋輸入法（ChiaKey）的詞庫資料 repository。

主輸入法的 repository 應該專注在 macOS 輸入法 runtime、資料庫讀取、builder script、安裝工具，以及一份小型 bundled fallback database。這個 repository 則負責持續演進的詞庫資料、來源 manifest、授權紀錄、release database artifacts、checksums 與 changelog。

## 分工

`ChiaKey` 負責：

1. macOS IMK runtime。
2. 輸入引擎整合。
3. 資料庫 schema 與 reader。
4. 可消費此 repo release artifacts 的 builder 或安裝 script。
5. bundled fallback `KeyKeySource.db`。

`ChiaKey-Lexicon` 負責：

1. source manifests。
2. source license 與 attribution records。
3. vendored raw lexicon sources。
4. release-ready `KeyKeySource` database artifacts。
5. checksums 或 signatures。
6. lexicon release changelog。

## 目前狀態

目前 pipeline 會從已審查來源資料、專案維護修正、生成 metadata、source inventories 與 checksum manifests 建出完整的 `KeyKeySource.db`。本機檢查 build 預設會輸出到 `dist/dev/`，公開 release 的版本號則由 CI 計算後注入，並上傳到 GitHub Releases。

合併到 `main` 後，會透過 GitHub Actions 建置並發布版本化詞庫 release。

更多說明請見：

- [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)
- [Docs/SourceReview.md](Docs/SourceReview.md)
- [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md)

若要建立本機檢查用 package 請執行：

```sh
cargo run --release -- prepare-release
```

公開 release 不需要在 repo 內手動更新版號；GitHub Actions 會依既有 tag 計算下一個 `YYYY.MM.N`。

## 架構

這個 repository 以可重現的資料 pipeline 為核心：

1. `sources/<source-id>/` 放每個已審查 input source、本地 README，以及 `source-inventory.sha256` provenance file。
2. `LICENSES/` 記錄每個可公開 release source 所需的 license text 或 license notes。
3. `src/` 是 Rust release toolchain，負責驗證 inputs、將資料層匯入 KeyKey database shape、寫出 generated audit artifacts、更新 release metadata、產生 manifests。
4. `normalized/smart-mandarin.tsv` 是 Smart Mandarin language-model rows 的 generated normalized audit view，不 commit。
5. `manifests/lexicon-manifest.json` 是輸入法端消費的 generated update contract，不 commit；發版時會複製到 `dist/`。
6. `dist/dev/` 或 `dist/<version>/` 是本機 release artifacts staging 目錄，不 commit。

資料層大致分成四類：

1. **Runtime compatibility data**：原輸入法既有的 database reader 與 input modules 需要的 KeyKey-origin data。
2. **Lexicon sources**：現代繁中 / 注音詞彙，以及補充字詞 coverage。
3. **Project-owned corrections**：小型 overlay，用來修已知輸入缺漏、指定讀音、調整候選排序。
4. **Policy layers**：小型已審查規則，讓預設繁中 release 符合輸入法的語言與地區期待。

## 目前資料來源

這個 repository 目前採用的資料來源分成「相容性基底」、「現代詞庫」、「補充 coverage」與「維護政策」幾層。每個 source 都有明確責任，release builder 會按固定順序整合，避免不同來源互相覆蓋到不可追蹤。

| Source | 選用理由 | 負責工作 |
| --- | --- | --- |
| `keykey-boneyard-bootstrap` | ChiaKey 的 runtime 和 database reader 原本就建立在 KeyKey / Yahoo KeyKey 的資料形狀上；用 cooked bootstrap DB 可以保留既有 schema、metadata 與基本注音資料。 | 作為 release DB 的初始基底。builder 先複製這份 `KeyKeySource.db`，後續 sources 再疊加或替換資料。 |
| `keykey-punctuations-cin` | 標點不是一般詞彙，但 Smart Mandarin runtime 會查 `_punctuation_*`、`_ctrl_*` 等 key；缺少時輸入法端會拒絕或得到空符號表。 | 從原始 `bpmf-punctuations.cin` 匯入 BPMF 標點與符號列表，寫入 `unigrams` 和 `Mandarin-bpmf-cin`。 |
| `chiakey-symbols-overlay` | Yahoo 原始符號列表偏舊，缺少現代常用的貨幣、數學、圈號數字、勾叉、音樂與其他特殊符號；這些補充屬於 ChiaKey 自有維護資料。 | 只追加 `_punctuation_list` 候選，並跳過 Yahoo 原表已有符號，不改任何直接按鍵標點映射。 |
| `keykey-prepopulated-service-data` | canned messages 仍是 ChiaKey 會讀取的預載資料，需要跟 release DB 一起提供，並帶正值 timestamp 才不會被 user DB 空資料蓋掉。 | 寫入 `prepopulated_service_data/canned_messages` 和 `canned_messages_timestamp`。builder 會把 supplemental symbol overlay 依類型追加成多個符號表分類，並以 Mozc 顏文字資料取代原本帶說明文字的內建 `顏文字` 列表。已移除不用的 OneKey service data。 |
| `mozc-emoticon-data` | Mozc 是 Google Japanese Input 的開源版，提供乾淨、可再散布的日文 IME 顏文字資料。 | 供 canned messages 的 `顏文字` 分類使用。builder 只輸出顏文字本體，不把日文讀音或描述放進符號表列表。 |
| `keykey-module-cin` | KeyKey runtime 不只讀 Smart Mandarin 詞庫，也可能讀其他 module tables；這些表不是主要注音詞庫，但缺少會造成相容性破洞。 | 匯入 `Generic-cj-cin`、`Generic-simplex-cin`、倉頡標點表與 `BopomofoCorrection-bopomofo-correction-cin`。 |
| `libchewing-data` | libchewing-data 是活躍維護的繁中注音資料來源，包含明確注音讀音，比只靠舊 KeyKey bootstrap 推導更可靠。 | 作為主要現代詞庫層。`tsi.csv`、`alt.csv` 提供詞與替代讀音；`word.csv` 補單字讀音；單字頻率也用來修正常用字排序。 |
| `bpmf-ext-cin` | libchewing 和 bootstrap 仍可能缺少單字候選；這份 public-domain CIN 表可以補足單字 coverage。 | 只補 CJK BMP 單字的缺失 `(reading, character)` pair，不覆蓋 libchewing 或 bootstrap 既有權重。 |
| `rime-essay` | Rime essay 有較廣的現代詞彙與語言模型分數，但沒有注音讀音；適合當低優先補充與排序證據層，而不是主詞庫。 | 對既有弱詞做有限度 rerank；僅在詞尚未存在、分數達門檻、長度合理，且每個字都能從目前 DB 推得 primary reading 時匯入補充詞。 |
| `chiakey-rime-conversion-policy` | Rime essay 的詞形有時不符合預設台灣現代繁中輸出期待，例如以 `喫` 表示常用的 `吃`。這類資料仍有頻率價值，不應只靠 modern overlay 補另一筆詞。 | 在 Rime supplemental import 與 Rime rerank 前套用小型 from/to 轉換規則，把 Rime 的頻率證據移到專案偏好的輸出詞形，例如 `喫壞` → `吃壞`。 |
| `chiakey-modern-overlay` | 真實打字測試會發現少量立即需要修的缺漏或排序問題；這些修正應由專案自己維護，不能等大型來源更新。 | 補專案自有詞、指定明確 qstring，或針對已知 case 調整候選排序，例如 neutral-tone `ㄍㄜ˙` / `ek7`。 |
| `chiaki-web-overlay` | 經人工審過的網路用語 overlay，僅作為 ChiaKey 詞庫的窄補充；其他專案或非 ChiaKey 用途預設應排除，除非自行完成來源審查。 | 匯入 explicit unigram 與 runtime bigram rows；只保存最終詞庫 rows，不保存原始語料。 |
| `chiaki-synthetic-overlay` | Chiaki.C 維護的 synthetic 台灣網路用語 overlay。 | 匯入 unigram rows 與 runtime bigram probabilities；此來源採 CC BY-NC 4.0，商用請聯絡 Chiaki.C。 |
| `openformosa-common-voice-25-zh-tw` | OpenFormosa / Mozilla Common Voice 的 CC0 zh-TW validated sentences。 | 匯入選出的 runtime bigram rows；不保存原始語音句庫。 |
| `opencc-variant-policy` | 預設繁中輸入法不應讓簡體或非台灣慣用字因 tie-break 排在繁體字前面。OpenCC 可作為 variant knowledge 的參考，但不當作頻率詞典匯入。 | 用小型 policy table 降低指定 variant 的最大權重，例如讓 `个` 不會排在 `個` 前面。 |
| `chiakey-fragment-denylist` | libchewing 收錄的非詞彙碎片（助動詞/情態詞+動詞，如 `會比`、`會在`）權重過高時，會偷走鄰詞的音節形成錯誤斷詞（如 `會比較準` 被切成 `會比\|校準`）。頻率與結構都無法把碎片和真詞分開。 | 用 phrase-level 上限把清單內碎片壓到 `w(lead)+w(stolen)−0.3` 的安全界（只降不升）。清單以結構過濾 → 教育部修訂本詞目比對 → 人工 spot-check 產出；教育部詞典僅為離線建表工具，不轉載其內容。 |

另外，release builder 會從整合完成的 `unigrams` 派生 `associated_phrases` runtime table。這張表不是獨立詞源，而是提供「聯想詞提示」使用的 head-character -> phrase-tail 候選，例如輸出 `我` 後可提示 `們`、`的` 等詞尾。

## 整合方式

release builder 的整合流程是 deterministic 的：

1. 先驗證每個必要 source file 存在，並為各 source 產生 `source-inventory.sha256`。
2. 複製 `keykey-boneyard-bootstrap` 的 cooked `KeyKeySource.db` 作為基底。
3. 匯入 `libchewing-data`，以明確注音資料補強現代詞彙；libchewing phrase 會替換 bootstrap 中同詞的舊推導資料。
4. 匯入 `bpmf-ext-cin`，只補缺少的單字讀音，不覆蓋既有資料。
5. 讀取 `chiakey-rime-conversion-policy`，在 Rime phrase 被用作 rerank evidence 或 supplemental 詞之前先修正常見詞形轉換問題。
6. 套用 `rime-essay` rerank：同音候選只允許有限幅度提升，既有弱詞可用 Rime 分數與切分證據有限度升權；單字同音群會在 Rime 單字頻率有足夠優勢時小幅重排；接著只加入目前 DB 尚無、且能安全推得注音的補充詞。
7. 匯入 `chiakey-modern-overlay/phrases.tsv`，讓專案自有修正可以替換已知問題詞。
8. 匯入 `chiakey-modern-overlay/explicit.tsv`，處理需要指定 qstring 或排序的精準修正。
9. 匯入 `chiaki-web-overlay/explicit.tsv` 與 `chiaki-synthetic-overlay/unigrams.tsv`。
10. 套用 `opencc-variant-policy`，降低不符合預設繁中期待的 variant 權重，再套用 `chiakey-fragment-denylist`，把偷字的非詞彙碎片壓到安全界。
11. 匯入 `chiaki-synthetic-overlay/bigrams.tsv`、`openformosa-common-voice-25-zh-tw/bigrams.tsv`，再匯入 `chiaki-web-overlay/bigrams.tsv`，讓 reviewed web bigrams 可以覆蓋重疊的統計來源 rows。
12. 補入 runtime compatibility data：BPMF 標點、ChiaKey supplemental symbol list、canned messages、Mozc 顏文字、module CIN tables。
13. 從最終 `unigrams` 派生 `associated_phrases`，供聯想詞提示使用。
14. 執行 runtime-required validations，寫出 normalized TSV、release metadata、manifest 與 checksums。

整合後，每筆可追蹤的詞庫 row 會帶有 source path、source kind、checksum 與 tags；輸入法端消費的是最後生成的 `KeyKeySource.db` 和 `lexicon-manifest.json`，維護端可在本機 build 後從 generated `normalized/smart-mandarin.tsv` 和 metadata 回查來源。

各來源的授權、redistribution decision 與風險紀錄放在 [Docs/SourceReview.md](Docs/SourceReview.md)。日常 release 操作放在 [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)。

## Repository 目錄

```text
Docs/
  ReleaseFlow.zh-TW.md
  SourceReview.md
LICENSES/
  README.md
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
KeyKeySource-YYYY.MM.N.db
KeyKeySource-YYYY.MM.N.json
lexicon-manifest.json
SHA256SUMS
```

輸入法端應下載並驗證 `lexicon-manifest.json`，再把相容的 `KeyKeySource` database 安裝到：

```text
~/Library/Application Support/ChiaKey/Lexicons/
```

runtime 載入資料庫時，若 active external database 不存在、無效或不相容，應 fallback 到 bundled database。

## 授權政策

Rust release tooling 與 repository scripts 使用 MIT License；見 [LICENSE-CODE](LICENSE-CODE)。

詞庫資料沒有單一 repository-wide license。

每個 source 都必須在公開 release 前宣告自己的 license。未知授權資料只能做本機實驗，不可包含在 public release artifacts。
