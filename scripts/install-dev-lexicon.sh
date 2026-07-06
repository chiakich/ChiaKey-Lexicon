#!/usr/bin/env bash
# Build the dev lexicon and install it into the local ChiaKey "active" slot so
# the IME picks it up. Mirrors the active-folder layout:
#
#   $ACTIVE_ROOT/
#   ├── active                  -> versions/$SLOT   (symlink the IME follows)
#   └── versions/
#       ├── $SLOT/
#       │   ├── ChiaKeySource.db          (built ChiaKeySource-dev.db, renamed)
#       │   ├── metadata.json             (built ChiaKeySource-dev.json, renamed)
#       │   └── lexicon-manifest.json
#       └── $SLOT-backup-<timestamp>/     (previous slot, kept before overwrite)
#
# Usage:
#   scripts/install-dev-lexicon.sh             # build, back up, install, activate
#   scripts/install-dev-lexicon.sh --no-build  # install the existing dist/dev build
#
# Env overrides:
#   ACTIVE_ROOT  default: ~/Library/Application Support/ChiaKey/Lexicons
#   SLOT         default: local-dev
#   RESTART_IME  default: 1
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ACTIVE_ROOT="${ACTIVE_ROOT:-$HOME/Library/Application Support/ChiaKey/Lexicons}"
SLOT="${SLOT:-local-dev}"
DIST_DIR="$REPO_ROOT/dist/dev"
RESTART_IME="${RESTART_IME:-1}"
PROCESS_NAME="ChiaKey"
APP_NAME="ChiaKey.app"
LEGACY_PROCESS_NAME="千秋輸入法"
LEGACY_APP_NAME="千秋輸入法.app"

rewrite_manifest_for_active_slot() {
  local source="$1"
  local target="$2"
  /usr/bin/ruby -rjson - "$source" "$target" <<'RUBY'
source, target = ARGV
manifest = JSON.parse(File.read(source))

Array(manifest["artifacts"]).each do |artifact|
  next unless artifact.is_a?(Hash)

  case artifact["kind"]
  when "chiakey-source-db"
    artifact["filename"] = "ChiaKeySource.db"
  when "metadata"
    artifact["filename"] = "metadata.json"
  end
end

File.write(target, JSON.pretty_generate(manifest) + "\n")
RUBY
}

build=1
[[ "${1:-}" == "--no-build" ]] && build=0

if [[ "$build" == 1 ]]; then
  echo "==> Building dev lexicon (prepare-release)"
  ( cd "$REPO_ROOT" && cargo run --release -- prepare-release )
fi

db_src="$DIST_DIR/ChiaKeySource-dev.db"
meta_src="$DIST_DIR/ChiaKeySource-dev.json"
manifest_src="$DIST_DIR/lexicon-manifest.json"
for f in "$db_src" "$meta_src" "$manifest_src"; do
  [[ -f "$f" ]] || { echo "ERROR: missing build artifact: $f" >&2; exit 1; }
done

versions_dir="$ACTIVE_ROOT/versions"
slot_dir="$versions_dir/$SLOT"
mkdir -p "$versions_dir"

if [[ -d "$slot_dir" ]]; then
  backup="$versions_dir/$SLOT-backup-$(date +%Y%m%d%H%M%S)"
  echo "==> Backing up existing slot -> $backup"
  mv "$slot_dir" "$backup"
fi

echo "==> Installing into $slot_dir"
mkdir -p "$slot_dir"
cp "$db_src"       "$slot_dir/ChiaKeySource.db"
cp "$meta_src"     "$slot_dir/metadata.json"
rewrite_manifest_for_active_slot "$manifest_src" "$slot_dir/lexicon-manifest.json"

echo "==> Pointing 'active' -> versions/$SLOT"
ln -sfn "$versions_dir/$SLOT" "$ACTIVE_ROOT/active"

echo "==> Done. Active lexicon:"
ls -l "$ACTIVE_ROOT/active"
ls -l "$slot_dir"

# Surface the installed DB hash so you can confirm the runtime loaded this build.
if command -v shasum >/dev/null 2>&1; then
  db_hash="$(shasum -a 256 "$slot_dir/ChiaKeySource.db" | awk '{print $1}')"
else
  db_hash="$(sha256sum "$slot_dir/ChiaKeySource.db" | awk '{print $1}')"
fi
echo
echo "ChiaKeySource.db sha256: $db_hash"

if [[ "$RESTART_IME" == "1" ]]; then
  echo
  echo "==> Restarting ChiaKey input method process"
  /usr/bin/pkill -x "$PROCESS_NAME" >/dev/null 2>&1 || true
  /usr/bin/pkill -f "$APP_NAME/Contents/MacOS/$PROCESS_NAME" >/dev/null 2>&1 || true
  /usr/bin/pkill -x "$LEGACY_PROCESS_NAME" >/dev/null 2>&1 || true
  /usr/bin/pkill -f "$LEGACY_APP_NAME/Contents/MacOS/$LEGACY_PROCESS_NAME" >/dev/null 2>&1 || true
  echo "Restarted. Switch back to ChiaKey if macOS does not reactivate it automatically."
else
  echo "Restart the IME (or re-select the input source) to load the new database."
fi
