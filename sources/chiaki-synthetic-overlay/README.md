# 合成文本覆蓋層

## 來源代號

`chiaki-synthetic-overlay`

## 資料層

專案詞庫

## 狀態：bigrams 已停用（2026-07-29）

`bigrams.tsv` 不再匯入 release。以四個語域的 held-out 輸入正確率量測，其在
`chiaki-tw-homophone-bigram` 之上的邊際貢獻為零至微負；單獨評估亦僅書面 +377、
口語 −974、訊息 +5。主因是本來源產生時尚未建立「同音撞碼」選材判準，46,822 列中僅
11.6% 落在 bigram 能改變結果的位置。

`unigrams.tsv`（4,117 列）**仍持續匯入**，未受影響。

檔案保留供追溯，不刪除。停用點：`src/release.rs`（原 `import_chiaki_synthetic_bigrams`）。

## 用途與定位

此來源收錄由合成語料萃取並審核的 unigram 與 bigram 資料列。

專案不散布原始合成語料，只保留萃取後的最終詞庫列。

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
