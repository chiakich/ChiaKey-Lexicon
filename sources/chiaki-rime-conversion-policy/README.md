# ChiaKey Rime OpenCC 例外轉換

## 來源代號

`chiaki-rime-conversion-policy`

## 資料層

校正層

## 用途與定位

此來源定義專案自有的 OpenCC 後處理例外，用於 Rime essay 片語在進入 ChiaKey 補詞與 rerank 前的前處理。

Rime essay 涵蓋廣泛詞彙與語言模型訊號。匯入流程會先使用 OpenCC `t2tw` 將一般傳統異體字正規化為台灣標準繁體，例如 `喫`、`爲`、`羣`、`裏` 系列。此層只保留 `t2tw` 無法安全判斷、但 ChiaKey 明確需要的例外，例如地名中的 `里` 與食物詞 `里肌`。

規則多為片語層（避免過度轉換），但當某異體字對整族詞都應正規化、且無語意分歧時，可用單字規則做整族轉換。例如 `粘 → 黏`：教育部標準以 `黏` 為「黏著」義之正字，`粘` 為其異體，而 rime essay 語料偏好 `粘`（`粘土 802` 對 `黏土 577`，整族皆然），若不轉換會在 overlap-rerank 被扶正到 `黏` 之上。`粘` 唯一的獨立用法為姓氏（讀 ㄓㄢ，與「黏著」義 ㄋㄧㄢˊ 不同音，且不出現於 rime 語料），故整族轉換安全。

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

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-rime-conversion-policy/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
