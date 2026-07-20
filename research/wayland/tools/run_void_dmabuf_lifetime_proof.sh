#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/sophia"
EVIDENCE_DIR="$STATE_DIR/dmabuf-promotion"
EVIDENCE_FILE="$EVIDENCE_DIR/controlled-lifecycle.log"
DIAGNOSTIC=false
TRACE=false
TRACE_ENV=0
CORE_DUMP=false
CORE_ENV=0
RUNS=1

usage() {
    echo "usage: $0 [--diagnostic|--trace|--core] [--runs COUNT]" >&2
}

while (( $# > 0 )); do
    case "$1" in
        --diagnostic)
            DIAGNOSTIC=true
            ;;
        --trace)
            TRACE=true
            TRACE_ENV=1
            EVIDENCE_FILE="$EVIDENCE_DIR/controlled-lifecycle-trace.log"
            ;;
        --core)
            CORE_DUMP=true
            CORE_ENV=1
            EVIDENCE_FILE="$EVIDENCE_DIR/controlled-lifecycle-core.log"
            ;;
        --runs)
            shift
            if (( $# == 0 )); then
                usage
                exit 2
            fi
            RUNS="$1"
            ;;
        --runs=*)
            RUNS="${1#--runs=}"
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage
            exit 2
            ;;
    esac
    shift
done

enabled_modes=0
if [[ "$DIAGNOSTIC" == true ]]; then
    (( enabled_modes += 1 ))
fi
if [[ "$TRACE" == true ]]; then
    (( enabled_modes += 1 ))
fi
if [[ "$CORE_DUMP" == true ]]; then
    (( enabled_modes += 1 ))
fi
if (( enabled_modes > 1 )); then
    echo "--diagnostic, --trace, and --core cannot be used together." >&2
    exit 2
fi
if [[ ! "$RUNS" =~ ^[0-9]+$ ]] || (( RUNS < 1 || RUNS > 5 )); then
    echo "--runs must be an integer from 1 to 5." >&2
    exit 2
fi
if (( RUNS > 1 )) && [[ "$DIAGNOSTIC" == true || "$TRACE" == true || "$CORE_DUMP" == true ]]; then
    echo "--runs cannot be combined with --diagnostic, --trace, or --core." >&2
    exit 2
fi

if [[ ! -r /etc/os-release ]] || ! grep -Eq '^ID="?void"?$' /etc/os-release; then
    echo "This helper supports Void Linux only." >&2
    exit 1
fi
if [[ ! -t 0 ]] || [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
    echo "Run this proof from a dedicated local text TTY." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
chmod 700 "$STATE_DIR" "$EVIDENCE_DIR"

echo "Sophia DMA-BUF 300-frame lifetime proof"
if (( RUNS == 1 )); then
    echo "  evidence: $EVIDENCE_FILE"
else
    echo "  evidence: $EVIDENCE_DIR/controlled-lifecycle-run-N.log"
    echo "  runs:     $RUNS"
fi

cd "$ROOT_DIR"
if [[ "$DIAGNOSTIC" == true ]]; then
    echo "  mode:     GDB allocator/lifecycle diagnostic"
    exec env SOPHIA_DMABUF_DIAGNOSTIC_FRAMES=300 \
        tools/diagnose_void_dmabuf_heap.sh
fi

if [[ "$TRACE" == true ]]; then
    echo "  mode:     release-timing lifecycle trace"
fi
if [[ "$CORE_DUMP" == true ]]; then
    echo "  mode:     release core capture on SIGABRT"
fi

for (( run = 1; run <= RUNS; run += 1 )); do
    run_evidence="$EVIDENCE_FILE"
    if (( RUNS > 1 )); then
        run_evidence="$EVIDENCE_DIR/controlled-lifecycle-run-$run.log"
        echo "  run $run/$RUNS"
    fi
    env \
        SOPHIA_DMABUF_PRODUCER_FRAMES=300 \
        SOPHIA_DMABUF_FIRST_FRAME_EVIDENCE="$run_evidence" \
        SOPHIA_DMABUF_TRACE="$TRACE_ENV" \
        SOPHIA_DMABUF_CORE_DUMP="$CORE_ENV" \
        tools/wayland_dmabuf_first_frame_hardware_proof.sh
done

if (( RUNS > 1 )); then
    echo "Sophia DMA-BUF lifetime stability proof passed: $RUNS/$RUNS runs"
fi
