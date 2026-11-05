# 繼承字頻的次要讀音降權實驗

日期：2026-10-29。這是 [多音字精準升權實驗](VanguardPreciseExperiment.zh-TW.md)（PR #50）的反向操作：PR #50 只把被壓低的讀音升回去，刻意沒動被灌滿的次要讀音；本輪處理 ChiaKey repo 的古音黑名單比對實驗（`ChiaKey/tmp/benchmark/archaic-reading-audit/`，2026-10-29）在 18,425 句 LINE 語料 gold set 上找出的六組「次要讀音繼承整個字的頻率、壓過以該讀音為主讀音的字」。

結論先講：六組裡 **四組可以用 repo 既有證據支持降權**（挾、稱、甚、繫），已寫進 `explicit.tsv`；**兩組（衝、波）既有證據反對降權**，只做了對照量測，沒有採用。採用版在同一份 gold set 上整句 +3、字元 +6、沒有任何句子變錯，但量測本身是單向的（見〈限制〉），不能單憑它宣稱沒有成本。

## 範圍與資料層

沿用 PR #50 的資料層與證據：降權列寫在 `sources/chiaki-modern-overlay/explicit.tsv`，tag 為 `polyphone-reading-weight,vanguard-fixed-anchor,reading-demote`；位置由先鋒繁體核心（type 6）的單字順位決定，**其他字全部固定不動**，不重排整群。`explicit.tsv` 在 bigram 匯入之前套用，所以以被降權字為 `current` 的 bigram 會跟著重新校準（`calibrate_bigram_boost` 以 unigram 為錨），不需要另外處理。

腳本：`scripts/lexicon/demote-inherited-readings.py`（`scripts/audit/test_reading_demotion.py` 是單元測試，`scripts/audit/compare-reading-demotion.py` 做前後比較）。

### 放置規則

對每個目標（字、讀音）：取先鋒核心排在它**前面**、且目前在 ChiaKey 同讀音候選中有列的字當「上位錨」；目標移到上位錨中權重最低者之下，往下一個固定錨內插（同一個間隙有多個目標時依先鋒順位分配）；下方沒有固定錨時每個目標往下 0.05。先鋒核心把目標排第一、或目標已經低於全部上位錨時不動，腳本會寫出原因而不猜。

這與 PR #50 的「固定錨插入」是同一套機制，只是方向相反；兩者都不保證先鋒全部字對的相對先後，只投影目標自己的位置。PR #50 用「錨的數量」決定插入位置，在 ChiaKey 錨序與先鋒衝突時會把目標插到先鋒認為應排在它前面的字之上（「折／ㄕㄜˊ」就是這樣跑到「蛇」前面的）；本輪的「低於全部上位錨」規則沒有這個問題。

## 六組證據與決定

tsi 分數取自 libchewing `tsi.csv` 單字列（release 單字頻率的來源）；國教院值是《通用詞頻表》的單字詞頻（每百萬，不分讀音）；先鋒順位是該讀音核心候選的名次。

| 組 | 字／讀音 | tsi（該讀音／最高讀音） | 國教院單字 | 先鋒順位 | 原名次 → 新名次 | 決定 |
| --- | --- | ---: | ---: | ---: | --- | --- |
| 1 | 挾／ㄒㄧㄚˊ | 3100／3100 | 挾 6.8；俠 1.7、匣 0.7、峽 0.5、霞 1.3 | 9（轄 呷 俠 狹 霞 暇 洽 斜 之後） | 1 → 13（斜之下、珨之上） | 降權 |
| 2 | 稱／ㄔㄣˋ | 1003／10031 | 稱 169（幾乎全是 ㄔㄥ）；趁 41.3 | 2（趁 之後） | 1 → 2（趁之下） | 降權 |
| 3 | 甚／ㄕㄜˊ | 2599／2599 | 甚 43.5（ㄕㄣˋ 用法）；蛇 41.7 | 6 | 1 → 7（佘之下） | 降權 |
| 3 | 什／ㄕㄜˊ | 1888／1888（與 ㄕˊ 完全相同） | 什 1.0 | 5 | 2 → 5 | 連帶降權 |
| 3 | 折／ㄕㄜˊ | 1／（ㄓㄜˊ） | — | 3（蛇 舌 之後） | 3 → 3，但權重自 −1.602 降到 −2.103（舌之下） | 修正 PR #50 的插入位置 |
| 4 | 繫／ㄐㄧˋ | 11440／11440 | 繫 11.6；寄 23.3 | 11 | 14 → 14（已低於全部上位錨，不動） | 改 cap 兩個補充詞 |
| 5 | 衝／ㄔㄨㄥˋ | 139／2314 | 衝 28.1；銃 0.23 | 1（銃 第 2） | — | 不採用 |
| 6 | 波／ㄆㄛ | 1／6864 | 波 67.4；坡 5.2、潑 5.1 | 1（頗 坡 潑 之後才是其他） | — | 不採用 |

