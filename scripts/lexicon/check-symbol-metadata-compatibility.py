#!/usr/bin/env python3
"""Parse the actual producer output with an independent plist reader.

Invoked by the Rust compatibility test in Verify and Release CI.
"""
import json
import plistlib
import sys

payload = json.load(sys.stdin)
legacy = plistlib.loads(payload["legacy"].encode())
modern = plistlib.loads(payload["modern"].encode())
assert len(legacy["CannedMessages"]) == len(modern["CannedMessages"])
metadata_count = 0
for old, new in zip(legacy["CannedMessages"], modern["CannedMessages"]):
    metadata = new.pop("SymbolMetadata", {})
    metadata_count += len(metadata)
    if new.get("Name", {}).get("zh_TW") in ("常用符號", "基本符號"):
        # Keys with &, <, > are forbidden (shipped apps do not escape plist keys).
        named = {s for s in new["Buttons"] if not set(s) & set("&<>")}
        assert named <= metadata.keys(), "Common/basic symbol names are incomplete"
    # Old apps ignore unknown category keys, then display/insert Buttons strings.
    assert new == old, "Legacy category fields, ordering, or input strings changed"
    for symbol, fields in metadata.items():
        assert symbol in new["Buttons"]
        assert not set(symbol) & set("&<>")
        assert isinstance(fields["Name"], str) and fields["Name"]
        if symbol == "\u3000":
            assert fields["DisplayLabel"] == "全形空白"
            assert symbol != fields["DisplayLabel"]
    if "Buttons" in new:
        assert all(isinstance(symbol, str) for symbol in new["Buttons"])
assert metadata_count > 0, "Producer failed to attach metadata"
assert legacy == modern, "Optional metadata must be the only format change"
print("Legacy Buttons contract and generated metadata plist passed.")
