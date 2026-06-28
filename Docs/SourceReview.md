# 來源審查

最後審查日期：2026-06-23

## v1 已納入

### keykey-boneyard-bootstrap

- 名稱：KeyKey Boneyard bootstrap data
- 本地 release input：`sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db`
- 上游封存：<https://github.com/vChewing/KeyKey-Boneyard>
- 目前 fork 註記：<https://github.com/akira02/ChiaKey>
- 授權：BSD-3-Clause 風格的 Yahoo! KeyKey 上游授權
- 署名：Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
- 再散布決策：納入第一版公開 seed release

這個 repository 只 vendor release builder 需要的 cooked bootstrap database：

```text
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db.sha256
```

產生該 bootstrap database 的 source files 限於可再散布的 KeyKey Boneyard inputs：

- `YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Addendum/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Overrides/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Modern/*.txt`

產生的 source inventory 存放於：

```text
sources/keykey-boneyard-bootstrap/source-inventory.sha256
```

manifest 記錄的是該 inventory file 的 SHA-256，而不是單一 upstream archive 的 SHA-256。

`source-inventory.sha256` 保留作為 vendored cooked database 的 provenance。完整 KeyKey Boneyard tree 不會複製到這個 repository。

## 自 2026.06.2 起納入

### chiakey-modern-overlay

- 名稱：ChiaKey modern overlay phrases
- 本地來源：
  - `sources/chiakey-modern-overlay/phrases.tsv`
  - `sources/chiakey-modern-overlay/explicit.tsv`
- 授權：CC0-1.0
- 署名：ChiaKey Lexicon maintainers
- 再散布決策：納入公開 release

這個來源刻意保持小型且由專案自有維護。它用於實測時發現的明顯 seed lexicon 缺漏，例如不應等未來大型 frequency corpus 才補上的基本輸入法用語。

`phrases.tsv` 讓 release builder 從單字資料推導 readings。`explicit.tsv` 則用於依賴特定 KeyKey qstring 的修正，例如為 neutral-tone `ㄍㄜ˙` / `ek7` 提升 `個`。

## 自 2026.06.3 起納入

### libchewing-data

- 名稱：libchewing-data Traditional Chinese Zhuyin dictionary
- 本地來源：`sources/libchewing-data/raw/`
- 上游 release：<https://github.com/chewing/libchewing-data/releases/tag/v2026.3.22>
- 目前上游 home：<https://codeberg.org/chewing/libchewing-data>
- 授權：LGPL-2.1-or-later
- 署名：libchewing Core Team
- 再散布決策：自 `2026.06.3` 起納入公開 release

release builder 匯入這些 pinned files：

- `dict/chewing/tsi.csv`
- `dict/chewing/alt.csv`
- `dict/chewing/word.csv`

`tsi.csv` 與 `alt.csv` 會作為主要現代詞彙層匯入，因為它們包含明確注音讀音。對 libchewing-data 中存在的詞，builder 會用 libchewing 的明確讀音取代 bootstrap database 中較舊的推導讀音。`word.csv` 只用來補缺少的單字讀音。

自 `2026.06.5` 起，`tsi.csv` 中的單字 rows 也會匯入作為 character-frequency correction layer。這讓 `我` 等常用字保留 libchewing 頻率，而不是與 bootstrap database 中同讀音的罕用字 tie。
character-frequency mapping 會保留一個小型 single-character segmentation penalty，避免常用字意外超過同讀音的明確詞彙 rows。
較低排序的 libchewing 多字 rows 也會取得 bounded segment bonus，讓 `地基`、`權重` 這類已知詞能高於同讀音的逐字切分，同時不把原本已強的詞推過 phrase scale 的上緣。
當多個高頻 libchewing phrase 都以同一個字與同一讀音開頭時，weak single-character readings 也可被提升。這能讓 `數` / `ㄕㄨˋ` 這類讀音依據 `數位`、`數學`、`數量`、`數字` 等詞的證據出現在候選列表，同時仍保持在最強 phrase evidence 之下。

最終 release database 也會從組裝完成的 `unigrams` table 派生 `associated_phrases`，供 runtime associated-phrase module 使用。每列會把已 commit 的 head character 映射到 comma-separated phrase tails，所以 commit `我` 後可以提示 `們`、`的` 等 tails。這張表會在所有 lexical imports 與 policy layers 套用後產生，release 完成前也會驗證代表性 rows。

