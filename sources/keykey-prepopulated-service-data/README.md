# KeyKey 預載服務資料

## 來源代號

`keykey-prepopulated-service-data`

## 資料層

相容性基底詞庫

## 用途與定位

此來源收錄 Yahoo KeyKey 原始預載 canned-message 資料。

主要檔案：

- `sources/keykey-prepopulated-service-data/vendor/CannedMessages.plist`

上游路徑：

- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData/CannedMessages.plist`

## Release 匯入規則

Release builder 會把 plist 內容寫入 `prepopulated_service_data`：

- `canned_messages`
- `canned_messages_timestamp`（正值 release timestamp）

在 release cooking 過程，payload 會再做兩項增補：

- `chiaki-symbols-overlay/symbols.tsv` 轉成 8 個補充按鈕分類：
  `補充標點`、`貨幣與標記`、`數字序號`、`補充箭頭`、`補充數學`、`勾叉與星號`、`花色與音樂`、`單位符號`
- 以 `mozc-emoticon-data` 全面替換原本帶註解的 `顏文字` 分類，改為乾淨的 Mozc `Messages` 清單

`onekey_services` 與 `onekey_services_timestamp` 刻意不輸出，因現代 ChiaKey 已不再載入 Yahoo 時代 OneKey URL launcher。

## 上游與授權

此層為 KeyKey 相容資料與專案整合輸出。

## 驗證

```sh
cd sources/keykey-prepopulated-service-data
shasum -a 256 -c source-inventory.sha256
```
