# ChiaKey 現代詞覆蓋層

## 來源代號

`chiaki-modern-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源提供小型、專案自有的修正列，用於快速修補 seed lexicon 的明顯缺漏、排序問題，以及長句走訪時的前後詞選字問題。

## 檔案與格式

`explicit.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

所有修正都必須綁定特定讀音、聲調或 KeyKey 內部 qstring，並寫入 `explicit.tsv`。此表只替換精確的 qstring/phrase 配對。

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

此表用於修正「長句中兩個已存在詞的組合」選字，例如輸入 `天意難測` 時，若 walker 選成 `天意南側`，應加入 `天意 -> 難測` 的 bigram，而不是把整句當成缺詞。`qstring` 是 `previous` 的讀音 + 空白 + `current` 的讀音，`probability` 是 runtime bigram log-probability；人工修正常用接近 `-0.35` 到 `-0.80` 的值，越接近 0 越強。

## Release 匯入規則

- `explicit.tsv`：以明確 qstring 進行精準覆蓋。
- `bigrams.tsv`：在統計來源 bigram 之後匯入，用於覆蓋長句選字修正。

## 上游與授權

此層為專案自有資料。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-modern-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