raw files vendored 於 `sources/libchewing-data/raw/`，所以 release builds 不需要 network access。maintainers 可用以下指令刷新 pinned snapshots：

```text
cargo run --release -- fetch-modern-sources
```

產生的 source inventory 存放於：

```text
sources/libchewing-data/source-inventory.sha256
```

### rime-essay

- 名稱：Rime essay shared vocabulary and language model
- 本地來源：`sources/rime-essay/raw/essay.txt`
- 上游 commit：<https://github.com/rime/rime-essay/tree/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed>
- 授權：LGPL-3.0
- 署名：Rime essay contributors
- 再散布決策：自 `2026.06.3` 起納入公開 release

Rime essay 有可用的現代詞彙與分數，但不包含注音讀音。因此 release builder 只會在另一個來源已提供讀音後使用它。

Rime essay phrase 進入 rerank 與 supplemental 匯入前，會先以 OpenCC `t2tw` 批次正規化為台灣標準繁體。`sources/chiakey-rime-conversion-policy/replacements.tsv` 只作為 `t2tw` 後的專案例外表，保留 OpenCC 無法安全判斷的地名 `里` 與食物詞 `里肌` 等偏好，不再維護一般異體字轉換清單。

首先，overlap rerank pass 會用 Rime scores 提升同一 KeyKey qstring group 內的既有候選。這個 pass 只會把較低排序的候選提升到足以尊重 Rime ordering，不會 demote 既有 rows，也會限制 promotion，避免 Rime 把 ambiguous candidate 推過既有高頻詞範圍。

另有 single-character homophone rerank pass 會處理 libchewing 單字頻率對同音字近乎攤平的問題。它只在同一單字 qstring group 內比較 Rime essay 單字頻率，而且 Rime winner 與目前 top candidate 都必須有 Rime 單字頻率；預設 winner 至少要有 `5x` 頻率優勢，才會被小幅提升到目前 top 之上。這個 pass 只 raise，不 demote。

接著，低優先補充詞 pass 會匯入符合以下條件的 entries：

1. 該詞在 libchewing-data 匯入後尚不存在。
2. 詞長介於 2 到 7 個 Unicode codepoints。
3. Rime score 至少為 `40`。
4. 每個字在目前 database 中都有 primary single-character reading。

在這個補充 pass 中，原本會低於既有 split path 的 entries 會被提升到剛好超過該 split，並以 Rime supplemental range 的上緣為 cap。這讓 `趁現在` 這類完整詞能比 `稱` + `現在` 這種不太可能的字詞切分取得小幅 segmentation advantage，而不需要 per-phrase explicit overrides。

這能避免用推導讀音取代 libchewing 的明確注音資料，同時在讀音能安全推導到足以作為補充層時，加入社群、新聞、科技等現代詞彙。

產生的 source inventory 存放於：

```text
sources/rime-essay/source-inventory.sha256
```

## 自 2026.06.5 起納入

### bpmf-ext-cin

- 名稱：Public domain extended BPMF character table
- 本地來源：`sources/bpmf-ext-cin/vendor/bpmf-ext.cin`
- 上游來源檔：<https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-ext.cin>
- 授權：Public Domain，依來源檔 header
- 署名：opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers
- 再散布決策：自 `2026.06.5` 起納入公開 release

這個來源會在 libchewing-data 之後、Rime essay 之前匯入。release builder 只把它當作低優先的單字讀音補充：

1. 只匯入 CJK BMP 字元。
2. 排除 non-BMP 與 private-use 字元。
3. 只加入缺少的 exact `(reading, character)` pairs。
4. 不覆蓋 libchewing character frequencies。

這會補齊 native/Yahoo character coverage gaps，例如 `ㄨㄛˇ` 候選集合：

```text
我 婐 捰 倭 䂺 婑 䰀 㦱
```

產生的 source inventory 存放於：

```text
sources/bpmf-ext-cin/source-inventory.sha256
```

## 自 2026.06.6 起納入

### keykey-punctuations-cin

- 名稱：KeyKey BPMF punctuation table
- 本地來源：`sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin`
- 上游來源檔：<https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-punctuations.cin>
- 授權：BSD-3-Clause 風格的 Yahoo! KeyKey 上游授權
- 署名：Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
- 再散布決策：自 `2026.06.6` 起納入公開 release

這個來源恢復原始 KeyKey runtime punctuation lookup rows。release builder 只匯入 `%chardef` 中 key 以 `_punctuation_` 或 `_ctrl_` 開頭的 rows，並同時寫入 `unigrams` 與 `Mandarin-bpmf-cin`。