逐組說明：

- **挾／ㄒㄧㄚˊ 七字並列 −1.446 的原因**：上游 `tsi.csv` 對 挾 俠 狎 轄 填 3100、匣 呷 峽 填 3099、狹 暇 陜 填 3090、霞 填 3000，是手填的佔位階層，不是語料頻率；ChiaKey 再用國教院單字詞頻在同權重之間排序，而國教院不分讀音，「挾」的 6.8 幾乎全來自 ㄒㄧㄝˊ 用法（PR #50 已把 挾／ㄒㄧㄝˊ 升到 −1.373），所以 ㄒㄧㄚˊ 這一格被 挾 搶走。本輪只把 挾 移到 斜 之下；同階層其他字（轄 俠 狎 洽 呷 匣 峽 狹 暇 陜 霞）的次序沒有可信的逐讀音頻率可依，先鋒與國教院也各說各話（先鋒：轄 呷 俠 狹 霞；國教院：呷 狹 轄 俠 霞），因此維持原狀。降權後首選是 轄，與先鋒一致。
- **稱／ㄔㄣˋ**：tsi 的 1003 恰好是 ㄔㄥ 10031 的十分之一，像是按比例分配的值，不是獨立觀測；國教院的 趁 是單字詞 41.3／百萬，稱／ㄔㄣˋ 的詞組證據只有 10.3／百萬且全在詞內（對稱、相稱）。移到 趁 之下，與 疢 之間只有 0.002 的間隔，但排序已確定。
- **甚／ㄕㄜˊ**：tsi 把「甚」的全部 2599 給了 ㄕㄜˊ（ㄕㄣˋ、ㄕㄣˊ 各 1），而國教院 ㄕㄣˋ 詞組證據是 512／百萬（甚至）、ㄕㄜˊ 只有 甚麼 的 26／百萬；甚麼 本身是詞條，不受單字降權影響。什／ㄕㄜˊ 與 ㄕˊ 權重完全相同（同屬 1,631 筆同權重次要讀音），且 PR #50 把 折／ㄕㄜˊ 插到 蛇 之前；不連帶處理這兩個字，蛇 仍然不是首選。
- **繫／ㄐㄧˋ**：先鋒把 繫 排第 11、ChiaKey 排第 14，單字層已經不比參考差；walker 選到 繫 的四句全是 Rime 補充詞 `直繫`（n?A_，−2.281）與 `所繫`（CRA_，−1.948）靠 +1.0 詞長加分壓過「直＋寄」「所＋寄」。依 README 既有的 rare-word-cap 公式 `cap = baseline − 0.30 − 1.0×(字數−1)`：直繫 → −3.646（拆字路徑 −2.346，無同長對手）、所繫 → −2.257（同長對手 瑣記 有效分數 −0.957）。
- **衝／ㄔㄨㄥˋ**：這不是繼承字頻的案例。tsi 給 ㄔㄨㄥˋ 139、ㄔㄨㄥ 2314，已經分開；先鋒把 衝 排第一且領先 銃 1.5（−5.201 對 −6.749）；國教院 衝 28／百萬對 銃 0.23／百萬。gold set 的四句（銃槍、麻銃、銃矛）全是同一個群組的遊戲用語。沒有獨立證據支持 銃 應排在 衝 之前。
- **波／ㄆㄛ**：tsi 單字列是 1，但 ChiaKey 的 −1.55 來自 `post_supplement_phrase_evidence` 的單字補強（libchewing 自己就把 一波、中波、奔波 等詞收 ㄆㄛ 讀音），反映的是台灣口語把 波 讀 ㄆㄛ 的實況；先鋒同樣把 波 排第一。國教院 波 67／百萬對 坡 5.2、潑 5.1，只要一成使用者以 ㄆㄛ 打 波 就超過 坡。沒有獨立證據支持降權。

## 同一份 gold set 的前後數字

基準是本分支 HEAD（`7db2b19`）的本機 dev 建置（與 2026-10-16 的 dev 建置只差一列 `教他`）；gold set 是 ChiaKey repo 的 `gold-line-d1.tsv`（`--dominance 1.0`，18,425 句、124,658 個音節位置），walker 為 `Scripts/eval-walker-goldset.sh` 的 native C++ 實作。使用者安裝的 2026.10.2 在同一份 gold set 是 70.52%／93.45%，與本機 dev 基準不同，本表只比本機建置。

