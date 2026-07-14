#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
EVIDENCE_DIR="$STATE_DIR/x11-live-session-stability"
MODE=normal
RUNS=1

usage() {
    echo "usage: $0 [--diagnostic|--trace|--core] [--runs COUNT]" >&2
}

while (( $# > 0 )); do
    case "$1" in
        --diagnostic) MODE=diagnostic ;;
        --trace) MODE=trace ;;
        --core) MODE=core ;;
        --runs)
            shift
            (( $# > 0 )) || { usage; exit 2; }
            RUNS="$1"
            ;;
        --runs=*) RUNS="${1#--runs=}" ;;
        -h|--help) usage; exit 0 ;;
        *) usage; exit 2 ;;
    esac
    shift
done

if [[ ! "$RUNS" =~ ^[0-9]+$ ]] || (( RUNS < 1 || RUNS > 20 )); then
    echo "--runs must be an integer from 1 through 20." >&2
    exit 2
fi
if [[ "$MODE" != normal && "$RUNS" != 1 ]]; then
    echo "Diagnostic modes cannot be combined with --runs." >&2
    exit 2
fi
if [[ ! -t 0 ]] || [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run this from a dedicated local text TTY." >&2
    exit 1
fi
for process in river niri sway Hyprland kwin_wayland Xorg; do
    if pgrep -x "$process" >/dev/null 2>&1; then
        echo "Refusing to take DRM ownership while $process is active." >&2
        exit 1
    fi
done
if [[ "$MODE" == diagnostic ]] && ! command -v gdb >/dev/null 2>&1; then
    echo "Diagnostic mode requires gdb." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
chmod 700 "$STATE_DIR" "$EVIDENCE_DIR"
cd "$ROOT_DIR"

if [[ "$MODE" == diagnostic ]]; then
    env RUSTFLAGS="${RUSTFLAGS:-} -C debuginfo=2" CARGO_INCREMENTAL=0 \
        cargo build --release --offline -p sophia-cli --features atomic-scanout-live
else
    cargo build --release --offline -p sophia-cli --features atomic-scanout-live
fi
tools/atomic_scanout_preflight.sh

session=(
    "$ROOT_DIR/target/release/sophia"
    sophia-live-session
    --display=:181
    --native-scanout
    --max-runtime-ms=30000
    --inject-text=sophia
    --exit-after-input-proof
)

run_session() {
    local evidence="$1"
    local diagnostic="${2:-0}"
    local diagnostic_env=()
    if [[ "$diagnostic" == 1 ]]; then
        diagnostic_env=(SOPHIA_LIVE_SESSION_DIAGNOSTIC=1 MALLOC_CHECK_=3 MESA_DEBUG=1)
    fi
    env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 "${diagnostic_env[@]}" \
        timeout --foreground 45s "${session[@]}" >"$evidence" 2>&1
    "$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$evidence"
}

case "$MODE" in
    diagnostic)
        evidence="$EVIDENCE_DIR/x11-live-session-diagnostic.log"
        gdb_log="$evidence.gdb.log"
        set +e
        env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
            SOPHIA_LIVE_SESSION_DIAGNOSTIC=1 MALLOC_CHECK_=3 MESA_DEBUG=1 \
            timeout --foreground 45s gdb --batch --return-child-result \
                -ex 'set pagination off' \
                -ex 'set confirm off' \
                -ex 'set follow-fork-mode parent' \
                -ex 'set detach-on-fork on' \
                -ex run \
                -ex 'thread apply all bt full' \
                --args "${session[@]}" >"$gdb_log" 2>&1
        status=$?
        set -e
        install -m 600 "$gdb_log" "$evidence"
        if (( status != 0 )); then
            echo "X11 live-session diagnostic failed; GDB evidence: $gdb_log" >&2
            exit "$status"
        fi
        "$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$gdb_log"
        ;;
    trace)
        evidence="$EVIDENCE_DIR/x11-live-session-trace.log"
        run_session "$evidence" 1
        ;;
    core)
        evidence="$EVIDENCE_DIR/x11-live-session-core.log"
        core_file="$evidence.core"
        rm -f "$EVIDENCE_DIR"/core "$EVIDENCE_DIR"/core.* "$core_file"
        set +e
        (
            cd "$EVIDENCE_DIR"
            ulimit -c 262144
            env SOPHIA_RUN_REAL_ATOMIC_SCANOUT_SMOKE=1 \
                timeout --foreground 45s "${session[@]}"
        ) >"$evidence" 2>&1
        status=$?
        set -e
        captured_core="$(find "$EVIDENCE_DIR" -maxdepth 1 -type f -name 'core*' -print -quit)"
        if [[ -n "$captured_core" ]]; then
            install -m 600 "$captured_core" "$core_file"
        fi
        if (( status != 0 )); then
            if [[ -f "$core_file" ]]; then
                echo "X11 live-session failed; core evidence: $core_file" >&2
            else
                echo "X11 live-session failed without a captured core." >&2
            fi
            exit "$status"
        fi
        "$ROOT_DIR/tools/verify_live_session_persistent_evidence.sh" "$evidence"
        ;;
    normal)
        for (( run = 1; run <= RUNS; run += 1 )); do
            evidence="$EVIDENCE_DIR/x11-live-session-run-$run.log"
            echo "X11 live-session stability run $run/$RUNS"
            run_session "$evidence"
        done
        echo "X11 live-session stability proof passed: $RUNS/$RUNS runs"
        ;;
esac
