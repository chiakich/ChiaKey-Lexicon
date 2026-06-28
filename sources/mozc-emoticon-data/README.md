# Mozc 顏文字資料

## 來源代號

`mozc-emoticon-data`

## 資料層

外部詞庫

## 用途與定位

此來源收錄 Mozc 顏文字資料，用於填充 ChiaKey 預載 `顏文字` canned-message 分類。

## 檔案與格式

vendored 檔案：

- `sources/mozc-emoticon-data/raw/categorized.tsv`
- `sources/mozc-emoticon-data/raw/emoticon.tsv`

上游資訊：

- Repository: <https://github.com/google/mozc>
- Commit: `28da5a39f9a7fd70251c85d269f4a8b47aa31cf8`
- Source directory: `src/data/emoticon/`

## Release 匯入規則

Release builder 先讀 `categorized.tsv`，再追加 `emoticon.tsv` 中尚未出現的顏文字。

最終只輸出顏文字值到 `prepopulated_service_data/canned_messages`；日文讀音鍵與說明文字僅作來源上下文，不會顯示在符號表。

## 上游與授權

授權：BSD-3-Clause

授權檔：`sources/mozc-emoticon-data/LICENSE`

## 驗證

```sh
cd sources/mozc-emoticon-data
shasum -a 256 -c source-inventory.sha256
```
