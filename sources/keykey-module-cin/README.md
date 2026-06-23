# KeyKey module CIN tables

Source id: `keykey-module-cin`

This source vendors original Yahoo KeyKey / OpenVanilla CIN tables used by
runtime modules outside the Smart Mandarin language model:

- `vendor/cj-ext.cin` -> `Generic-cj-cin`
- `vendor/simplex-ext.cin` -> `Generic-simplex-cin`
- `vendor/cj-punctuations-halfwidth.cin` -> `Punctuations-cj-halfwidth-cin`
- `vendor/cj-punctuations-mixedwidth.cin` -> `Punctuations-cj-mixedwidth-cin`
- `vendor/bopomofo-correction.cin` -> `BopomofoCorrection-bopomofo-correction-cin`

Upstream paths are under:

```text
YahooKeyKey-Source-1.1.2528/DataTables/
```

The release builder imports these as module SQLite key/value tables. They
are not merged into `unigrams`, so they do not affect Smart Mandarin candidate
ranking.

Verify vendored files with:

```sh
cd sources/keykey-module-cin
shasum -a 256 -c source-inventory.sha256
```
