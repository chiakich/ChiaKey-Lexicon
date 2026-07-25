# Walker 計分算法與權重驗證

本文件說明 ChiaKey 輸入引擎（Manjusri / Gramambular 系）的 walker 如何用詞庫中的
unigram、bigram 機率計分，並據此整理出詞庫資料的權重驗證規則。

## 分數單位

所有機率皆為 **log10 機率**（負值，越接近 0 代表越可能）。

- 因為是 log 空間，分數相加 = 機率相乘；分數比大小 = 機率比大小。
- 找不到的詞用 log(0) 地板值 `-99.0`

## 計分流程

每個節點（節點 = 一段讀音對應的候選詞集合）在 walk 時，會帶著前一個詞 `previous`
呼叫 `findHighestScorePair(previous)`：

1. **override 優先**：若該節點被使用者覆寫選字，直接回傳 `c_defaultOverrideScore`，
   結束。（該常數為 `100`，是正值，log 空間中必勝。）
2. **查 bigram**：在 `m_bigramMap[previous]` 找此 context 的 bigram。資料已依機率
   排序，取 `at(0)`（最高者）作為 `bigramResult`。可能不存在。
   **只取第 0 筆**：同一 `(qstring, previous)` 底下可以掛多個 `current`，但走訪器
   不會往下找，第二名以後的列永遠不會被讀取。
3. **取 backoff weight**：查 `m_unigramPreviousBackoffs[previous]`，得到 `previous`
   這個 context 的 backoff weight（即 BOW(previous)）；查不到則用預設
   `c_defaultUNKBackoff`。
   **實務上兩者都是 0**：`Node.cpp` 的 `c_defaultUNKBackoff = 0`，而 release DB 與
   上游 `KeyKeySource.db` 的 `backoff` 欄位也全為 0。等於退避不打折扣，bigram 必須
   硬贏過 unigram path。
4. **算 unigram path**：取 `m_unigramCurrents[0]`，分數**加上** backoff weight：
   `result = unigram_logP + backoff(previous)`。若此節點完全沒有 unigram，
   則用 `backoff(previous) + c_defaultUNKProbability`。
   注意 `m_unigramCurrents[0]` 是節點的**當前排頭**，不一定是分數最高者——見
   〈使用者學習會改變排頭〉。
5. **比大小取 max**：若 bigram 存在且 `bigramResult > result`，回傳 bigram；
   否則回傳 unigram path 的結果。

對應的部份原始碼：

```cpp
// Node.h:304-317
StringScorePair result;
if (m_unigramCurrents.size()) {
  result = m_unigramCurrents[0];
  result.second += backoffWeight;            // unigram + backoff
} else {
  result.first = "";
  result.second = backoffWeight + Node::c_defaultUNKProbability;
}

if (hasBigramResult)
  if (bigramResult.second > result.second)   // 取 max
    return bigramResult;
return result;
```

這是標準的 Katz back-off，bigram 直接用條件機率，unigram 退避時補上
context 的 backoff weight，兩條路徑取較高分者。（但如第 3 點所述，目前 backoff
恆為 0，退避折扣實際上沒有作用。）

## 使用者學習會改變排頭

`Graph.h` 建圖時會查 `fetchCachedOverrideSelection(qstring)`，命中就呼叫
`Node::adjustScoreWithSelection()`。該函式把選中的候選插回 `m_unigramCurrents`
最前面，**但保留它原本的分數**（改分數的幾行在原始碼中被註解掉）。

所以第 4 點的 unigram path 門檻會在 `[該讀音最低分, 該讀音最高分]` 之間浮動，而且
是**往下浮動**。一筆 bigram 現在打不過最高分，不代表它沒用——使用者學了較弱的候選
之後門檻降低，它就會生效。

另有 `user_bigram_cache`（`useUserTable` 開啟時自動啟用）：`findBigrams()` 一旦命中
就直接回傳快取那筆、跳過詞庫所有 bigram，寫入分數為 `0`（必勝）。使用者已學過的
context，詞庫 bigram 不參與競爭。

## 長度先驗

`Node.cpp` 的 `c_phraseLengthBonus = 1.0`（log10，每多一個音節）。`lengthPrior()`
對每個節點加 `1.0 × (音節數 − 1)`，在 `Graph.h` 的路徑計分中累加：

```cpp
ssp.second + node.lengthPrior() + nextPath[0].score
```

整句總加成 = 音節數 − 節點數，**詞越長分數越高**。四字詞當一個節點比拆成兩個二字詞
多 +1.0，遠大於多數 bigram 的作用；bigram 只有在競爭路徑的**節點數相同**時，才對
斷詞有實質影響。

## 詞長加分（phrase length bonus）

上面「計分流程」只涵蓋單一節點的分數。但整句的最佳路徑是把沿路所有節點的分數
**相加**（graph walk，見 `ChiaKey` 的 `Headers/Graph.h`：`Graph::walk` /
`walkMemoized`），而每個節點在相加前，還會先被加上一個**詞長加分**：

