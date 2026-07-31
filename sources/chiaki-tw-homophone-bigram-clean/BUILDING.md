# Clean bigram 建置方式

本文件記錄 `bigrams.tsv` 的建置範圍。原文語料與中間統計檔不隨 repository 散布。

## 允許的輸入

本版本使用：

- 政府新聞：行政院、大陸委員會、中央研究院、客家委員會；
- 新北市政府新聞；
- 立法院公報詢答逐字稿。

## 參考程序

先依 `../chiaki-tw-homophone-bigram/pipeline/README.md` 產生句子檔。令 `$WORK` 為暫存目錄，並以目前的 `normalized/smart-mandarin.tsv` 固定詞庫：

```sh
PIPELINE=sources/chiaki-tw-homophone-bigram/pipeline/target/release/chiakey-bigram-pipeline

$PIPELINE extract \
  --lexicon normalized/smart-mandarin.tsv \
  --out "$WORK/pairs-all.tsv" \
  "$WORK/govnews-train.txt" "$WORK/ntpc-train.txt" "$WORK/ly-train.txt"

awk -F '\t' 'NR > 1 && $4 >= 3 && NF == 4' \
  "$WORK/pairs-all.tsv" > "$WORK/pairs-eligible.tsv"

$PIPELINE emit \
  --lexicon normalized/smart-mandarin.tsv \
  --input "$WORK/pairs-eligible.tsv" \
  --rival-evidence "$WORK/pairs-all.tsv" \
  --out "$WORK/bigrams-raw.tsv"

# A walker can reach only one row per (qstring, previous). Keep the strongest
# row; resolve equal strengths by lexical `current` order so the result is
# deterministic across importers.
{
  head -n 1 "$WORK/bigrams-raw.tsv"
  tail -n +2 "$WORK/bigrams-raw.tsv" \
    | LC_ALL=C sort -t $'\t' -k1,1 -k2,2 -k4,4nr -k3,3 \
    | awk -F '\t' '!seen[$1 FS $2]++' \
    | LC_ALL=C sort
} > sources/chiaki-tw-homophone-bigram-clean/bigrams.tsv
```

`ly-train.txt` 應排除保留作評測的立院會期。若將原本保留的立院測試集納回訓練，必須另換一份未參與過調校的新 hold-out 才能宣稱泛化評估。

每次重建後，至少在未被使用為訓練輸入的新北市政府新聞、立法院公報與日常訊息語域上，以 pipeline 的 `evaluate` 子命令重跑校準後的品質檢查。資料來源、版本、取得日期與檔案雜湊應一併記錄在 release note。
