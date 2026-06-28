# KeyKey 注音標點 CIN 字表

## 來源代號

`keykey-punctuations-cin`

## 資料層

相容性基底詞庫

## 用途與定位

此來源收錄 KeyKey 原始 BPMF 標點 CIN 字表，作為 Smart Mandarin runtime 標點查表的相容基底。

## 檔案與格式

主要檔案：

```text
sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin
```

上游路徑：

```text
YahooKeyKey-Source-1.1.2528/DataTables/bpmf-punctuations.cin
```

來源清單（含雜湊）：

```text
sources/keykey-punctuations-cin/source-inventory.sha256
```

## Release 匯入規則

Release builder 只匯入 `%chardef` 區段中 key 以 `_punctuation_` 或 `_ctrl_` 開頭的列。

這些列是 runtime 標點查找必需資料，例如 Shift+, 會解析 `_punctuation_<` 並預期回傳 `，`。

## 上游與授權

資料來自 Yahoo KeyKey 原始標點字表。

## 驗證

可用 `source-inventory.sha256` 驗證 vendored 檔案完整性。
