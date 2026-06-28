# ChiaKey 現代詞覆蓋層

## 來源代號

`chiakey-modern-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源提供小型、專案自有的修正列，用於在整合大型公開頻率語料前，先快速修補 seed lexicon 的明顯缺漏與排序問題。

## 檔案與格式

`phrases.tsv`：

```text
phrase<TAB>weight<TAB>tags
```

Release script 會根據 bootstrap KeyKey DB 的單字讀音推導每個詞的 reading，並以指定權重插入或替換 unigram。

`explicit.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

當修正必須綁定特定讀音、聲調或 KeyKey 內部 qstring 時，使用 `explicit.tsv`。此表只替換精確的 qstring/phrase 配對。

## Release 匯入規則

- `phrases.tsv`：以推導讀音匯入一般修正。
- `explicit.tsv`：以明確 qstring 進行精準覆蓋。

## 上游與授權

此層為專案自有資料。

授權：CC0-1.0

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
