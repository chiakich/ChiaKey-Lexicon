# OpenFormosa Common Voice 25 zh-TW Bigram 覆蓋層

## 來源代號

`openformosa-common-voice-25-zh-tw`

## 資料層

專案詞庫

## 用途與定位

此來源收錄從 OpenFormosa Common Voice 25 zh-TW 驗證句資料集中萃取、並經挑選的 runtime bigram 列。

專案只散布可直接釋出的審核結果，不散布原始句庫文本。

## 檔案與格式

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

候選列由本 repo 的 `build-bigram-stats` workflow，對當前正規化 ChiaKey lexicon 計算產生。

## Release 匯入規則

Bigram 以條件 log-probability 匯入，但會在 release 階段重新錨定到當前 unigram floor：

```text
stored = min(unigram(current) + boost + (raw - raw_max_of_source), -0.05)
```

此機制保留來源內部排序，並讓強 disambiguation 邊可高於 unigram 路徑；較弱配對維持在 unigram floor 下方而不生效。

- 預設 `boost`：`1.5`
- 覆寫環境變數：`COMMONVOICE_BIGRAM_BOOST`
- 設為 `0` 時：保留原始數值

## 上游與授權

- Dataset: <https://huggingface.co/datasets/OpenFormosa/common_voice_25_zh-TW>
- Source file: `validated_sentences.tsv`
- License: CC0-1.0
- Provider: OpenFormosa / Mozilla Common Voice

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
