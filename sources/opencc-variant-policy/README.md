# OpenCC-derived Traditional Chinese variant policy

Source id: `opencc-variant-policy`

This source contains small, reviewed Traditional Chinese variant preferences
derived from OpenCC's simplified/traditional conversion policy. It is not a
frequency dictionary. The release builder uses it only to demote Simplified or
non-Taiwan-preferred variants when those forms otherwise tie with Traditional
Chinese candidates.

The table format is:

```text
phrase<TAB>max_weight<TAB>tags
```

`max_weight` is an upper bound. Existing candidates above that value are lowered
to the bound; candidates already below it are left unchanged.

Upstream reference:

- <https://github.com/BYVoid/OpenCC>

Verify vendored files with:

```sh
cd sources/opencc-variant-policy
shasum -a 256 -c source-inventory.sha256
```
