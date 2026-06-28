# 公有領域 BPMF 擴充字表

## 來源代號

`bpmf-ext-cin`

## 資料層

相容性基底詞庫

## 用途與定位

此來源收錄來自 KeyKey Boneyard 的公有領域 `bpmf-ext.cin` 字表，作為低優先級單字補充層。

此層只在 release 準備時補齊缺漏，不會覆蓋既有頻率權重，主要用來填補 `libchewing` 與 bootstrap 可能缺少的單字候選。

## 檔案與格式

主要檔案：

```text
sources/bpmf-ext-cin/vendor/bpmf-ext.cin
```

來源清單（含雜湊）：

```text
sources/bpmf-ext-cin/source-inventory.sha256
```

## Release 匯入規則

1. 只匯入 CJK BMP 字元。
2. 排除非 BMP 與私用區字元。
3. 只新增缺失的精確 `(reading, character)` 配對。
4. 不覆蓋 `libchewing` 的既有單字頻率。

## 上游與授權

檔頭記載此字表由 `opendesktop.org.tw` 的 `phone.cin` 修訂而來，並標示為 Public Domain。

## 補充說明

此層可補上原生/Yahoo 常見的 `ㄨㄛˇ` 候選集合，例如：

```text
我 婐 捰 倭 䂺 婑 䰀 㦱
```
