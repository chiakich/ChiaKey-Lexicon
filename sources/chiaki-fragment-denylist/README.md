# ChiaKey 片段詞降權清單

## 來源代號

`chiaki-fragment-denylist`

## 資料層

校正層

## 用途與定位

此來源為專案自建的片段詞權重上限清單，目標是抑制「非詞彙化句片段」在斷詞時搶分。

典型例子是助動詞/情態詞 + 動詞的雙字片段（如 `會比`、`會在`、`想用`）。由於 KeyKey walker 會最大化節點 log-probabilities 之和，若此類雙字片段權重過高，可能在二節點剖分中偷走原本三節點剖分的一個音節，例如在輸入 `會比較準` 時讓 `會比`（會 + 比）錯誤壓過 `會 | 比較 | 準`。

## 檔案與格式

`fragment-demotions.tsv`：

```text
phrase<TAB>max_weight<TAB>tags
```

## Release 匯入規則

此表作為最後階段 phrase-level cap（`apply_variant_demotions`）：

- 若某片語 unigram 權重高於 `max_weight`，則降到 `max_weight`。
- 若原本已低於上限，則保持不變。

安全上限採用：

```text
w(lead_char) + w(stolen_word) - 0.30
```

這是能讓正確 `lead | stolen_word | ...` 剖分勝出的最小降權幅度。

此策略不影響單字直出能力：同一組字元仍可由字元切分輸出。

## 清單建置流程

1. 結構篩選（離線）：挑出尾字會引出更強跨界詞的雙字片語（需同字，不僅同音）。
2. 限縮前導字集合為助動詞/情態詞（`會能要想該可肯願敢應須必得`）；未限縮規則誤傷太多真詞。
3. 以教育部《重編國語辭典修訂本》詞頭做離線比對：辭典未收錄者列為片段候選。
4. 人工複核殘餘項（約 64 列），將實際為真詞但辭典未收者（如 `可愛`）排除。

## 上游與授權

辭典只作為離線建置工具，並未隨專案散布其文本。

- 辭典文本授權：CC BY-ND 3.0 TW（僅作離線比對參考，未隨本清單散布）
- 本清單為專案自行撰寫資料列

授權：CC BY-NC 4.0（Chiaki.C）

非商業與開源專案可於標示來源為 Chiaki.C 前提下使用；商業用途需另行取得授權。

授權全文見：`sources/chiaki-fragment-denylist/LICENSE`

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
