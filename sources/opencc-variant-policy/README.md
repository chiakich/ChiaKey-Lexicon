# OpenCC 衍生繁中詞形政策

## 來源代號

`opencc-variant-policy`

## 資料層

校正層

## 用途與定位

此來源收錄小型、人工審查過的繁中字形偏好政策，依 OpenCC 簡繁轉換規則衍生。

它不是頻率詞典；release builder 僅在候選同分時用於下調簡體或非台灣偏好詞形，改善預設繁中排序。

## 檔案與格式

`variant-demotions.tsv`：

```text
phrase<TAB>max_weight<TAB>tags
```

`max_weight` 為上限值：

- 候選權重大於上限時，降到上限。
- 候選權重已低於上限時，保持不變。

## Release 匯入規則

作為 policy cap 套用於指定詞形，避免簡體/非偏好詞形在 tie-break 時錯誤領先。

## 上游與授權

上游參考：<https://github.com/BYVoid/OpenCC>

此政策表為專案整理後的衍生規則。

## 驗證

此來源屬於 internal（專案詞庫或校正層）資料。

- release 流程不產生 `source-inventory.sha256`
- 不需要額外進行 inventory 驗證
