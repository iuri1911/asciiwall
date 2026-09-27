#!/bin/bash
# Install, update, or remove asciiwall on an Omarchy system.
# Idempotent: ./scripts/install.sh is also the update command.
#
#   ./scripts/install.sh
#   ./scripts/install.sh uninstall
#   ./scripts/install.sh uninstall --purge
#   ./scripts/install.sh --dest /tmp/root --no-build --binary ./target/release/asciiwall
#
# --dest installs files under that directory instead of $HOME and skips
# systemctl and `omarchy hook install`. It is the sandbox used by tests.

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEST=${HOME}
NO_SYSTEMD=0
NO_BUILD=0
UNINSTALL=0
PURGE=0
BINARY="$ROOT/target/release/asciiwall"

usage() {
  echo "Usage: $0 [uninstall] [--purge] [--dest DIR] [--no-systemd] [--no-build] [--binary PATH]" >&2
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    uninstall) UNINSTALL=1 ;;
    --purge) PURGE=1 ;;
    --dest)
      DEST=$2
      NO_SYSTEMD=1
      shift
      ;;
    --no-systemd) NO_SYSTEMD=1 ;;
    --no-build) NO_BUILD=1 ;;
    --binary)
      BINARY=$2
      shift
      ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage ;;
  esac
  shift
done

if [[ $DEST != "$HOME" ]]; then
  NO_SYSTEMD=1
fi

BIN_DIR="$DEST/.local/bin"
UNIT_DIR="$DEST/.config/systemd/user"
HOOK_DIR="$DEST/.config/omarchy/hooks/theme-set.d"
MENU="$DEST/.config/omarchy/extensions/omarchy-menu.jsonc"
CANON="$ROOT/contrib/omarchy-menu.jsonc"
PLUGIN_ID="asciiwall.picker"
PLUGIN_DIR="$DEST/.config/omarchy/plugins/$PLUGIN_ID"

merge_menu() {
  local mode=$1
  python3 - "$MENU" "$CANON" "$mode" <<'PY'
import pathlib, sys
dest, canon, mode = sys.argv[1:]
path = pathlib.Path(dest)
text = path.read_text() if path.exists() else """{
  // Extend the Quickshell Omarchy menu with JSONC.
}
"""

def key_of(line):
    s = line.strip()
    if not s.startswith('"'):
        return None
    end = s.find('"', 1)
    return s[1:end] if end > 1 else None

def owned(key):
    return key == "style.background" or (key or "").startswith("style.asciiwall")

lines = text.splitlines()
kept = [ln for ln in lines if not owned(key_of(ln))]
if mode == "install":
    rows = []
    for ln in pathlib.Path(canon).read_text().splitlines():
        if key_of(ln):
            rows.append(ln.strip().rstrip(","))
    if not rows:
        sys.exit("contrib menu has no entries")
    inserted = ["  " + row + ("," if i < len(rows) - 1 else "") for i, row in enumerate(rows)]
else:
    inserted = []

idx = next((i for i in range(len(kept) - 1, -1, -1) if kept[i].strip() == "}"), None)
if idx is None:
    sys.exit(f"{dest} has no closing brace")
if idx > 0:
    prev = kept[idx - 1].rstrip()
    if prev.strip().startswith('"'):
        kept[idx - 1] = prev[:-1] if prev.endswith(",") and not inserted else (prev if prev.endswith(",") or not inserted else prev + ",")
        if inserted and not kept[idx - 1].rstrip().endswith(","):
            kept[idx - 1] = kept[idx - 1].rstrip() + ","
    elif inserted and prev.strip() and not prev.strip().startswith("//") and not prev.strip().endswith("{") and not prev.strip().endswith(","):
        kept[idx - 1] = prev + ","
kept[idx:idx] = inserted
path.parent.mkdir(parents=True, exist_ok=True)
tmp = path.with_suffix(".jsonc.tmp")
tmp.write_text("\n".join(kept) + "\n")
tmp.replace(path)
PY
}

# The animated scene picker is an Omarchy shell plugin. The shell discovers it
# asynchronously after a rescan, and answers "unknown" to an earlier enable.
enable_plugin() {
  if ! omarchy-shell shell rescanPlugins >/dev/null 2>&1; then
    echo "note: omarchy-shell is not running; afterwards run: omarchy plugin enable $PLUGIN_ID" >&2
    return
  fi
  for _ in {1..40}; do
    omarchy-shell shell listPlugins 2>/dev/null |
      jq -e --arg id "$PLUGIN_ID" 'any(.[]; .id == $id)' >/dev/null && break
    sleep 0.05
  done
  omarchy plugin enable "$PLUGIN_ID" >/dev/null ||
    echo "note: \`asciiwall pick\` uses the still picker until: omarchy plugin enable $PLUGIN_ID" >&2
}

