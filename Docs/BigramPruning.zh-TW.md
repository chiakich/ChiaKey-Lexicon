# Bigram 可達性稽核與剪枝

說明 `scripts/audit/audit-bigram-effectiveness.mjs` 與 `scripts/lexicon/prune-dead-bigrams.mjs` 的用途、判準與操作流程。

分類規則本身（A/B/C/D 四類的定義與依據）見 [WalkerScoring.zh-TW.md](WalkerScoring.zh-TW.md) 的〈bigram 有效性〉。本文只談工具怎麼用、什麼時候用、以及有哪些前提。

## 前置條件

兩支腳本都直接讀 release DB，因此必須先建置：

```sh
cargo run --release -- prepare-release
```

預設讀 `dist/dev/ChiaKeySource-dev.db`，可用 `--db` 指定其他路徑。

## 稽核

```sh
node scripts/audit/audit-bigram-effectiveness.mjs \
  --prune-out tmp/prunable.tsv \
  --ties-out tmp/tied-qstrings.tsv
```

輸出四個區塊：

**backoff 健檢**。統計 `unigrams.backoff` 的分布。若全為 0，會提示 Katz 退避折扣沒有作用，此時 bigram 必須硬贏過該讀音的最高分候選，是生效條件最嚴苛的情況，後續分類即以此為準。

**可達性分類**。每筆 bigram 歸入 A/B/C/D 其中一類，並列出總計與可安全剪除的數量。

**各來源分項**。同樣的分類按 source 拆開，用於比較各來源的體質。`--sources-dir` 可指定回溯目錄，預設 `sources`。

**最高分並列的讀音**。同一 qstring 下最高分完全相同的候選群組。這類讀音的預設排頭由 SQLite 回傳順序決定，實質上是任意的，只有 bigram 能提供區分依據；其中「完全沒有任何 bigram 覆蓋」的組別是補詞的優先目標。

### 輸出檔

| 選項          | 內容                                             |
| ------------- | ------------------------------------------------ |
| `--prune-out` | A 類與 B 類的完整清單，含 `reason` 與所屬 source |
| `--ties-out`  | 所有並列讀音，含候選詞與是否已有 bigram 覆蓋     |

## 剪枝

```sh
node scripts/lexicon/prune-dead-bigrams.mjs              # 試算，不寫檔
node scripts/lexicon/prune-dead-bigrams.mjs --apply      # 實際寫回 sources/*/bigrams.tsv
```

只剪 A（群組非第一名，`at(0)` 取不到）與 B（機率低於該讀音最弱候選，任何學習狀態都贏不了）。

**C 與 D 都不剪。** D 是使用者學習翻轉排序後的救援路徑：`Node::adjustScoreWithSelection()` 只把選中的候選移到 `m_unigramCurrents` 最前面而不改分數，所以 unigram path 的門檻會往下浮動到該讀音的最低分。以「贏不過該讀音最高分」當死權重判準會誤殺整個 D 類。

### 跨來源共用的 key

同一個 `(qstring, previous, current)` 若出現在多個來源檔，DB 只保留最後匯入的那一筆，無法判斷該從哪一份剪除，預設整組略過並計入「跨來源略過」。

要指定從哪個來源剪，用：

```sh
node scripts/lexicon/prune-dead-bigrams.mjs --prune-shared-from chiaki-web-overlay --apply
```

注意這會改變行為而非單純刪除死列：被剪掉的那份消失後，下次 build 時同 key 的另一份會接手，並以它自己的機率重新校準，可能從失效轉為生效。剪完應重跑稽核確認。

## 前提與限制

**分類隨 DB 狀態變動。** B 的判準是「機率 ≤ 該讀音最弱候選」，因此在某個讀音下新增更弱的候選會使下界下降，部分原本的 B 會轉成 D。大幅改動詞庫後應重跑稽核再決定是否剪枝。

**剪枝不可逆。** 腳本直接改寫 `sources/*/bigrams.tsv`，還原請用 git。建議在乾淨的工作目錄執行，並以 `git diff --numstat` 確認只有刪除、沒有新增或修改。

**calibration 的錨點會位移。** `calibrate_bigram_boost` 計算 `(raw - raw_max_of_source)`，若被剪掉的列剛好是該來源 raw 最高的一筆（A 類有可能），`raw_max` 會下降，導致該來源其餘所有列的 stored 值整體位移。幅度通常極小，但邊界附近的少數列可能因此改變分類。
