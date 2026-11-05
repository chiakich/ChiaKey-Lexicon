# 精確詞條排除層

`exclusions.tsv` 逐筆指定要從最終詞庫移除的 `(qstring, phrase)`，格式為：

```text
qstring<TAB>phrase<TAB>reason
```

這一層在所有詞條、補讀音與 bigram 匯入後套用。它會同步移除 `unigrams`、`Mandarin-bpmf-cin` 的精確配對，以及引用該讀音配對的 bigram；`normalized/smart-mandarin.tsv` 因由最終 DB 產生，也不會再出現該列。其他讀音不受影響。若指定配對已不存在，建置會失敗，提醒維護者檢查上游資料是否變動。

此表只收經人工確認的錯誤讀音或錯誤詞條，不用於一般候選排序。每列必須填原因，以便日後複核。原始上游資料不在此層改寫。

此層為專案自有資料，授權為 CC BY-NC 4.0；詳見 `LICENSE`。
