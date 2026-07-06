# Chiaki 網路語料覆蓋層

## 來源代號

`chiaki-web-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源收錄在 repo 外部產生、經人工審核後導入的 unigram 與 bigram 資料列。

專案不散布原始語料文本，只保留可審查、可發布的最終詞庫列。

## 檔案與格式

`unigrams.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

## Release 匯入規則

- `unigrams.tsv`：在 modern overlay 後、variant demotion policy 前匯入。
- `bigrams.tsv`：在 unigram 政策層處理完後，匯入 runtime `bigrams` 表。

## 上游與授權

審核後覆蓋列由 ChiaKey Lexicon 維護者維護。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-web-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
