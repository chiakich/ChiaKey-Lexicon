# libchewing-data

Source id: `libchewing-data`

This source imports the New Chewing built-in dictionary data as the main modern Traditional Chinese / Zhuyin lexicon layer.

Pinned upstream files:

- `dict/chewing/tsi.csv` from `chewing/libchewing-data` tag `v2026.3.22`
- `dict/chewing/word.csv` from `chewing/libchewing-data` tag `v2026.3.22`
- `dict/chewing/alt.csv` from `chewing/libchewing-data` tag `v2026.3.22`

Local raw files are vendored under `sources/libchewing-data/raw/` so normal
release builds do not need network access. Maintainers refresh pinned snapshots
with:

```sh
cargo run --release -- fetch-modern-sources
```

The raw files are tracked in git. Their checksums are recorded in
`source-inventory.sha256`.

License: LGPL-2.1-or-later, based on the source file `dc:license` headers and libchewing project license.