```cpp
// Manjusri/Node.cpp:13-14
c_phraseLengthBonus = (Score)1.0;  // log10 per extra syllable

// Manjusri/Node.h:392-395，lengthPrior()
// 單音節或被 override 時回傳 0，否則：
c_phraseLengthBonus * (音節數 - 1)
```

這個加分在 `Graph.h:532` 與 `Graph.h:644` 的路徑分數累加中直接生效
（`ssp.second + node.lengthPrior() + nextPath[0].score`），**不是**候選字清單
排序才用的裝飾，而是會改變 walker 選中的最佳路徑本身。目前專案沒有呼叫
`SetPhraseLengthBonus` 覆寫，所以是編譯期寫死的 `1.0`。

**實務影響**：一個 N 字組成的單一詞節點，比起拆成「(N-1) 字詞 + 1 字」兩個
節點，會多拿到固定 **+1.0** 的加分（`1.0×(N-1)` 對 `1.0×(N-2)+0`），跟 N 的
長度無關。也就是說，即使某詞的 raw log-prob 權重比拆開路徑的權重總和低了
不少，只要沒低過這 +1.0 的差距，它在 walker 實際的最佳路徑搜尋中仍然會贏。

比較拆開路徑更多節點（例如整段退化成逐字）時，加分差距會更大：
`1.0×(N-1)` 對全字元拆分的 `0`，優勢等於 `N-1`。

**校正計分時務必記得**：比較「整詞 vs. 拆開路徑」何者會贏，不能只比較
`normalized/smart-mandarin.tsv` 裡的 raw weight，必須先各自套用
`effective = weight + 1.0 × (字數 - 1)` 換算成有效分數再比大小；`explicit.tsv`
的 reading-demote／`fragment-demotions.tsv` 的降權門檻，也都要用這個有效分數
反推，而不是直接對 raw weight 打一個固定 margin。

## 關於詞庫權重驗證

因為是「取 max」而非「取代」或「相加」，所以一筆 bigram 要真正生效，它的 log 機率
必須贏過對應的 unigram path。據此整理出以下可機械化檢查的規則。

### 1. 數值範圍

- unigram、bigram `probability` 必須為 log10 機率，皆 `<= 0`（地板約 `-99.0`）。
- 偵測明顯單位錯誤：若出現 `0 < p <= 1`（疑似填了原始機率而非 log10），應警示。
- 例外：`c_defaultOverrideScore = 100` 是正值，硬覆寫用，不受此規則約束。

### 2. bigram 有效性

一筆 bigram `(previous, current)` 只有在下式成立時才會被 walker 選中：

```
bigram_logP(current | previous)  >  m_unigramCurrents[0] + backoff(previous)
```

由於 backoff 恆為 0，而排頭會因使用者學習而在 `[lo, hi]`（該讀音候選的分數區間）
之間浮動，可分成四類：

| 類別 | 條件 | 說明 |
| --- | --- | --- |
| **A 不可達** | 不是 `(qstring, previous)` 群組中機率最高者 | `at(0)` 取不到，永不執行 |
| **B 永不勝出** | 是群組第一名，但 `probability <= lo` | 任何學習狀態都贏不了 |
| **C 恆生效** | 是群組第一名，且 `probability > hi` | 任何學習狀態都會被選中 |
| **D 條件生效** | 介於 B 與 C 之間 | 使用者學過較弱候選才生效 |

**只有 A + B 可以安全剪除。** D 是學習翻轉排序後的救援路徑，用「贏不過該讀音最高分」
當死權重判準會誤殺整個 D 類。

檢查工具：`scripts/audit-bigram-effectiveness.mjs`（分類並輸出清單）、
`scripts/prune-dead-bigrams.mjs`（實際剪除）。

### 3. 排序前提

walker 取 `m_bigramMap[previous].at(0)` 與 `m_unigramCurrents[0]`，需要資料依
probability 由高到低排序。

**這件事由 runtime 負責**：`LanguageModel::findBigrams()` / `findUnigrams()` 撈完
資料後會執行 `stable_sort(..., GramCompare<T>())`，SQL 的 `ORDER BY` 反而是註解掉
的。因此詞庫 build 端**不需要**維持排序。

但要注意：同一 qstring 下若最高分**完全並列**，`stable_sort` 會保留輸入順序，等於
排頭由 SQLite 回傳順序決定，實質上是任意的（例如 `載你`／`載妳`、`燒機`／`燒雞`）。
這類讀音只有 bigram 能提供區分依據，是補詞的優先目標。

### 4. 一致性

- 每筆 bigram 的 `current` 應在對應 unigram 表中存在（否則 unigram path 缺基準，
  back-off 比較失真）。
- 同一 `(previous, current)` 不應有重複或矛盾的多筆 bigram。

### 5. 整詞 vs. 拆詞路徑競爭

- 若要判斷「某個 N 字詞會不會被拆詞路徑（如更短的詞 + 單字）比下去」，
  必須先依「詞長加分」章節換算成有效分數（`weight + 1.0×(字數-1)`）再比較，
  否則會低估整詞的實際優勢，得出錯誤的降權門檻。