| 版本 | 整句正確 | 字元正確 | 相對基準 |
| --- | ---: | ---: | --- |
| 基準（HEAD dev） | 12,980（70.448%） | 116,472（93.433%） | — |
| 採用版 A（挾 稱 甚 什 折 ＋ 直繫 所繫 cap） | 12,983（70.464%） | 116,478（93.438%） | +3 句、+6 字，**0 句變錯** |
| 對照版 B（A ＋ 衝 銃之下、波 潑之下） | 12,984（70.470%） | 116,482（93.441%） | 相對 A +1 句、+4 字，0 句變錯；但見下文 |

六組逐字錯誤（walker 在 gold 要別的字的位置輸出了被降權字的次數）：

| 讀音 | 被降權字 | 基準 | A | B |
| --- | --- | ---: | ---: | ---: |
| ㄒㄧㄚˊ | 挾 | 7 | 0 | 0 |
| ㄔㄣˋ | 稱 | 5 | 0 | 0 |
| ㄕㄜˊ | 甚 | 4 | 1 | 1 |
| ㄐㄧˋ | 繫 | 4 | 0 | 0 |
| ㄔㄨㄥˋ | 衝 | 4 | 4 | 0 |
| ㄆㄛ | 波 | 3 | 3 | 0 |

這些數字只說明被降權字不再搶到，不等於句子變對：

- ㄒㄧㄚˊ 的七句（婆俠、寸又峽、昇仙峽、高千穗峽、刀匣、霞海）在 A 改成輸出 轄。俠／峽／匣／霞 是四個不同的字，單字順序只能選一個當首選，這些句子要靠詞條或 bigram 才救得了。
- ㄔㄣˋ 的五句有三句整句變對（趁今年、到趁雙、趁股東…），另兩句其他位置仍有錯。
- ㄕㄜˊ 剩下的一句是「做蛇棍」被詞條 `作甚`（−2.42）靠詞長加分吃掉，與單字權重無關。蛇 的錯誤由 4 降到 1。
- 對照版 B 的 銃 四句全部變對（其中一句整句正確），但 坡 的三句錯誤一個都沒少：波 一降，ㄆㄛ 的首選變成 PR #50 升上來的 頗（−1.587），輸出從「波雖然很短」變成「頗雖然很短」。要讓 坡 出線得再把 頗 也壓下去，而 頗／ㄆㄛ 正是 PR #50 用同一套先鋒證據升權的字。這讓 B 的 +1 句只來自 銃，而 銃 的證據只有這份 gold set。
- ㄐㄧˋ 的四句改成輸出 記／值／瑣記：ㄐㄧˋ 這一格 記（170 次）遠多於 寄（27 次），首選 記 是對的；「營業所寄紙箱」改由另一個 Rime 補充詞 `瑣記` 搶走，是同類問題，本輪沒有處理。

## 首選變更與不變項

- 單字讀音首選只改了 3 個：ㄔㄣˋ 稱→趁、ㄕㄜˊ 甚→蛇、ㄒㄧㄚˊ 挾→轄（1,449 個單字讀音中）。
- unigram 權重實際改變 8 列：目標 5 列、cap 2 列，以及 `直繫` 的 ㄒㄧˋ 讀音（n?C_）跟著降到 −3.646——`reading-supplements` 的補充讀音會複製該詞目前最高權重，而 n?C_ 本來就由 `直系`（−1.216）主導，沒有順序影響。
- unigram 少了 9 列：`趁年輕`、`什一世`、`什四世`、`什基爾`、`什肯納`、`什科爾`、`什伯恩`、`什維克`、`什瓦拉`。它們是 `joined_phrase_records_from_bigrams` 只在 previous 字**不是**該讀音首選時才併出的「拐杖詞」（趁 原本輸給 稱、什 原本輸給 甚）；首選修正後不再需要，屬設計行為。`布爾什維克` 等正式詞條仍在。
- bigram 列數不變；193 列以 什（191）、尼→什、阿尼→什 為 current 的 bigram 權重隨 unigram 錨點下移 0.872，是校準的預期結果。沒有以 甚、稱、挾、折 為 current 的 bigram。
- 其他讀音群的全部單字權重與相對順序逐筆不變（`tmp/reading-demotion/u-head.txt` 與 `u-expA.txt` 的 diff 只有上述列）。

## 限制

