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
   結束。
2. **查 bigram**：在 `m_bigramMap[previous]` 找此 context 的 bigram。資料已依機率
   排序，取 `at(0)`（最高者）作為 `bigramResult`。可能不存在。
3. **取 backoff weight**：查 `m_unigramPreviousBackoffs[previous]`，得到 `previous`
   這個 context 的 backoff weight（即 BOW(previous)）；查不到則用預設
   `c_defaultUNKBackoff`。
4. **算 unigram path**：取最高分 unigram `m_unigramCurrents[0]`，分數**加上** backoff
   weight：`result = unigram_logP + backoff(previous)`。若此節點完全沒有 unigram，
   則用 `backoff(previous) + c_defaultUNKProbability`。
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
context 的 backoff weight，兩條路徑取較高分者。

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
- backoff weight 通常 `<= 0`（log 空間），不應為過大正值。
- 偵測明顯單位錯誤：若出現 `0 < p <= 1`（疑似填了原始機率而非 log10），應警示。

### 2. bigram 有效性

一筆 bigram `(previous, current)` 只有在下式成立時才會被 walker 選中：

```
bigram_logP(current | previous)  >  unigram_logP(current) + backoff(previous)
```

驗證時可分級：

- **死權重（dead weight）**：`bigram_logP <= unigram_logP(current) + backoff(previous)`。
  這筆 bigram 永遠不會贏過 unigram，等於沒作用，應標記（可能是權重算錯或可刪）。
- **退化（degenerate）**：`bigram_logP <= unigram_logP(current)`（即使不算 backoff 也輸）。
  幾乎必為錯誤資料。
- backoff(previous) 取不到時，驗證需用預設 `c_defaultUNKBackoff` 代入，與 runtime 一致。

### 3. 排序前提

walker 取 `m_bigramMap[previous].at(0)` 與 `m_unigramCurrents[0]`，**假設資料已依
probability 由高到低排序**。詞庫 build 出的同一 `qstring`／同一 `previous` 群組，
必須維持 `probability DESC`；否則 walker 會誤取到非最高分者。

### 4. 一致性

- 每筆 bigram 的 `current` 應在對應 unigram 表中存在（否則 unigram path 缺基準，
  back-off 比較失真）。
- 同一 `(previous, current)` 不應有重複或矛盾的多筆 bigram。

### 5. 整詞 vs. 拆詞路徑競爭

- 若要判斷「某個 N 字詞會不會被拆詞路徑（如更短的詞 + 單字）比下去」，
  必須先依「詞長加分」章節換算成有效分數（`weight + 1.0×(字數-1)`）再比較，
  否則會低估整詞的實際優勢，得出錯誤的降權門檻。
