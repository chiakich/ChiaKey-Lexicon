#!/usr/bin/env bash
# Stop using the locally installed development lexicon.
#
# Usage:
#   scripts/uninstall-dev-lexicon.sh
#   scripts/uninstall-dev-lexicon.sh --keep-slot
#
# Env overrides:
#   ACTIVE_ROOT  default: ~/Library/Application Support/ChiaKey/Lexicons
#   SLOT         default: local-dev
#
# The script only removes `active` when it points at the selected local slot;
# it refuses to modify an active link that points at a release or another slot.
set -euo pipefail

ACTIVE_ROOT="${ACTIVE_ROOT:-$HOME/Library/Application Support/ChiaKey/Lexicons}"
SLOT="${SLOT:-local-dev}"
KEEP_SLOT=0

usage() {
  cat <<'EOF'
Usage:
  scripts/uninstall-dev-lexicon.sh
  scripts/uninstall-dev-lexicon.sh --keep-slot

Env overrides:
  ACTIVE_ROOT  default: ~/Library/Application Support/ChiaKey/Lexicons
  SLOT         default: local-dev
EOF
}

case "${1:-}" in
  "") ;;
  --keep-slot) KEEP_SLOT=1 ;;
  --help|-h) usage; exit 0 ;;
  *)
    usage >&2
    exit 2
    ;;
esac

active_link="$ACTIVE_ROOT/active"
slot_dir="$ACTIVE_ROOT/versions/$SLOT"

if [[ ! -L "$active_link" ]]; then
  echo "ERROR: $active_link is not a symbolic link; nothing was changed." >&2
  exit 1
fi

if [[ ! -d "$slot_dir" ]]; then
  echo "ERROR: local slot does not exist: $slot_dir" >&2
  exit 1
fi

active_target="$(cd "$active_link" && pwd -P)"
slot_target="$(cd "$slot_dir" && pwd -P)"
if [[ "$active_target" != "$slot_target" ]]; then
  echo "ERROR: active points to $active_target, not local slot $slot_target; nothing was changed." >&2
  exit 1
fi

rm "$active_link"
echo "==> Removed local dev active link: $active_link"

if [[ "$KEEP_SLOT" == 0 ]]; then
  rm -rf "$slot_dir"
  echo "==> Removed local dev slot: $slot_dir"
else
  echo "==> Kept local dev slot: $slot_dir"
fi

cat <<'EOF'

Done. Restart ChiaKey (or re-select the input source), then check Preferences → Update
to download or activate the normal release lexicon.
EOF
