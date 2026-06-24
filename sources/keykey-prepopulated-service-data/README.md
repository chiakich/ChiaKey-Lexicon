# KeyKey prepopulated service data

Source id: `keykey-prepopulated-service-data`

This source vendors the original Yahoo KeyKey prepopulated canned-message data:

- `sources/keykey-prepopulated-service-data/vendor/CannedMessages.plist`

Upstream path:

- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData/CannedMessages.plist`

The release builder stores the plist contents in `prepopulated_service_data`
under `canned_messages`, and writes a positive release timestamp under
`canned_messages_timestamp`.

During release cooking, the payload is augmented before it is written:

- `chiakey-symbols-overlay/symbols.tsv` becomes eight supplemental button
  categories: `補充標點`, `貨幣與標記`, `數字序號`, `補充箭頭`, `補充數學`,
  `勾叉與星號`, `花色與音樂`, and `單位符號`.
- `mozc-emoticon-data` replaces the original annotated `顏文字` category with
  a clean Mozc `Messages` list.

OneKey service data is intentionally omitted. Modern ChiaKey no longer
loads the Yahoo-era OneKey URL launcher, so releases must not ship
`onekey_services` or `onekey_services_timestamp`.

Verify vendored files with:

```sh
cd sources/keykey-prepopulated-service-data
shasum -a 256 -c source-inventory.sha256
```
