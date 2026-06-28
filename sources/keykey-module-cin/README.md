# KeyKey 模組 CIN 字表

## 來源代號

`keykey-module-cin`

## 資料層

相容性基底詞庫

## 用途與定位

此來源收錄 Yahoo KeyKey / OpenVanilla 原始 CIN 字表，供 Smart Mandarin 語言模型之外的 runtime 模組使用。

## 檔案與格式

匯入對應：

- `vendor/cj-ext.cin` -> `Generic-cj-cin`
- `vendor/simplex-ext.cin` -> `Generic-simplex-cin`
- `vendor/cj-punctuations-halfwidth.cin` -> `Punctuations-cj-halfwidth-cin`
- `vendor/cj-punctuations-mixedwidth.cin` -> `Punctuations-cj-mixedwidth-cin`
- `vendor/bopomofo-correction.cin` -> `BopomofoCorrection-bopomofo-correction-cin`

上游路徑位於：

```text
YahooKeyKey-Source-1.1.2528/DataTables/
```

## Release 匯入規則

Release builder 會將這些檔案匯入為模組用 SQLite key/value tables。

這些資料不會併入 `unigrams`，因此不影響 Smart Mandarin 候選排序。

## 上游與授權

資料來自 Yahoo KeyKey 原始模組字表。

## 驗證

```sh
cd sources/keykey-module-cin
shasum -a 256 -c source-inventory.sha256
```
