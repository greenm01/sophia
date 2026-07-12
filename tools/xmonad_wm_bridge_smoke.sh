#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
xmonad_bin=${SOPHIA_XMONAD_BIN:-}

if [ -z "$xmonad_bin" ] && command -v xmonad >/dev/null 2>&1; then
    xmonad_bin=$(command -v xmonad)
fi

if [ -z "$xmonad_bin" ]; then
    xmonad_source=${SOPHIA_XMONAD_SOURCE:-"$HOME/src/xmonad"}
    xmonad_out=${SOPHIA_XMONAD_NIX_OUT:-/tmp/sophia-xmonad}
    if [ -x "$xmonad_out/bin/xmonad" ]; then
        xmonad_bin="$xmonad_out/bin/xmonad"
    elif [ ! -f "$xmonad_source/flake.nix" ]; then
        echo "xmonad not found; set SOPHIA_XMONAD_BIN or SOPHIA_XMONAD_SOURCE" >&2
        exit 1
    else
        nix build "$xmonad_source#defaultPackage.x86_64-linux" \
            --no-write-lock-file \
            --out-link "$xmonad_out"
        xmonad_bin="$xmonad_out/bin/xmonad"
    fi
fi

exec cargo run \
    --offline \
    --quiet \
    --manifest-path "$repo_root/Cargo.toml" \
    --package sophia-x11-wm-bridge \
    -- xmonad-smoke \
    "--xmonad=$xmonad_bin"
