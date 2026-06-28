# KeyKey Boneyard Bootstrap 來源

## 來源代號

`keykey-boneyard-bootstrap`

## 資料層

相容性基底詞庫

## 用途與定位

此來源是 ChiaKey Lexicon 的基礎 bootstrap 層。

為了讓 CI 與 release build 不依賴本機 `../KeyKey-Boneyard` checkout，repo 內保留一份已知可用的 cooked database。

## 檔案與格式

實際 bootstrap 輸入：

```text
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db
```

資料庫雜湊：

```text
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db.sha256
```

來源 provenance（當初製作此 DB 所用 upstream 檔案與雜湊）：

```text
sources/keykey-boneyard-bootstrap/source-inventory.sha256
```

## Release 匯入規則

1. `vendor/KeyKeySource.db` 是 release builder 的實際 bootstrap 輸入。
2. `source-inventory.sha256` 提供此 bootstrap DB 的來源可追溯性。
3. `KeyKey-Boneyard` 的完整歷史原始資料不複製進本 repo。

由 repository root 執行 release build：

```sh
cargo run --release -- prepare-release
```

預設使用 vendored bootstrap DB。若要在本機測試其他 DB，可覆寫 `BONEYARD_DB`：

```sh
BONEYARD_DB=/path/to/KeyKeySource.db cargo run --release -- prepare-release
```

建置輸出：

- `normalized/smart-mandarin.tsv`
- `manifests/lexicon-manifest.json`
- `dist/<version>/ChiaKeySource-<version>.db`
- `dist/<version>/ChiaKeySource-<version>.json`
- `dist/<version>/lexicon-manifest.json`
- `dist/<version>/SHA256SUMS`

## 上游與授權

此層為相容性基底資料，延續 KeyKey 既有 schema、metadata 與基本注音資料形狀。

## 驗證

可使用本目錄中的雜湊檔案進行完整性檢查。
