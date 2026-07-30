# chiaki-tw-homophone-bigram 產生管線

`bigrams.tsv` 的產生工具與步驟。演算法本身與各步驟的理由寫在
[../README.md](../README.md) 的研究附錄，這份文件只講怎麼跑。

## 內容

| 路徑 | 說明 |
| --- | --- |
| `src/main.rs` | 管線工具（Rust，無外部依賴）。子命令：`extract` / `collision` / `candidates` / `emit` / `evaluate` / `coverage` |
| `scripts/build-corpora.mjs` | 步驟 1：原始語料 → 一句一行的 `.txt` |
| `scripts/score_discriminative.py` | 判別式打分器，用來過濾真實文本產生的配對（不可用來發明配對） |

```bash
cd sources/chiaki-tw-homophone-bigram/pipeline && cargo build --release
```

## 語料

語料本身不隨 repo 散布（體積與授權），來源如下：

| 語料 | 來源 |
| --- | --- |
| 政府新聞 | 行政院、大陸委員會、中央研究院、客家委員會的新聞發布，依政府資料開放授權條款 |
| 新北市政府新聞 | 新北市政府開放資料的新聞稿匯出 |
| 立法院公報詢答 | `scripts/corpus/build-ly-corpus.sh`（repo 內，可中斷續傳，全量數小時） |
| PTT 論壇 | HuggingFace `yuhuanstudio/PTT-pretrain-zhtw` 的 `ppt_pretrain.json`，Apache-2.0 |

新聞語料請自行下載後放到以下路徑，`build-corpora.mjs` 會檢查欄位是否相符：

| 路徑 | 格式 | 欄位 |
| --- | --- | --- |
| `sources/taiwan-gov-news-ey/raw/news.json` | JSON array | `標題`、`內容` |
| `sources/taiwan-gov-news-mac/raw/news.csv` | CSV | `標題`、`內文` |
| `sources/taiwan-gov-news-sinica/raw/news.json` | JSON array | `標題`、`網頁內容` |
| `sources/taiwan-gov-news-hakka/raw/news.json` | JSON array | `name`、`description` |
| `sources/taiwan-gov-news-ntpc/raw/news.csv` | CSV | `Subject_`、`Content` |

評估用的 LINE 群組語料不在這裡，也不會加入 repo——那是真實私人對話，群組成員同意的範圍是
基準測試使用，不含散布。它只用於量測、從不參與訓練，而且凍結半已經用過一次，後續演算法改動
需要新的凍結資料。

## 跑法

前置：`normalized/smart-mandarin.tsv`（repo 根目錄 `cargo run --release -- prepare-release`）。
詞庫要先定版再跑，因為詞庫會改變斷詞與撞碼判定。

出貨版用的七個語料檔與句數（可用 `wc -l` 對）：`govnews-train.txt` 57,460、
`ntpc-train.txt` 324,621、`ntpc-test.txt` 326,069、`ly-train.txt` 847,844、
`ly-test.txt` 74,304、`ptt-95.txt` 2,909,870、`ptt-gold5.txt` 156,656，合計 4,696,824。

```bash
# 1. 語料 → 一句一行
node scripts/build-corpora.mjs --out data --ptt-json path/to/ppt_pretrain.json

# 2. 抽取相鄰詞對（Viterbi 斷詞，詞長加成 +1.0/字，對齊 walker）
./target/release/chiakey-bigram-pipeline extract \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --out data/pairs-all.tsv \
  data/govnews-train.txt data/ntpc-train.txt data/ntpc-test.txt \
  data/ly-train.txt data/ly-test.txt data/ptt-95.txt data/ptt-gold5.txt

# 3. 排除複合詞（extract 標在第 5 欄）並套 doc_count >= 3 門檻
awk -F'\t' 'NR>1 && $4>=3 && NF==4' data/pairs-all.tsv > data/pairs-eligible.tsv

# 4. 產出（撞碼過濾＋讀音綁定＋語料證據檢查）
#    --input 是過濾後的表，--rival-evidence 必須是未過門檻的完整表，
#    否則對手的真實計數會被當成 0
./target/release/chiakey-bigram-pipeline emit \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --input data/pairs-eligible.tsv \
  --rival-evidence data/pairs-all.tsv \
  --out ../bigrams.tsv

# 5. 客觀關卡：每個語域分別跑，比較改動前後
./target/release/chiakey-bigram-pipeline evaluate \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --overlay ../bigrams.tsv --overlay-cur-column 2 \
  data/ntpc-test.txt
```

`evaluate` 以 `already_correct + net_gain` 為總正確位置數。`net_gain` 單獨下降不一定是回歸，
可能是原本要靠 bigram 救的位置已經自己對了。`--boost` 預設 1.5，對齊 release 的
`CHIAKI_TW_HOMOPHONE_BIGRAM_BOOST`。

匯入由 release 流程處理（`src/release.rs` 的 `import_chiaki_tw_homophone_bigrams`），權重會被
`calibrate_bigram_boost` 重新錨定到 `(current, cur_code)` 的 unigram。

## 幾件要知道的事

- **重產不會得到一樣的 230,993 列**，因為詞庫已經變了（撞碼判定與斷詞都跟著變），這也正是重產
  的目的。`evaluate` 的數字要重新量，不要沿用 [../README.md](../README.md) 附錄裡的數值。
- **當初 `extract` 的完整配對表沒有留下來**，重產要從步驟 2 開始跑。
- 如果是從頭下載語料，`build-corpora.mjs` 是事後重建的，跟當初的前處理不完全一致
  （govnews 差 0.07%，ntpc 還沒對上），新聞來源本身也會隨時間變動。手上已有那七個
  `.txt` 的話就直接用，不需要這一步。
- `--all-prev-readings` 沒有用在出貨版。767,923 筆因 `previous` 讀音歧義被棄用的候選可以靠它
  救回，但還沒量測過。
- repo 根目錄的 `src/bigram.rs`（`build-bigram-stats`）是另一個較舊的實作，缺步驟 7 與步驟 10，
  沒有產生過出貨資料，不要誤改那一份。
