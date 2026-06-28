# Chiaki.C 合成台灣網路語用覆蓋層

## 來源代號

`chiaki-synthetic-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源收錄由 Chiaki.C 維護並經審核的 unigram 與 synthetic bigram 資料列。

專案不散布原始 synthetic 語料，只保留可供 release 使用的最終詞庫列。

## 檔案與格式

`unigrams.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

句界 bigram 允許 `previous` 或 `current` 其中一側為空。

## Release 匯入規則

- `unigrams.tsv`：在 variant demotion policy 前匯入。
- `bigrams.tsv`：在 reviewed web bigrams 前匯入。

Bigram 校準公式：

```text
stored = min(unigram(current) + boost + (raw - raw_max_of_source), -0.05)
```

此機制保留來源內部排序，同時讓強 disambiguation 邊可高於 unigram 路徑；較弱配對會留在 unigram floor 下方而不生效。

- 預設 `boost`：`1.5`
- 覆寫環境變數：`SYNTHETIC_BIGRAM_BOOST`
- 設為 `0` 時：保留原始數值

## 上游與授權

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-synthetic-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
