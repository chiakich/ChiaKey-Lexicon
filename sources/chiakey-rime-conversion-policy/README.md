# ChiaKey Rime OpenCC 例外轉換

## 來源代號

`chiakey-rime-conversion-policy`

## 資料層

校正層

## 用途與定位

此來源定義專案自有的 OpenCC 後處理例外，用於 Rime essay 片語在進入 ChiaKey 補詞與 rerank 前的前處理。

Rime essay 涵蓋廣泛詞彙與語言模型訊號。匯入流程會先使用 OpenCC `t2tw` 將一般傳統異體字正規化為台灣標準繁體，例如 `喫`、`爲`、`羣`、`裏` 系列。此層只保留 `t2tw` 無法安全判斷、但 ChiaKey 明確需要的例外，例如地名中的 `里` 與食物詞 `里肌`。

## 檔案與格式

`replacements.tsv`：

```text
from<TAB>to<TAB>tags
```

## Release 匯入規則

Rime 片語會先套 OpenCC `t2tw`，再套此檔案的例外規則。

規則僅作用於 Rime 片語文本，且 `from`/`to` 必須字數相同，以維持推導 qstring 與片語對齊。

## 上游與授權

此層為專案自有政策資料。

授權：CC0-1.0

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
