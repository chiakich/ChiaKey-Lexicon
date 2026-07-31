# ChiaKey 台灣同音消歧 clean bigram overlay

## 來源代號

`chiaki-tw-homophone-bigram-clean`

## 定位

這是可獨立整合的 clean bigram add-on，不是第二套 ChiaKey 完整資料庫，也不會由目前的正式 release 預設匯入。

這份資料集以政府機關新聞發布與立法院公報詢答逐字稿建置，為希望採用明確資料授權的使用者準備。

想在付費開源輸入法中使用可公開回饋的同音消歧表，可整合本檔；這不表示 ChiaKey 的完整資料庫或其他來源也適用同一授權。

## 授權

`bigrams.tsv` 採 [Open Database License 1.0 (ODbL)](https://opendatacommons.org/licenses/odbl/1-0/)。

ODbL 允許商業使用，但公開使用衍生資料庫時必須以 ODbL 提供該衍生資料庫，並依 [`LICENSE`](LICENSE) 的方式署名。這是資料庫層的 reciprocal 條件；它不會自動改變使用該表的輸入法程式本身的授權。

## 格式

`bigrams.tsv`：

```text
qstring<TAB>previous<TAB>current<TAB>probability
```

格式與 `chiaki-tw-homophone-bigram` 相同。`probability` 是本來源內部強度序，不是條件機率；整合端應以自己的 unigram 尺度重新校準。ChiaKey 的參考公式為：

```text
stored = min(unigram(current) + boost + (raw - raw_max_of_source), -0.05)
```

預設 `boost` 為 `1.5`。

## 建置來源與署名

本版本使用下列材料的句子切分與詞對統計；不散布原文或中間語料。

| 材料 | 權利依據 | 應保留的來源標示 |
| --- | --- | --- |
| 行政院、大陸委員會、中央研究院、客家委員會、新北市政府新聞發布 | 政府資料開放授權條款 | 各機關名稱 |
| 立法院公報詢答逐字稿 | 立法院網站資料開放宣告 | 立法院議事暨公報資訊網 |

## 與 full 版本的關係

`chiaki-tw-homophone-bigram` 是現有完整品質版本，使用更多語域的文本，且維持其既有的來源與授權聲明。本 clean add-on 是以不同來源集合重新建置的資料庫。

## 本版量測

2026-07-30 建置版有 56,048 列。下列數字以與正式來源相同的 pipeline 校準和判定方式量測；`net_gain` 是修對位置減去搶錯位置，不是端到端輸入正確率。

| 未參與訓練的測試語域 | 修對 | 搶錯 | 淨值 |
| --- | ---: | ---: | ---: |
| 新北市政府新聞後半 | 288,397 | 10,918 | +277,479 |
| 立法院公報保留會期 | 19,033 | 1,661 | +17,372 |
| PTT 保留 5% | 16,547 | 5,548 | +10,999 |

這份表刻意不以 PTT 或日常訊息語料訓練，故不應把它當作 full 版本在非公共事務語域的等價替代品。使用者應依自己的輸入語域決定是否整合。

## 重建

使用現有 `chiaki-tw-homophone-bigram/pipeline` 的 `extract` 與 `emit` 工具，將政府新聞、NTPC 新聞與立法院詢答語料作為唯一輸入。具體的建置命令與輸入檔案說明見 [`BUILDING.md`](BUILDING.md)。
