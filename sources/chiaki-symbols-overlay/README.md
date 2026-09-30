# ChiaKey 補充符號清單

## 來源代號

`chiaki-symbols-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源收錄專案自有補充符號，作為 Smart Mandarin 標點清單的增補層。

Yahoo KeyKey 原始 `bpmf-punctuations.cin` 仍是相容基底；本層只補 `_punctuation_list` 缺漏，不改動 `_punctuation_<` 等直接按鍵映射。

## 檔案與格式

`symbols.tsv`：

```text
symbol<TAB>tags
```

`punctuation-alternatives.tsv`：

```text
qstring<TAB>symbol<TAB>tags
```

## Release 匯入規則

每個通過檢查的符號會寫成：

```text
_punctuation_list<TAB>symbol
```

若符號已存在於 Yahoo 原始列表，則跳過以維持原有順序。

`punctuation-alternatives.tsv` 會補充既有 runtime 標點 key 的候選符號，例如在`_punctuation_[` 原本輸出 `「` 之後，追加 `『`、`《`、`﹁` 等同族開符號候選。若 exact key/value 已存在，則跳過以維持 Yahoo 原始資料的排序與相容性。

tags 若含 `primary`，該列權重會高於 Yahoo 基底列（基底為 `0.0`），成為該按鍵的首選候選，原本的基底符號退居選單。例如 `_punctuation_|` 以 `、` 為首選、`｜` 退為第二候選，與其他注音輸入法的慣例一致。沒有 `primary` 的列一律排在基底列之後。

## 上游與授權

此層為專案自有資料。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-symbols-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證

## 符號說明

`symbol-metadata.json` 以完整的原始符號字串為 key，欄位為 `Name`（必要）、`Description` 與 `DisplayLabel`（選用），值均為非空字串。空白也是有效的符號 key，不得 trim。key 不得包含 `&`、`<`、`>`：已出貨的 app 重新輸出 plist 時不會 escape key，這類 key 會讓整份符號表無法載入，產生器會直接拒絕。

Release 會把說明寫入 `prepopulated_service_data/canned_messages` 的各個符號分類，新增 `SymbolMetadata` dictionary，僅附上該分類 `Buttons` 中存在的符號說明。原始 `Buttons` 字串陣列與順序不變，因此舊版 app 仍可讀取。此來源亦納入 release 的來源雜湊紀錄。

App 顯示名稱、說明及依原始字元計算的 Unicode 碼位；`DisplayLabel` 只改變按鈕外觀，點擊仍輸入原始字元。沒有 metadata 的符號使用 app 的 Unicode 名稱 fallback。此檔不會自動把符號加入 `Buttons` 或 `⌃0`，例如全形空白的說明可先備妥，再由符號來源決定是否收錄。

### 相容性 CI

Verify 與 Release 的 `cargo test` 會執行 `symbol_metadata_compatibility_with_legacy_consumers`，透過 Python 3 的 plist parser 檢查真正產製的 metadata：原始分類、Buttons 字串與順序不變，新欄位可以安全忽略，空白 key 不得變成顯示標籤。測試包括原始符號表與全形空白 fixture。執行測試需要 Python 3。

「常用符號」與「基本符號」的每個按鈕均提供中文名稱；容易混淆的點、引號、小型標點及線框另附辨識說明。相容性 CI 同時檢查這兩區的 metadata 覆蓋率，新增按鈕時必須補齊名稱。其他分類未收錄的符號仍使用 Unicode 名稱 fallback。