Smart Mandarin 的標點處理需要這些 rows，例如：

```text
_punctuation_<          ，
_punctuation_Standard_< ，
```

產生的 source inventory 存放於：

```text
sources/keykey-punctuations-cin/source-inventory.sha256
```

### keykey-prepopulated-service-data

- 名稱：KeyKey prepopulated service data
- 本地來源：`sources/keykey-prepopulated-service-data/vendor/`
- 上游來源目錄：<https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData>
- 授權：BSD-3-Clause 風格的 Yahoo! KeyKey 上游授權
- 署名：Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
- 再散布決策：自 `2026.06.6` 起納入公開 release

這個來源恢復原始 KeyKey 預載 canned-message payload。release builder 會把完整 `CannedMessages.plist` 內容寫入 `prepopulated_service_data` 的 `canned_messages`，並以正值 release timestamp 寫入 `canned_messages_timestamp`。

release cooking 時，builder 會用 `chiakey-symbols-overlay/symbols.tsv` 產生一組 button categories 來補強 canned-message payload。這讓可見的符號表取得與 `_punctuation_list` 相同的補充符號，但不會把所有符號塞進單一過大的分類。builder 也會用 Mozc-backed `Messages` list 取代原始帶註解的 `顏文字` 分類，讓符號表只顯示顏文字本體，而不是 `顏文字 + 中文說明` 這類字串。

`OneKey.plist` 會刻意從公開 release 省略。OneKey 是 Yahoo 時代的 URL launcher，不是輸入詞庫資料；現代 ChiaKey 也不再載入它。新的 release databases 不應包含 `onekey_services` 或 `onekey_services_timestamp`。

產生的 source inventory 存放於：

```text
sources/keykey-prepopulated-service-data/source-inventory.sha256
```

### keykey-module-cin

- 名稱：KeyKey module CIN tables
- 本地來源：`sources/keykey-module-cin/vendor/`
- 上游來源目錄：<https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/DataTables>
- 授權：BSD-3-Clause 風格的 Yahoo! KeyKey 上游授權 / Public Domain source tables
- 署名：Yahoo! Inc., OpenVanilla contributors, opendesktop.org.tw CIN contributors, KeyKey Boneyard / ChiaKey maintainers
- 再散布決策：自 `2026.06.6` 起納入公開 release

這個來源恢復 KeyKey runtime 在 Smart Mandarin language model 之外會使用的 module SQLite tables：

```text
Generic-cj-cin
Generic-simplex-cin
Punctuations-cj-halfwidth-cin
Punctuations-cj-mixedwidth-cin
BopomofoCorrection-bopomofo-correction-cin
```

產生的 source inventory 存放於：

```text
sources/keykey-module-cin/source-inventory.sha256
```

## Release builder generated policy

### opencc-variant-policy

- 來源代號：`opencc-variant-policy`
- 類型：release build generated stage，不是 `sources/` input source
- 上游參考：<https://github.com/BYVoid/OpenCC>
- 授權脈絡：Generated from OpenCC `t2tw` at release build time
- 署名：OpenCC contributors; ChiaKey Lexicon maintainers

這個 policy 不再維護手寫 TSV。release builder 會掃描目前 database 內的 unigram 候選，批次套用 OpenCC `t2tw`；只有在原詞與 `t2tw` 後的詞形同時存在於同一個 qstring group 時，才會把原詞權重壓到 `t2tw` 詞形下方。這讓 `个` / `個`、`喫` / `吃`、`爲` / `為` 等同音 variant 可以自動處理，同時避免對沒有 counterpart 的歷史、人名或相容資料做粗暴降權。

## 自 2026.06.8 起納入

### chiakey-auto-hotwords-overlay

- 名稱：ChiaKey automatically refreshed hotwords overlay
- 本地來源：
  - `sources/chiakey-auto-hotwords-overlay/phrases.tsv`
  - `sources/chiakey-auto-hotwords-overlay/state.json`
- 授權：CC0-1.0 for the generated overlay rows maintained in this repository
- 署名：ChiaKey Lexicon maintainers
- 再散布決策：納入公開 release

這個來源是自動維護的短期熱詞補充層。Google Trends 只作為 discovery signal；
daily collector 會查詢 24 小時、48 小時與 7 天 trending windows，並把最小化、
正規化後的 observations 存成 GitHub Actions artifacts。weekly refresh 才會彙整狀態
並寫出本 repository 自有的 `phrases.tsv`。

