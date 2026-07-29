# 千秋輸入法綜合詞庫

[English](README.en.md)

<img width="256" height="256" alt="chiakey-lexicon-icon" src="https://github.com/user-attachments/assets/222e1ddb-65b4-419c-88df-1f10b841ef49" />

千秋輸入法綜合詞庫（ChiaKey Lexicon）是[千秋輸入法（ChiaKey）](https://github.com/chiakich/ChiaKey)衍生的台灣用詞詞庫專案。該輸入法專案將專注在輸入法本體，本詞庫則負責持續演進的外部詞庫資料同步、轉換，實驗性的自製語料處理，以及持續定時收集的網路熱門詞。

> 歡迎贊助與支持！您的贊助將支持本詞庫的持續開發、更新與維護。

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/A0A21UAIV9)  
(適用台灣以外的贊助方式)

[綠界贊助連結](https://p.ecpay.com.tw/2A3B186)  
(僅適用台灣信用卡 / Apple Pay / ATM / 超商代碼)

## 回報詞庫問題

- [缺詞回報](https://github.com/akira02/ChiaKey-Lexicon/issues/new?template=add-unigram.yml)：回報一個應作為單獨詞語、但目前詞庫缺少的詞，例如「泳鏡」。
- [長句選字錯誤](https://github.com/akira02/ChiaKey-Lexicon/issues/new?template=add-bigram.yml)：回報兩個已存在詞的前後組合在長句中選錯字，例如想輸入「天意難測」卻出現「天意南側」。

## 為什麼有這個專案

[千秋輸入法（ChiaKey）](https://github.com/chiakich/ChiaKey) 作為「Yahoo奇摩輸入法」的開源後繼者，該輸入法主要需要依賴注音表、單字詞頻表（unigram）、以及二元語法表（bigram）。而繁體中文 / 注音的開源詞庫資源雖然相當豐富，但幾乎都集中在單詞組詞組 + 頻率這一種形式：

- 新酷音（libchewing）的 `tsi.csv` 是「詞組, 頻率, 注音」。
- Rime 共享的 `essay.txt` 是「詞, 頻率」。

這類資料能告訴你「哪個詞比較常用」，卻難以描述詞與詞之間的接續關係（打完 A 之後，接 C 是否比 B 更合理），要做到這點，需要依賴二元語法表（bigram，或者轉移機率），這恰好是同音歧義與自動選字最吃重的資訊。台灣開源注音生態長期受限於 n-gram 推論詞庫的匱乏，雖然網路上不乏靜態文本，但能精準反映現代台灣本土語境與日常口語的高品質對話語料卻極度稀缺，這導致傳統統計模型容易面臨語境偏差與選字失準；同時，n-gram 權重表依賴龐大且複雜的資料清洗、機率計算與二進位模型（如 .gram 或 .klm）編譯管線，無法像傳統單詞庫（Unigram）那樣透過簡單修改純文字檔來快速新增時事熱詞，難以長期維持一個持續迭代的台灣繁體推論模型。

千秋輸入法綜合詞庫的目標是：嘗試融合成熟的 unigram 詞庫，並在此在之上，疊加各種自製的 bigram 資料（來自網路語料、Mozilla Common Voice 句料與大語言模型合成語料），並以可重現、可追蹤來源的 pipeline 產生輸入法可直接消費的 release DB。

### bigram 資料怎麼來

最初的做法是請語言模型寫出整段擬真文本，再從文本萃取詞與詞的搭配關係，靠語料規模去覆蓋
真實的搭配分布。這條路做了兩個版本（`chiaki-synthetic-overlay`，以及後來用本機 Gemma 4
列舉撞碼詞、請模型補上下文的版本），都在評估工具建立之後被判定失敗。模型推測的搭配
幾乎全部落在真實用法之外，在 32.6 萬句測試語料裡只命中約 200 次。

現在的 bigram 主力是 `chiaki-tw-homophone-bigram`，改成直接從真實台灣文本抽取：政府機關
新聞、立法院公報詢答逐字稿、以及 PTT 語料，合計 3.67 億字。

#### 只收「會改變結果」的配對

這一輪最重要的認知是：bigram 對注音輸入法的唯一作用，是在使用者實際會打出的那個讀音上，
把落後的同音候選推到前面。如果某個詞在自己的常用讀音上本來就是最高權重候選，walker 已經
會選它，那筆 bigram 不會改變任何結果，只是體積。

以這個判準回頭檢視舊的合成語料層，46,822 列中只有 11.6% 落在能改變結果的位置。新的一層
在產生階段即強制這個條件，230,993 列全部落在撞碼位置。

#### 用真實輸入正確率當閘門

本專案建立了一套 held-out 評估：在真實文本上斷詞，逐個相鄰詞對判定「沒有 bigram 時
walker 會不會選錯」，再量測一個 bigram 層修對幾個位置、又把幾個原本正確的位置搶錯。評估會重播
release 的 calibration，並依 [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md)
的可達性分類只計入恆生效的資料列。

測試集分四個語域：政府新聞（書面）、立法院公報（正式口語）、PTT（論壇），以及作者與其朋友的群組對話（成員皆已同意作為 benchmark 使用，該語料僅用於量測，不進入訓練，亦不在本專案散布）。

分語域量測是必要的。僅以公共事務語料訓練的版本，在書面語域淨值 +85,128，看起來很好，但
在真實訊息語域只有 +65：修對 1,603、搶錯 1,538，幾乎完全抵銷。加入 PTT 語料後訊息語域
提升約 65 倍。只看單一指標無法發現這件事。

#### 從語料到權重

`bigrams.tsv` 的格式是 `qstring<TAB>previous<TAB>current<TAB>probability`，並允許句界列
（一側留空，以 `!` / `$` 標記）。`probability` 是來源內部的強度序，不是條件機率。

匯入時以 unigram 為錨進行校準（`src/importers.rs`，`calibrate_bigram_boost`）：

```
stored = min( unigram(current) + boost + (raw − raw_max_of_source), −0.05 )
```

`boost` 預設 1.5，各來源可用自己的環境變數覆寫（設為 0 則 raw 值直通）。
`raw − raw_max_of_source` 這一項保留了來源自身的信心排序，同時把整組權重錨定到 unigram
基準上：足夠強的 disambiguation 邊會高於逐字 unigram 路徑而生效，弱邊則落在基準線以下
保持 inert。

各層的完整方法、實測數字與已知限制寫在
[sources/chiaki-tw-homophone-bigram/README.md](sources/chiaki-tw-homophone-bigram/README.md)
的研究附錄。

## 致謝

本專案建立在許多優秀開源詞庫與社群多年累積之上，謹此致謝：

- **新酷音 / libchewing**（`chewing/libchewing-data`）：提供主要的現代繁中 / 注音詞彙與明確讀音基底。
- **Rime / 中州韻**（`rime/rime-essay`）：提供高品質詞頻與斷詞證據，是候選 rerank 與補充詞的重要依據。
- **Mozilla Common Voice / OpenFormosa**：bigram 句料的語料來源。
- **立法院議事暨公報資訊網 / g0v `ly.govapi.tw`**：公報詢答逐字稿，bigram 的即席口語語料來源。
- **行政院、大陸委員會、中央研究院、客家委員會、新北市政府**：依政府資料開放授權條款釋出的新聞發布，bigram 的書面語料來源。
- **`yuhuanstudio/PTT-pretrain-zhtw`**（Apache-2.0）：PTT 論壇語料，bigram 的日常語域來源。
- **Mozc**：顏文字預載分類資料。
- **教育部《重編國語辭典修訂本》／`g0v/moedict-data`**：`chiaki-modern-overlay/reading-supplements.tsv` 破音字讀音補充的資料來源，依教育部「創用CC－姓名標示－禁止改作 3.0 台灣」授權條款重製讀音；姓名標示：中華民國教育部（終身教育司）。

我們的工作主要是把這些前人的成果，整合成可重現、可追蹤來源的現代輸入法詞庫。各來源的授權、整合決定與風險紀錄詳見 [Docs/SourceReview.md](Docs/SourceReview.md)。

更多說明請見：

- 詞庫釋出流程： [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)
- 來源審查： [Docs/SourceReview.md](Docs/SourceReview.md)
- ChiaKey 走訪器實作說明：[Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md)
- Bigram 可達性稽核與剪枝：[Docs/BigramPruning.zh-TW.md](Docs/BigramPruning.zh-TW.md)

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

目標：由專案維護詞庫資料。

- `chiaki-auto-hotwords-overlay`：自動刷新 hotwords overlay（僅保留專案輸出 rows）。
- `chiaki-symbols-overlay`：補 `_punctuation_list` 缺漏符號與 runtime 標點候選。
- `chiaki-web-overlay`：網路用語 unigram/bigram 補充。
- `chiaki-tw-homophone-bigram`：從政府新聞、立法院公報與 PTT 語料萃取的 bigram rows，只收「在常用讀音上會輸給同音對手」的配對。目前的 bigram 主力層。
- `chiaki-modern-overlay`：專案自有 unigram 補充、精準覆蓋與 bigram 修正；其中保留原 `chiaki-synthetic-overlay` 的 provenance tags。
- `openformosa-common-voice-25-zh-tw`：從 Common Voice 句料挑選的 bigram rows。
- `tw-ly-transcript`：從立法院公報詢答逐字稿萃取、經人工複核的 bigram rows。已於 2026-07-29 整層停用，資料與研究紀錄保留供追溯。

### 校正層

目標：把外部證據轉成預設繁中（zh-TW）輸出期待，並抑制已知斷詞風險。

- `chiaki-rime-conversion-policy`：OpenCC `t2tw` 後的 Rime 例外規則，只保留地名 `里`、食物詞 `里肌` 等 `t2tw` 無法安全判斷的專案偏好。
- `chiaki-fragment-denylist`：句段碎片權重上限（降低偷字造成的錯誤斷詞），在 explicit overlay 後最後套用並優先於一般排序調整。

## 整合方式

Release builder 的整合流程是具有確定性的：

1. 先驗證每個必要 source file 存在，並為「相容性基底詞庫」與「外部詞庫」中有 vendored/pinned upstream 檔案的 source 產生 `source-inventory.sha256`。
2. 複製 `keykey-boneyard-bootstrap` 的 cooked `KeyKeySource.db` 作為基底。
3. 匯入 `libchewing-data`，以明確注音資料補強現代詞彙；libchewing phrase 會替換 bootstrap 中同詞的舊推導資料。
4. 匯入 `bpmf-ext-cin`，只補缺少的單字讀音，不覆蓋既有資料。
5. 將 Rime essay phrase 批次套用 OpenCC `t2tw`，再讀取 `chiaki-rime-conversion-policy` 套用少量後處理例外；normalized 結果會在 Rime rerank 與 supplemental 匯入之間共用。
6. 套用 `rime-essay` rerank：同音候選只允許有限幅度提升，既有弱詞可用 Rime 分數與切分證據有限度升權；單字同音群會在 Rime 單字頻率有足夠優勢時小幅重排；接著只加入目前 DB 尚無、且能安全推得注音的補充詞。
   - supplemental phrase 的 `split-rerank` 只作為保守輔助：若 Rime base 與最佳既有切分差距太大，不升權；若可升權，也只允許 bounded boost，避免像 `的`+`是` 這類高頻切分把整個同音 qstring（例如 `地市`、`的事`）拉平成同權重。
7. 匯入 `chiaki-web-overlay/unigrams.tsv`、`chiaki-modern-overlay/unigrams.tsv` 與 auto-hotwords，並以 phrase evidence 補強單字讀音。
8. 由 OpenCC `t2tw` 產生同 qstring variant 權重上限，並套用 Rime 單字同音 rerank。
9. 匯入 `chiaki-modern-overlay/explicit.tsv`，處理專案自有且需要指定 qstring 或排序的精準修正；它覆蓋所有一般 unigram 來源與前述校正。
10. 最後套用 `chiaki-fragment-denylist`，把偷字的非詞彙碎片壓到安全界；這個安全上限優先於 explicit overlay。
11. 依序匯入 bigram 來源：`openformosa-common-voice-25-zh-tw`、`chiaki-tw-homophone-bigram`、`chiaki-web-overlay`、`chiaki-modern-overlay`。後匯入者覆蓋前者的重疊 rows，因此人工審查過的 web overlay 與人工修正 overlay 位於語料統計來源之上。
12. 補入 runtime compatibility data：BPMF 標點、ChiaKey supplemental symbol list、canned messages、Mozc 顏文字、module CIN tables。
13. 從最終 `unigrams` 派生 `associated_phrases`，供聯想詞提示使用。
14. 執行 runtime-required validations，寫出 normalized TSV、release metadata、manifest 與 checksums。

另外，release builder 會從整合完成的 `unigrams` 派生 `associated_phrases` runtime table。這張表不是獨立詞源，而是提供「聯想詞提示」使用的 head-character -> phrase-tail 候選，例如輸出 `我` 後可提示 `們`、`的` 等詞尾。

整合後，每筆可追蹤的詞庫 row 會帶有 source path、source kind、checksum 與 tags；輸入法端消費的是最後生成的 `ChiaKeySource.db` 和 `lexicon-manifest.json`，維護端可在本機 build 後從 generated `normalized/smart-mandarin.tsv` 和 metadata 回查來源。

各來源的授權、redistribution decision 與風險紀錄放在 [Docs/SourceReview.md](Docs/SourceReview.md)。日常 release 操作放在 [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)。

## 授權政策

Rust release tooling 與 repository scripts 使用 MIT License；見 [LICENSE](LICENSE)。

詞庫資料沒有單一 repository-wide license。

每個 source 都必須在公開 release 前宣告自己的 license。未知授權資料只能做本機實驗，不可包含在 public release artifacts。

對於本專案製作的 `chiaki` 系列實驗性詞庫與清單為開放資料集，預設採用 CC BY-NC 4.0 授權條款釋出。

歡迎學術研究與個人非營利專案自由使用，使用時請標示原作者姓名。

商業用途（Commercial Use）：
若您的專案涉及商業營利行為（例如：整合至付費產品、商業應用 API、企業內部使用等），則不在上述授權範圍內。如需商用，請透過以下方式與我聯繫，討論商業授權事宜。

聯絡信箱：maid@chiaki.ch

## 後續工作

- 清理 unigram 層的組合式條目。詞庫含約 6,300 個 rime-essay 衍生的「已知詞＋方位／虛詞」條目（`電影裡`、`適當的` 這類），其中 291 個在自己的讀音上壓過正確候選（`適當的` 壓過 `適當地`、`集中了` 壓過 `擊中了`）。這類條目讓整詞路徑勝出，bigram 層無從介入。清理之後 bigram 層需要重新產生。
- 剪除 `chiaki-tw-homophone-bigram` 的 B 類（永不勝出）15,358 列，以及新發現的 E 類（整詞路徑勝出）死列，判準見 [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md)。
- 補回因 `previous` 多音而整批棄用的 767,923 筆候選，需要讀音消歧。
- 依實際缺漏加入台灣現代用語。
- 依真實打字測試調整跨來源權重映射。
- 若外部詞庫變動時，需重新檢查 LGPL 再散布要求。
- 研究是否有辦法納入如教育部、國教院等詞庫。
