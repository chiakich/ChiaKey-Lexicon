# chiaki-tw-homophone-bigram 產生管線

`bigrams.tsv` 的產生工具與步驟。放在這裡是因為原本的工具只存在於 gitignored 的
`tmp/`，一次清理就會讓這一層無法重現——2026-07-30 已經發生過一次（見下方〈已知缺口〉）。

演算法與各步驟的理由寫在 [../README.md](../README.md) 的研究附錄，這份文件只講「怎麼跑」。

## 內容

| 路徑 | 說明 |
| --- | --- |
| `src/main.rs` | 管線工具（Rust，無外部依賴）。子命令：`extract` / `collision` / `candidates` / `emit` / `evaluate` / `coverage` |
| `scripts/build-corpora.mjs` | 步驟 1：原始語料 → 一句一行的 `.txt`。**部分重建**，見〈驗證狀態〉 |
| `scripts/score_discriminative.py` | 路線二的判別式打分器。保留作為真實文本配對的**過濾器**，不可用來產生配對 |

建置：

```bash
cd sources/chiaki-tw-homophone-bigram/pipeline && cargo build --release
```

## 語料來源

出貨版用了 4,696,824 句 / 367,050,071 字，組成見 [../README.md](../README.md) 的語料表。
**沒有任何語料在 repo 裡**——`.gitignore:15` 排除整個 `sources/taiwan-gov-news-*/`，那些目錄
只存在於原作者的工作機上。跑這條管線前必須自己備齊：

| 語料 | 取得方式 | 授權 |
| --- | --- | --- |
| 立法院公報詢答 | `scripts/corpus/build-ly-corpus.sh`（repo 內，抓取→萃取，可中斷續傳，全量數小時） | 政府資料開放 |
| PTT 論壇 | HuggingFace `yuhuanstudio/PTT-pretrain-zhtw` 的 `ppt_pretrain.json`（約 849MB） | Apache-2.0 |
| 政府新聞（4 個機關）＋新北市政府新聞 | **需自行下載，repo 內沒有腳本**，見下 | 政府資料開放授權條款 |

### 政府新聞：需自行下載

當初是手動取得的，repo 裡沒有 fetch 腳本（`fetch-modern-sources` 只處理 libchewing、
rime-essay、mozc-emoticon）。**確切的 dataset ID 或 API endpoint 沒有被記錄下來**，以下是
從出貨語料本身反查出的出處與規格。請從各機關的開放資料或新聞發布頁取得，放到對應路徑，
再用 `build-corpora.mjs` 的 schema 檢查確認格式相符。

| 路徑 | 機關 | 格式 | 必要欄位 | 出貨快照 |
| --- | --- | --- | --- | --- |
| `sources/taiwan-gov-news-ey/raw/news.json` | 行政院 | JSON array | `標題`、`內容`、`上版日期` | 500 筆，民國 114-10-03～115-06-23 |
| `sources/taiwan-gov-news-mac/raw/news.csv` | 大陸委員會 | CSV（BOM） | `發布日期`、`標題`、`內文` | 起自 2016-05-20 |
| `sources/taiwan-gov-news-sinica/raw/news.json` | 中央研究院 | JSON array | `標題`、`網頁內容`、`發布日期`、`來源網址` | 1,095 筆，2005-01-26～2026-06-23，來源 `https://www.sinica.edu.tw/News_Content/…` |
| `sources/taiwan-gov-news-hakka/raw/news.json` | 客家委員會 | JSON array（BOM） | `name`、`description`、`Date`、`sourceWebSite` | 1,853 筆，2017-01～2026-05，來源 `https://www.hakka.gov.tw/chhakka/app…` |
| `sources/taiwan-gov-news-ntpc/raw/news.csv` | 新北市政府 | CSV（BOM，21 欄） | `Subject_`、`Content`（`From_` 為 `NTPC`） | 44,372 筆，起自 2017-06 |

行政院那份只有 500 筆、且只涵蓋最近 8 個月，看得出來是「最新 N 筆」的抓法而非全量；換一個
時間點下載會拿到不同內容，這是重產無法逐位元相同的原因之一。

實驗過但**未進入出貨版**的語料（`lianghsun/tw-ptt-keyboard-warrior-chat`、Dcard、C4
中文子集）不在管線內，僅出現在研究附錄的失敗路線記錄。

### 不散布的資料