repository 不保存 Google Trends 的原始 CSV、排名表、新聞摘要或完整 related queries。
`state.json` 只保存每個候選詞的最小聚合狀態，例如 `first_seen`、`last_seen`、
`seen_dates` 與 `max_traffic`，用於自動權重與過期清理。

自動收詞規則刻意保守：

1. 只保留正規化後的純漢字詞。
2. 排除英數、5 字以上詞、查詢型詞。
3. 排除本地詞庫已存在的詞。
4. 排除已可由 top-ranked 既有 segments 打出的詞。
5. 排除無法從既有單字讀音推導 qstring 的詞。
6. 7 天 window 只作為佐證；單一 `7d` observation 不會自行進入 overlay。

這個來源的 rows 會隨時間自動衰退與移除，不應視為人工審核或永久詞彙層。

## 自 2026.06.9 起納入

### chiakey-symbols-overlay

- 名稱：ChiaKey supplemental symbol list
- 本地來源：
  - `sources/chiakey-symbols-overlay/symbols.tsv`
  - `sources/chiakey-symbols-overlay/punctuation-alternatives.tsv`
- 授權：CC0-1.0
- 署名：ChiaKey Lexicon maintainers
- 再散布決策：自 `2026.06.9` 起納入公開 release

這個來源用專案自有符號補強原始 KeyKey punctuation list，涵蓋現代文字輸入常用的 extended punctuation、貨幣符號、法律與商標符號、CJK 符號、圈號數字、羅馬數字變體、補充箭頭、數學運算與關係符號、勾叉、星號、撲克牌花色、音樂符號與單位符號。

release builder 會把 `symbols.tsv` 匯入為 `_punctuation_list` rows。它會在 `keykey-punctuations-cin` 之後載入，並跳過 Yahoo KeyKey punctuation list 已有的符號，以保留原始排序與直接標點 key mappings。

`punctuation-alternatives.tsv` 會補充既有 runtime 標點 key 的同族候選，例如 `_punctuation_[` 原本輸出 `「`，再追加 `『`、`《`、`﹁` 等開符號候選；`_punctuation_]` 則追加對應閉符號候選。這些 rows 使用較低權重，因此原始直接輸出的標點仍維持第一候選。

同一份來源也會用來在 `prepopulated_service_data/canned_messages` 中產生多個補充 canned-message button categories，因為 app 的符號表讀取 canned messages，而不是直接查詢 `_punctuation_list`。產生的分類為 `補充標點`、`貨幣與標記`、`數字序號`、`補充箭頭`、`補充數學`、`勾叉與星號`、`花色與音樂`、`單位符號`。

產生的 source inventory 存放於：

```text
sources/chiakey-symbols-overlay/source-inventory.sha256
```

### mozc-emoticon-data

- 名稱：Mozc emoticon data
- 本地來源：
  - `sources/mozc-emoticon-data/raw/categorized.tsv`
  - `sources/mozc-emoticon-data/raw/emoticon.tsv`
- 上游來源目錄：<https://github.com/google/mozc/tree/master/src/data/emoticon>
- 上游 commit：`28da5a39f9a7fd70251c85d269f4a8b47aa31cf8`
- 授權：BSD-3-Clause
- 署名：Google and Mozc contributors
- 再散布決策：自 `2026.06.9` 起納入公開 release

這個來源取代原始 KeyKey 的 `顏文字` canned-message category。release builder 會先讀取 `categorized.tsv`，再從 `emoticon.tsv` 追加不重複的 emoticon values。輸出到 `prepopulated_service_data/canned_messages` 時只使用第一欄，也就是顏文字本體；日文 reading keys、categories 與 descriptions 只保留作為 source context。

產生的分類刻意維持為一般 `Messages` list，不使用 `IsSymbolButtonList` 也不使用 `Buttons`，因為原始顏文字 UX 是 list。這能避免符號表 UI 顯示 Yahoo 時代 canned-message data 內附的舊中文註解，同時保留 list interaction。

產生的 source inventory 存放於：

```text
sources/mozc-emoticon-data/source-inventory.sha256
```

### chiaki-web-overlay

- 名稱：Chiaki reviewed web corpus overlay
- 本地來源：
  - `sources/chiaki-web-overlay/explicit.tsv`
  - `sources/chiaki-web-overlay/bigrams.tsv`