if [[ $UNINSTALL -eq 1 ]]; then
  if [[ $NO_SYSTEMD -eq 0 ]]; then
    systemctl --user disable --now asciiwall.service || true
  fi
  rm -f "$UNIT_DIR/asciiwall.service" "$HOOK_DIR/asciiwall" "$BIN_DIR/asciiwall"
  if [[ $NO_SYSTEMD -eq 0 ]]; then
    omarchy-shell shell setPluginEnabled "$PLUGIN_ID" false >/dev/null 2>&1 || true
  fi
  rm -rf "$PLUGIN_DIR"
  if [[ $NO_SYSTEMD -eq 0 ]]; then
    systemctl --user daemon-reload || true
    omarchy-shell shell rescanPlugins >/dev/null 2>&1 || true
  fi
  if [[ -f $MENU ]]; then
    merge_menu uninstall
  fi
  if [[ $PURGE -eq 1 ]]; then
    rm -rf "$DEST/.config/asciiwall" "$DEST/.local/share/asciiwall" "$DEST/.local/state/asciiwall"
  fi
  echo "uninstalled asciiwall from $DEST"
  exit 0
fi

if [[ $NO_SYSTEMD -eq 0 ]] && ! command -v omarchy >/dev/null; then
  echo "asciiwall installs into Omarchy. \`omarchy\` was not found on PATH." >&2
  exit 1
fi

if [[ $NO_BUILD -eq 0 ]]; then
  if ! command -v cargo >/dev/null; then
    echo "Rust is required to build asciiwall. On Omarchy: omarchy pkg add rustup && rustup default stable" >&2
    exit 1
  fi
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi

if [[ ! -x $BINARY ]]; then
  echo "binary not found: $BINARY" >&2
  exit 1
fi

install -Dm755 "$BINARY" "$BIN_DIR/asciiwall"
install -Dm644 "$ROOT/contrib/asciiwall.service" "$UNIT_DIR/asciiwall.service"
if [[ $NO_SYSTEMD -eq 0 ]]; then
  omarchy hook install theme-set "$ROOT/contrib/asciiwall"
else
  install -Dm755 "$ROOT/contrib/asciiwall" "$HOOK_DIR/asciiwall"
fi
merge_menu install
# The shell keeps a loaded keepLoaded overlay on its old code until it restarts.
PLUGIN_SRC="$ROOT/contrib/omarchy-plugin/$PLUGIN_ID"
PLUGIN_CHANGED=0
if [[ -d $PLUGIN_DIR ]] && ! diff -rq "$PLUGIN_SRC" "$PLUGIN_DIR" >/dev/null 2>&1; then
  PLUGIN_CHANGED=1
fi
# The shell watches the plugins directory and loads files as they appear; a
# half-copied plugin gets cached as broken ("File name case mismatch") until
# the shell restarts. Stage the copy outside that directory, then move it in.
mkdir -p "$(dirname "$PLUGIN_DIR")"
PLUGIN_STAGE=$(mktemp -d "$(dirname "$(dirname "$PLUGIN_DIR")")/.$PLUGIN_ID.XXXXXX")
cp -R "$PLUGIN_SRC/." "$PLUGIN_STAGE"
chmod 755 "$PLUGIN_STAGE"
rm -rf "$PLUGIN_DIR"
mv "$PLUGIN_STAGE" "$PLUGIN_DIR"

if [[ $NO_SYSTEMD -eq 0 ]]; then
  systemctl --user daemon-reload
  systemctl --user enable --now asciiwall.service
  systemctl --user restart asciiwall.service
  enable_plugin
  if [[ $PLUGIN_CHANGED -eq 1 ]]; then
    echo "note: the picker plugin changed; run omarchy-restart-shell to load it" >&2
  fi
fi

echo "installed $BIN_DIR/asciiwall"
echo "theme-set hook: $HOOK_DIR/asciiwall"
echo "menu: $MENU"
echo "picker plugin: $PLUGIN_DIR"
echo "tempo: Style → ASCII Wallpaper → Tempo, or \`asciiwall tempo low|medium|high\`"
