# ChiaKey 現代詞覆蓋層

## 來源代號

`chiaki-modern-overlay`

## 資料層

專案詞庫

## 用途與定位

此來源提供專案自有的 unigram 補充、精準覆蓋修正，以及長句走訪時的前後詞選字問題修正。

`unigrams.tsv` 收錄新增詞條與補充讀音；其中保留 `chiaki-synthetic-overlay` tag 的列，
是已退役的合成語料層留下的來源資訊。`explicit.tsv` 是一般 unigram 校正的最終精準
覆蓋層：它在 libchewing、Rime、web、OpenCC 與單字同音校正之後套用，因此可明確指定某個 qstring 的候選排序。
其後唯一可再降低 unigram 權重的規則是 `chiaki-fragment-denylist`；片段安全上限優先於一般頻率或排序偏好。

## 檔案與格式

`unigrams.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

用於新增缺詞或補充讀音。GitHub 的缺詞回報流程與 `add-unigram.mjs` 都會追加至此表。

`explicit.tsv`：

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

所有覆蓋修正都必須綁定特定讀音、聲調或 KeyKey 內部 qstring，並寫入 `explicit.tsv`。此表只替換精確的 qstring/phrase 配對。

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

此表用於修正「長句中兩個已存在詞的組合」選字，例如輸入 `天意難測` 時，若 walker 選成 `天意南側`，應加入 `天意 -> 難測` 的 bigram，而不是把整句當成缺詞。`qstring` 是 `previous` 的讀音 + 空白 + `current` 的讀音，`probability` 是 runtime bigram log-probability；人工修正常用接近 `-0.35` 到 `-0.80` 的值，越接近 0 越強。

`reading-supplements.tsv`：

```text
qstring<TAB>phrase<TAB>tags
```

補充「詞庫裡已有這個詞，但缺某個合法讀音」的情況——不限來源，libchewing、rime-essay
猜測、任何一層都適用。讀音直接以 qstring 記錄（跟 `unigrams.tsv`／`explicit.tsv` 同一套
編碼），由產生此表的腳本一次轉換好，不需要在 release 時重新從注音推導；`tags` 欄位會併入
最終列的 tags，保留這筆讀音的複核來源（例如 `moedict-reviewed`）。

一個詞若含破音字、且不同讀音都算正確用法（例如「教學」的 `ㄐㄧㄠˋ ㄒㄩㄝˊ` 是標準讀音，
但詞庫裡 libchewing 原本只有 `ㄐㄧㄠ ㄒㄩㄝˊ`），可以寫多行、同一 `phrase` 對應多個
`qstring`——兩種讀音都會可以打出這個詞。`reading_supplement_records`
（`src/importers.rs`）**只會新增缺的讀音，不會動、也不會蓋掉既有讀音本身贏不贏**：如果
原本錯的讀音目前是預設候選，套用後它還是預設候選，只是新讀音也變得同樣好打。

每一行套用時的權重規則：

- 若這個讀音的 qstring 目前**沒有**被其他詞佔用，權重採這個詞自己目前最高的權重（同一個
  詞、同一個使用頻率，只是多一個可以打出來的讀音）。
- 若這個讀音的 qstring 已經被其他詞佔用（兩個不同詞剛好同音撞碼），這一列的權重會被
  強制壓到略低於既有詞的權重（見 `READING_SUPPLEMENT_CONFLICT_MARGIN`），確保不會蓋過
  已經正確的那個詞，但使用者選字時仍能循環到這個候選。
- 若這個詞根本不在詞庫裡（來自任何來源），這一行會被跳過——這張表只補既有詞的讀音，
  不是新詞來源。

套用後的列會帶 `reading-supplements,supplemental-reading` tag，外加檔案內該行自己的 tags。

**目前這張表的內容全部重製自教育部《重編國語辭典修訂本》**（透過 `g0v/moedict-data`，
<https://github.com/g0v/moedict-data>，`dict-revised.json` 查閱、比對後產生）。教育部《重編
國語辭典修訂本》採「創用CC－姓名標示－禁止改作 3.0 台灣」授權條款釋出（詳見
<https://ti-wb.github.io/creativecommon-tw/index.html>），本授權允許重製、散布（含商業性
利用），但不得改作；本表只逐一記錄讀音本身、未做任何修改或詮釋，屬授權允許的重製範圍。

> 中華民國教育部《重編國語辭典修訂本》採「創用CC-姓名標示-禁止改作 3.0 臺灣授權條款」釋出，
> 姓名標示：教育部（終身教育司）。

這張表本身不隨這個檔案內嵌字典原文（釋義、例句等），只有讀音；release 匯入時也不會讀取
moedict-data 本身。整份表可用以下流程重新產生：

```sh
node scripts/audit/audit-moedict-readings.mjs
cargo build --release
node scripts/lexicon/generate-moedict-readings.mjs
```

`audit-moedict-readings.mjs` 需要本機另外準備 moedict-data 的 `dict-revised.json`（不隨
本 repo 提供，見該腳本檔頭說明）；`generate-moedict-readings.mjs` 直接把比對結果轉成
qstring 寫回這張表，可以直接 commit。未來若加入其他複核來源（非 moedict），沿用同一份
`reading-supplements.tsv`，只要在 `tags` 欄位標示清楚來源即可。

## Release 匯入規則

- `unigrams.tsv`：匯入一般新增詞條與補充讀音。
- `explicit.tsv`：以明確 qstring 進行精準覆蓋。
- `explicit.tsv` 在所有一般 unigram 來源與排序校正後、fragment denylist 前匯入；它可覆蓋
  一般候選權重，但不得重新提高被判定為片段的詞組。
- `bigrams.tsv`：在統計來源 bigram 之後匯入，用於覆蓋長句選字修正。
- `reading-supplements.tsv`：在 `explicit.tsv` 之後、fragment denylist 之前匯入，此時詞庫
  已完整合併，才能正確判斷「這個詞是否已在詞庫」與「這個讀音是否已被別的詞佔用」。

帶有 `naer-frequency-review` tag 的列是以國教院《通用詞頻表》進行本機審查後的多字同音
詞組排序調整。原始詞頻表與審查表在授權未確認前只供本機研究，不應隨公開 release 散布。

帶有 `de-tone-policy` tag 的列實作「的／得／地 以聲調區分」的政策：ㄉㄜ˙（`nq`）第一
候選為「的」、ㄉㄜˊ（`0C`）為「得」、ㄉㄧˋ（`:_`）為「地」，其餘同音字降到第一候選
下方 0.05，交由使用者自行選字。libchewing 的單字頻率讓這三個位置的前幾名相差不到
0.003（`nq` 三字甚至只差 1e-6），實際排序等於隨機；0.05 是讓預設排序確定的最小間距，
不改變這些字相對於其他讀音候選的強度。

## 上游與授權

此層絕大部分為專案自有資料。

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-modern-overlay/LICENSE`

**例外：`reading-supplements.tsv`**。此表內容重製自教育部《重編國語辭典修訂本》，適用該
辭典自己的「創用CC－姓名標示－禁止改作 3.0 台灣」授權條款，不是上述 CC BY-NC 4.0——教育部
的授權條款本身允許重製與商業性利用，比 Chiaki.C 的一般授權更寬鬆，但要求姓名標示（教育部
（終身教育司））且不得改作。散布這張表時請一併保留這項姓名標示。

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
