# Rime essay

Source id: `rime-essay`

This source imports Rime's shared vocabulary and language model as a low-priority supplemental phrase layer. Because `essay.txt` does not include Zhuyin readings, the release builder only imports entries whose readings can be inferred from the current database's single-character readings.

Pinned upstream file:

- `essay.txt` from `rime/rime-essay` commit `48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed`

Local raw files are downloaded to `sources/rime-essay/raw/` by:

```sh
Scripts/fetch-modern-sources.rb
```

The raw file is intentionally not tracked in git. Its checksum is recorded in `source-inventory.sha256`.

License: LGPL-3.0.
