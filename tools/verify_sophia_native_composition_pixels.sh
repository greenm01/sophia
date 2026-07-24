#!/usr/bin/env bash
set -euo pipefail

evidence_file="${1:-/tmp/sophia-kitty-tty3-launch.log}"
[[ -f "$evidence_file" ]] || {
    echo "missing Sophia composition evidence: $evidence_file" >&2
    exit 1
}

cpu_line="$(grep 'sophia_native_composition_pixels schema=1 status=read stage=cpu ' "$evidence_file" | head -n 1 || true)"
dmabuf_line="$(grep 'sophia_native_composition_pixels schema=1 status=read stage=dmabuf ' "$evidence_file" | head -n 1 || true)"
[[ -n "$cpu_line" ]] || {
    echo "missing readable CPU composition evidence" >&2
    exit 1
}
[[ -n "$dmabuf_line" ]] || {
    echo "missing readable DMA-BUF composition evidence" >&2
    exit 1
}

field() {
    local line="$1" name="$2" token
    for token in $line; do
        if [[ "$token" == "$name="* ]]; then
            printf '%s\n' "${token#*=}"
            return 0
        fi
    done
    return 1
}

cpu_checksum="$(field "$cpu_line" checksum)"
dmabuf_checksum="$(field "$dmabuf_line" checksum)"
cpu_rgb="$(field "$cpu_line" nonzero_rgb_pixels)"
dmabuf_rgb="$(field "$dmabuf_line" nonzero_rgb_pixels)"
dmabuf_alpha_zero="$(field "$dmabuf_line" alpha_zero_pixels)"
dmabuf_alpha_partial="$(field "$dmabuf_line" alpha_partial_pixels)"
dmabuf_alpha_opaque="$(field "$dmabuf_line" alpha_opaque_pixels)"

if [[ "$cpu_checksum" == "$dmabuf_checksum" ]]; then
    echo "sophia_native_composition_pixel_verdict schema=1 status=failed boundary=client_layer_no_framebuffer_delta cpu_checksum=$cpu_checksum dmabuf_checksum=$dmabuf_checksum cpu_rgb=$cpu_rgb dmabuf_rgb=$dmabuf_rgb"
    exit 1
fi
if (( dmabuf_rgb <= cpu_rgb )); then
    echo "sophia_native_composition_pixel_verdict schema=1 status=failed boundary=client_layer_no_visible_rgb_delta cpu_checksum=$cpu_checksum dmabuf_checksum=$dmabuf_checksum cpu_rgb=$cpu_rgb dmabuf_rgb=$dmabuf_rgb alpha_zero=$dmabuf_alpha_zero alpha_partial=$dmabuf_alpha_partial alpha_opaque=$dmabuf_alpha_opaque"
    exit 1
fi
echo "sophia_native_composition_pixel_verdict schema=1 status=passed boundary=client_layer_visible cpu_checksum=$cpu_checksum dmabuf_checksum=$dmabuf_checksum cpu_rgb=$cpu_rgb dmabuf_rgb=$dmabuf_rgb alpha_zero=$dmabuf_alpha_zero alpha_partial=$dmabuf_alpha_partial alpha_opaque=$dmabuf_alpha_opaque"