- 來源材料：經審查的 web-derived 台灣網路用語材料
- 授權：CC BY-NC 4.0；商用需取得 Chiaki.C 許可
- 署名：Chiaki.C
- 再散布決策：納入 ChiaKey 公開 release；其他專案或非 ChiaKey 用途預設應排除此來源，除非自行完成 source review

這個來源是窄範圍的 ChiaKey overlay，包含從 web usage material 得出的 reviewed unigram 與 bigram values。repository 只用 release-builder formats 再散布最終 lexicon rows：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
qstring<TAB>previous<TAB>current<TAB>probability
```

release builder 會在 `chiakey-modern-overlay` 之後匯入 unigram rows，並在 synthetic 與 Common Voice bigrams 之後匯入 bigram rows。

產生的 source inventory 存放於：

```text
sources/chiaki-web-overlay/source-inventory.sha256
```

### chiaki-synthetic-overlay

- 名稱：Chiaki.C GPT-5.5 synthetic Taiwan internet usage overlay
- 本地來源：
  - `sources/chiaki-synthetic-overlay/unigrams.tsv`
  - `sources/chiaki-synthetic-overlay/bigrams.tsv`
- 來源材料：GPT-5.5 產生的 synthetic「台灣網路用語」corpus
- 授權：CC BY-NC 4.0；商用需取得 Chiaki.C 許可
- 署名：Chiaki.C
- 再散布決策：納入公開 source review、open-source project use 與 non-commercial release builds

這個來源包含 Chiaki.C 維護的 synthetic 台灣網路用語 rows。raw synthetic corpus 不在這個 repository 再散布；这里只保留最終 lexicon rows：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
qstring<TAB>previous<TAB>current<TAB>probability
```

bigram file 會先依 release unigram table 做預先篩選，並可包含使用 `!` / `$` qstring markers 的 sentence-boundary rows。

產生的 source inventory 存放於：

```text
sources/chiaki-synthetic-overlay/source-inventory.sha256
```

### openformosa-common-voice-25-zh-tw

- 名稱：OpenFormosa Common Voice 25 zh-TW bigram overlay
- 本地來源：`sources/openformosa-common-voice-25-zh-tw/bigrams.tsv`
- 來源材料：OpenFormosa Common Voice 25 zh-TW validated sentences
- 上游 dataset：<https://huggingface.co/datasets/OpenFormosa/common_voice_25_zh-TW>
- 授權：CC0-1.0
- 署名：OpenFormosa / Mozilla Common Voice contributors
- 再散布決策：以選出的 runtime bigram rows 納入公開 release

這個來源只貢獻選出的 runtime bigram rows。raw Common Voice sentences 不在這個 repository 再散布。

產生的 source inventory 存放於：

```text
sources/openformosa-common-voice-25-zh-tw/source-inventory.sha256
```

## v1 未納入

這些來源可作為有用參考，但第一版 release artifacts 不會把它們當作 raw sources 納入：

- 歷史資料包中的 Yahoo search terms。
- Sinica Corpus raw material。
- Commercial CEROD / SQLite extension assets。
- CC-CEDICT、moedict、Wikimedia、Tatoeba、wordfreq、SUBTLEX-CH、Google Books Ngram、Google Chinese Web 5-gram。

部分繼承自開放 KeyKey Boneyard tree 的 bootstrap files 有 `Yahoo.txt` 或 `SinicaCorpusOverrides.txt` 這類歷史名稱。在 v1 中，這些檔案視為 BSD-style Boneyard bootstrap source 的一部分。repository 不會複製私有 raw Yahoo search logs、Sinica corpus files 或 CEROD binaries。

## 讀音格式

v1 normalized TSV 的第一欄使用目前 KeyKey / Manjusri 內部 `qstring` 讀音表示法。這是歷史 builder 的 `absolute_order_string` function 產生的 two-byte-per-syllable ordering string，不是字面上的注音文字。

這讓第一版 release 能直接相容目前的 database reader。若 builder contract 之後改變，後續 source-normalization pass 可以再加入 human-readable Bopomofo column。

## 目前風險註記

這個 release 仍是 seed lexicon，但已包含大幅擴充的現代繁中 / 注音層。

預期後續工作：

1. 依實際缺漏加入台灣現代用語。
2. 依真實打字測試調整跨來源權重映射。
3. release packaging 變動時重新檢查 LGPL 再散布要求。
4. 未來公開 release 若要納入 CC BY-SA 或 research-only sources，需先完成審查。
