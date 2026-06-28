# ChiaKey Rime 詞形轉換政策

## 來源代號

`chiakey-rime-conversion-policy`

## 資料層

校正層

## 用途與定位

此來源定義專案自有的詞形轉換規則，用於 Rime essay 片語在進入 ChiaKey 補詞與 rerank 前的前處理。

Rime essay 涵蓋廣泛詞彙與語言模型訊號，但部分詞形不符合現代台灣繁中預設輸出。此層會保留原頻率證據，同時映射到專案偏好詞形，例如將 `喫壞` 證據轉為 `吃壞`，將 `爲啥` 證據轉為 `為啥`。

## 檔案與格式

`replacements.tsv`：

```text
from<TAB>to<TAB>tags
```

## Release 匯入規則

規則僅作用於 Rime 片語文本，且 `from`/`to` 必須字數相同，以維持推導 qstring 與片語對齊。

## 上游與授權

此層為專案自有政策資料。

授權：CC0-1.0

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
