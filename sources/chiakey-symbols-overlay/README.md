# ChiaKey 補充符號清單

## 來源代號

`chiakey-symbols-overlay`

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

## Release 匯入規則

每個通過檢查的符號會寫成：

```text
_punctuation_list<TAB>symbol
```

若符號已存在於 Yahoo 原始列表，則跳過以維持原有順序。

## 上游與授權

此層為專案自有資料。

授權：CC0-1.0

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
