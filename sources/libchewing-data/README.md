# libchewing-data 來源

## 來源代號

`libchewing-data`

## 資料層

外部詞庫

## 用途與定位

此來源匯入 New Chewing 內建辭典，作為主要現代繁體中文注音詞庫層。

## 檔案與格式

固定上游檔案（`chewing/libchewing-data` tag `v2026.3.22`，上游 home 已移至 Codeberg：<https://codeberg.org/chewing/libchewing-data>）：

- `dict/chewing/tsi.csv`
- `dict/chewing/word.csv`
- `dict/chewing/alt.csv`

本機 vendored 路徑：

```text
sources/libchewing-data/raw/
```

## Release 匯入規則

一般 release build 直接使用 repo 內 vendored raw 檔案，不需網路下載。

維護者更新固定快照時可執行：

```sh
cargo run --release -- fetch-modern-sources
```

raw 檔案受 git 追蹤，檔案雜湊記錄於 `source-inventory.sha256`。

## 上游與授權

授權：LGPL-2.1-or-later

判定依據為來源檔案 `dc:license` 標頭與 libchewing 專案授權。

## 驗證

可用 `source-inventory.sha256` 驗證 vendored 檔案完整性。
