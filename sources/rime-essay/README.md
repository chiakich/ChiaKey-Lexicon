# Rime essay 來源

## 來源代號

`rime-essay`

## 資料層

外部詞庫

## 用途與定位

此來源匯入 Rime 共享詞彙與語言模型，作為低優先級補充片語層。

由於 `essay.txt` 不含注音讀音，release builder 只會匯入可由當前資料庫單字讀音安全推導的條目。

## 檔案與格式

固定上游檔案：

- `rime/rime-essay` commit `48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed` 的 `essay.txt`

本機 vendored 路徑：

```text
sources/rime-essay/raw/
```

## Release 匯入規則

一般 release build 直接使用 repo 內 vendored raw 檔案，不需網路下載。

維護者更新固定快照時可執行：

```sh
cargo run --release -- fetch-modern-sources
```

raw 檔案受 git 追蹤，檔案雜湊記錄於 `source-inventory.sha256`。

## 上游與授權

授權：LGPL-3.0

## 驗證

可用 `source-inventory.sha256` 驗證 vendored 檔案完整性。
