# ChiaKey 現代詞覆蓋層

## 來源代號

`chiaki-modern-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源提供小型、專案自有的修正列，用於快速修補 seed lexicon 的明顯缺漏、排序問題，以及長句走訪時的前後詞選字問題。

`explicit.tsv` 是一般 unigram 校正的最終精準覆蓋層：它在 libchewing、Rime、web、
synthetic、OpenCC 與單字同音校正之後套用，因此可明確指定某個 qstring 的候選排序。
其後唯一可再降低 unigram 權重的規則是 `chiaki-fragment-denylist`；片段安全上限優先於一般頻率或排序偏好。

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
- `explicit.tsv` 在所有一般 unigram 來源與排序校正後、fragment denylist 前匯入；它可覆蓋
  一般候選權重，但不得重新提高被判定為片段的詞組。
- `bigrams.tsv`：在統計來源 bigram 之後匯入，用於覆蓋長句選字修正。

帶有 `naer-frequency-review` tag 的列是以國教院《通用詞頻表》進行本機審查後的多字同音
詞組排序調整。原始詞頻表與審查表在授權未確認前只供本機研究，不應隨公開 release 散布。

帶有 `de-tone-policy` tag 的列實作「的／得／地 以聲調區分」的政策：ㄉㄜ˙（`nq`）第一
候選為「的」、ㄉㄜˊ（`0C`）為「得」、ㄉㄧˋ（`:_`）為「地」，其餘同音字降到第一候選
下方 0.05，交由使用者自行選字。libchewing 的單字頻率讓這三個位置的前幾名相差不到
0.003（`nq` 三字甚至只差 1e-6），實際排序等於隨機；0.05 是讓預設排序確定的最小間距，
不改變這些字相對於其他讀音候選的強度。

## 上游與授權

此層為專案自有資料。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-modern-overlay/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