評估用的 LINE 群組匯出語料（`line-*.txt`）**不在此處也不會加入 repo**。那是真實私人對話，
群組成員同意的範圍是基準測試使用，不包含散布。它只用於量測、從不參與訓練，而且**凍結半已
經使用過一次**——任何後續演算法改動需要新的凍結資料，不能重複使用它來聲稱改善。

## 跑法

前置：`normalized/smart-mandarin.tsv`（由 repo 根目錄 `cargo run --release -- prepare-release`
產生）。**注意順序**：詞庫改變會改變斷詞與撞碼判定，所以詞庫要先定版再跑這條管線。

```bash
# 1. 語料 → 一句一行
node scripts/build-corpora.mjs --out data --ptt-json path/to/ppt_pretrain.json

# 2. 抽取相鄰詞對（Viterbi 斷詞，詞長加成 +1.0/字，對齊 walker）
./target/release/chiakey-bigram-pipeline extract \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --out data/pairs-all.tsv \
  data/govnews-train.txt data/ntpc-train.txt data/ntpc-test.txt \
  data/ly-train.txt data/ptt-all.txt

# 3. 產出（撞碼過濾＋讀音綁定＋語料證據檢查）
#    --rival-evidence 要餵「未過門檻」的完整配對表，否則證據檢查會把
#    對手的真實計數誤判為 0
./target/release/chiakey-bigram-pipeline emit \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --input data/pairs-all.tsv \
  --rival-evidence data/pairs-all.tsv \
  --out ../bigrams.tsv

# 4. 客觀關卡：每個語域分別跑，比較改動前後
./target/release/chiakey-bigram-pipeline evaluate \
  --lexicon ../../../normalized/smart-mandarin.tsv \
  --overlay ../bigrams.tsv --overlay-cur-column 2 \
  data/ntpc-test.txt
```

`evaluate` 的輸出以 `already_correct + net_gain` 為總正確位置數；`net_gain` 單獨下降不代表
回歸，可能是原本要靠 bigram 救的位置已經自己對了。`--boost` 預設 1.5，對齊 release 的
`CHIAKI_TW_HOMOPHONE_BIGRAM_BOOST`。

匯入由 release 流程處理（`src/release.rs` 的 `import_chiaki_tw_homophone_bigrams`），
權重會被 `calibrate_bigram_boost` 重新錨定到 `(current, cur_code)` 的 unigram。

## 驗證狀態

`scripts/build-corpora.mjs` 是**事後重建**，不是當初用的腳本（那個沒有留下來）。目前狀態：

| 語料 | 出貨行數 | 重建行數 | 狀態 |
| --- | --- | --- | --- |
| govnews | 57,460 | 57,496 | 差 38 行（0.07%），全是 8–11 字的短標題，原因未確定 |
| ntpc | 650,690 | 487,258 | **不符**，少 25%。切法也不同（出貨的兩半是 324,621／326,069，非等分） |
| ly | 847,844 + 74,304 | 未驗證 | 需先跑 `build-ly-corpus.sh` |
| ptt | 3,066,526 | 未驗證 | 需先下載 849MB 資料集 |

已驗證的規則（從倖存的出貨語料反推）：句子切在全形 `。！？；` 與換行；保留 8 字以上且含
漢字者；HTML 標籤就地移除但**不解**字元實體（出貨語料仍有 373 行含 `&nbsp;`）；欄位對應
逐來源不同（ey `標題`/`內容`、mac `標題`/`內文`、sinica `標題`/`網頁內容`、hakka
`name`/`description`、ntpc `Subject_`/`Content`）。

**這意味著重產不會產出完全相同的 230,993 列**，`evaluate` 的數值必須重新量測，不能沿用
[../README.md](../README.md) 附錄裡的數字。

## 已知缺口

1. **當初的中間產物已遺失。** 從 367M 字抽出的完整配對表（`extract` 的輸出，同時作為
   `--rival-evidence`）不存在了，當時放在系統 `/tmp` 被清掉。重產必須從步驟 2 重跑。
2. **語料前處理未完全重建**，見上表。ntpc 的差異足以影響輸出，需要先解掉。
3. **`--all-prev-readings` 未用於出貨版。** 767,923 筆候選因 `previous` 讀音歧義被棄用；
   那個旗標可以繞過檢查，但尚未量測。
4. `src/bigram.rs`（repo 根目錄的 `build-bigram-stats`）是**另一個較舊的實作**，缺步驟 7
   與步驟 10，沒有產生過出貨資料。不要誤改那一份。
