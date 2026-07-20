#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST_DIR="${HOME}/.local/bin"
DEST="$DEST_DIR/sophia"
mkdir -p "$DEST_DIR"
cat >"$DEST" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ "\${1:-}" == stop ]]; then
    shift
    exec "$ROOT_DIR/tools/stop_sophia_kitty_session.sh" "\$@"
fi
exec "$ROOT_DIR/tools/run_sophia_kitty_session.sh" "\$@"
EOF
chmod 755 "$DEST"
echo "Installed $DEST"
echo "Run 'sophia' on a dedicated TTY; run 'sophia stop' from another TTY."