1. **量測是單向的。** `--dominance 1.0` 只接受最高讀音領先次高 ≥ 1.0 的多音字，否則整句丟掉；挾（領先 0.07）、稱（0.52）、甚（0.07）、繫（0.46）、衝（0.63）、波（0.28）都不符合，所以 gold set 裡**沒有任何一句**以這些讀音打出這些字。降權的成本（想打 衝 卻出 銃）在這份量測裡結構性地看不到，這也是對照版 B 一定會「進步」的原因。要量成本需要另建能固定這些讀音的 gold set（例如 `--word-level` 加 pins），本輪沒有做。
2. gold set 是單一 LINE 群組的語料，銃槍、寸又峽這類專有詞反映該群組的話題。
3. 甚、什 被移到 佘 之下（第 5、7 名）是規則推出的位置，不代表 ㄕㄜˊ 第 4～7 名的實際使用頻率；先鋒核心在這一格只有 9 個字，錨很少。
4. ㄒㄧㄚˊ 的 3100／3099／3090 佔位階層沒有重排；要處理得先有逐讀音的單字頻率，這份資料目前不存在（tsi 是佔位，先鋒與國教院互相矛盾）。
5. 1,631 筆同權重次要讀音沒有一起改，見下節。

## 1,631 筆同權重次要讀音：只分析、不改

腳本同時輸出 `inherited-readings.tsv`：本機 dev 建置的 27,000 多個單字列中，**1,510 個字**的讀音權重與該字最高權重完全相同的有 3,141 筆，扣掉每字一筆主讀音是 **1,631 筆次要讀音**（ChiaKey 實驗在 2026.10.2 上數到 1,742，差異來自 PR #50 已升權與本機建置差異）。以先鋒核心順位檢查：

| 狀態 | 筆數 | 其中目前是該讀音首選 |
| --- | ---: | ---: |
| 先鋒有這個字、且至少一個先鋒排在它前面的字目前在它後面（`reference-would-demote`） | 720 | 14 |
| 先鋒有這個字、順位沒有矛盾（`reference-agrees`） | 500 | 49 |
| 先鋒核心沒有這個字（`no-reference`） | 1,921 | 11 |

720 筆裡 634 筆 tsi 本來就把該讀音當最高分（也就是上游沒有分配過讀音頻率），但絕大多數是罕用字（攭、礜、斁…），排在後面動不動都無感。**真正值得提案的是 14 個目前排第一的**：圴 怔 耶 劗 括 硰 綜 哄 撒 歹 瘥 穹 腌 衰（各自的讀音與先鋒順位在表中）。其中 耶（ㄧㄝ，3542）、括（ㄍㄨㄚ）、綜（ㄗㄨㄥ）、撒（ㄙㄚ）、哄（ㄏㄨㄥˇ）、衰（ㄘㄨㄟ）是常用字，應逐字查讀音規範與台灣用法後再決定；用同一套規則批次套用 720 筆是另一個決定，本輪不做，也不建議在沒有能固定讀音的 gold set 之前做。

## 產物與重現

實驗目錄 `tmp/reading-demotion/`（不進版控）：`baseline-head.db`、`expA.db`、`expB.db`、各自的 `eval.log` 與 `mismatches.tsv`，以及 `u-*.txt`／`b-*.txt` 的 unigram／bigram 傾印。`dist/reading-demotion/` 有腳本輸出的 `explicit-rows.tsv`、`review.tsv`、`caps.tsv`、`inherited-readings.tsv`、`summary.json`。

`baseline-head.db` 是在套用本次 `explicit.tsv` 修改前執行 `prepare-release`，再把 `dist/dev/ChiaKeySource-dev.db` 複製過來；`expA.db` 是套用後的同一份產物。

```sh
mkdir -p tmp/reading-demotion/baseline-head tmp/reading-demotion/expA
python3 scripts/lexicon/demote-inherited-readings.py --baseline tmp/reading-demotion/baseline-head.db
python3 -m unittest discover -s scripts/audit -p 'test_reading_demotion.py'
../ChiaKey/Scripts/eval-walker-goldset.sh --lexicon tmp/reading-demotion/baseline-head.db --gold tmp/reading-demotion/gold-line-d1.tsv --mismatches tmp/reading-demotion/baseline-head/mismatches.tsv | tee tmp/reading-demotion/baseline-head/eval.log
cargo run --release -- prepare-release
cp dist/dev/ChiaKeySource-dev.db tmp/reading-demotion/expA.db
../ChiaKey/Scripts/eval-walker-goldset.sh --lexicon tmp/reading-demotion/expA.db --gold tmp/reading-demotion/gold-line-d1.tsv --mismatches tmp/reading-demotion/expA/mismatches.tsv | tee tmp/reading-demotion/expA/eval.log
python3 scripts/audit/compare-reading-demotion.py --gold tmp/reading-demotion/gold-line-d1.tsv --before tmp/reading-demotion/baseline-head --after tmp/reading-demotion/expA --before-db tmp/reading-demotion/baseline-head.db --after-db tmp/reading-demotion/expA.db
```

本輪沒有更換 active 詞庫、沒有 push、沒有開 PR。
