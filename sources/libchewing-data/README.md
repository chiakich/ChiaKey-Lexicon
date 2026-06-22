# libchewing-data

Source id: `libchewing-data`

This source imports the New Chewing built-in dictionary data as the main modern Traditional Chinese / Zhuyin lexicon layer.

Pinned upstream files:

- `dict/chewing/tsi.csv` from `chewing/libchewing-data` tag `v2026.3.22`
- `dict/chewing/word.csv` from `chewing/libchewing-data` tag `v2026.3.22`
- `dict/chewing/alt.csv` from `chewing/libchewing-data` tag `v2026.3.22`

Local raw files are downloaded to `sources/libchewing-data/raw/` by:

```sh
Scripts/fetch-modern-sources.rb
```

The raw files are intentionally not tracked in git. Their checksums are recorded in `source-inventory.sha256`.

License: LGPL-2.1-or-later, based on the source file `dc:license` headers and libchewing project license.
