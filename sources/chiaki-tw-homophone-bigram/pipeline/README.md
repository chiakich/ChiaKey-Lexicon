# chiaki-tw-homophone-bigram 產生管線

`bigrams.tsv` 的產生工具與步驟。演算法本身與各步驟的理由寫在[../README.md](../README.md) 的研究附錄，這份文件只講怎麼跑。

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
| 立法院公報詢答 | `scripts/corpus/build-ly-corpus.sh` |
| PTT 論壇 | HuggingFace `yuhuanstudio/PTT-pretrain-zhtw` 的 `ppt_pretrain.json`（公開 PTT 貼文） |
| Plurk | 透過 Plurk 官方 API 取得的公開貼文 |
| 中文維基百科 | 正體轉換後的中文維基百科條文（CC BY-SA 4.0） |

目前 `build-corpora.mjs` 提供政府、立院與 PTT 輸入的處理步驟；Plurk 與中文維基的資料準備流程不納入此腳本。`bigrams.tsv` 是演算法凍結後以完整來源集合建立的發行產物，重新建置時應一併記錄各來源的版本與取得日期。

新聞語料請自行下載後放到以下路徑，`build-corpora.mjs` 會檢查欄位是否相符：

| 路徑 | 格式 | 欄位 |
| --- | --- | --- |
| `sources/taiwan-gov-news-ey/raw/news.json` | JSON array | `標題`、`內容` |
| `sources/taiwan-gov-news-mac/raw/news.csv` | CSV | `標題`、`內文` |
| `sources/taiwan-gov-news-sinica/raw/news.json` | JSON array | `標題`、`網頁內容` |
| `sources/taiwan-gov-news-hakka/raw/news.json` | JSON array | `name`、`description` |
| `sources/taiwan-gov-news-ntpc/raw/news.csv` | CSV | `Subject_`、`Content` |

## 跑法

前置：`normalized/smart-mandarin.tsv`（repo 根目錄 `cargo run --release -- prepare-release`）。詞庫要先定版再跑，因為詞庫會改變斷詞與撞碼判定。

2026-08-02 重建用的六個語料檔與句數（可用 `wc -l` 對）：`ly-train.txt` 2,638,679、`ntpc-train.txt` 247,378、`ntpc-test.txt` 247,378、`plurk.txt` 102,484、`ptt-all.txt` 3,439,170、`wiki.txt` 892,029，合計 7,567,118。政府新聞四家中只取得陸委會，其餘端點已失效或不在 data.gov.tw，合計佔比不到 1%，該次重建未納入。

```bash
# 1. 語料 → 一句一行
node scripts/build-corpora.mjs --out data --ptt-json path/to/ppt_pretrain.json

# 2. 抽取相鄰詞對（Viterbi 斷詞，詞長加成 +1.0/多出的字，對齊 Node::lengthPrior）
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

`evaluate` 以 `already_correct + net_gain` 為總正確位置數。`net_gain` 單獨下降不一定是回歸，可能是原本要靠 bigram 救的位置已經自己對了。`--boost` 預設 1.5，對齊 release 的`CHIAKI_TW_HOMOPHONE_BIGRAM_BOOST`。

匯入由 release 流程處理（`src/release.rs` 的 `import_chiaki_tw_homophone_bigrams`），權重會被 `calibrate_bigram_boost` 重新錨定到 `(current, cur_code)` 的 unigram。

## 幾件要知道的事

- **重產不會得到一樣的 339,383 列**，因為詞庫已經變了（撞碼判定與斷詞都跟著變），這也正是重產的目的。`evaluate` 的數字要重新量，不要沿用 [../README.md](../README.md) 附錄裡的數值。
- `--all-prev-readings` 沒有用在出貨版。1,081,430 筆因 `previous` 讀音歧義被棄用的候選可以靠它救回，但還沒量測過。
- repo 根目錄的 `src/bigram.rs`（`build-bigram-stats`）是另一個較舊的實作，缺步驟 7 與步驟 10，沒有產生過出貨資料，不要誤改那一份。
