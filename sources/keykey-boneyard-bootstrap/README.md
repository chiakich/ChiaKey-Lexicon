# KeyKey Boneyard Bootstrap Source

This source entry documents the first Chiaki KeyKey Lexicon seed release.

The raw files are not copied into this repository. They are read from a local checkout of:

```text
../KeyKey-Boneyard/YahooKeyKey-Source-1.1.2528
```

Run the release preparation script from the repository root:

```sh
Scripts/prepare-v1-release.rb
```

The script expects `../KeyKey-Boneyard` by default. Override it with:

```sh
KEYKEY_BONEYARD_ROOT=/path/to/KeyKey-Boneyard Scripts/prepare-v1-release.rb
```

The script writes:

- `sources/keykey-boneyard-bootstrap/source-inventory.sha256`
- `normalized/smart-mandarin.tsv`
- `manifests/lexicon-manifest.json`
- `dist/<version>/KeyKeySource-<version>.db`
- `dist/<version>/KeyKeySource-<version>.json`
- `dist/<version>/lexicon-manifest.json`
- `dist/<version>/SHA256SUMS`

