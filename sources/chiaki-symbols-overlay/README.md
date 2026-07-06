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

`punctuation-alternatives.tsv` 會補充既有 runtime 標點 key 的候選符號，例如在
`_punctuation_[` 原本輸出 `「` 之後，追加 `『`、`《`、`﹁` 等同族開符號候選。
若 exact key/value 已存在，則跳過以維持 Yahoo 原始資料的排序與相容性。

## 上游與授權

此層為專案自有資料。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-symbols-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
