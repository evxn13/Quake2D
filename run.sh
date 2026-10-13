#!/usr/bin/env bash
# Lance LibreQuake en vue du dessus.
# Usage : ./run.sh [options de QuakeSpasm…]   ex. : ./run.sh +map lq_e1m1 +skill 1
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release -q

# Sous Hyprland, le pavé tactile est coupé pendant la frappe : impossible de
# viser en se déplaçant. On le réactive le temps de la partie, puis on remet
# la valeur d'origine (la configuration sur disque n'est pas modifiée).
set_dwt() {
  hyprctl eval "hl.config({ input = { touchpad = { disable_while_typing = $1 } } })" 2>/dev/null | grep -q '^ok' \
    || hyprctl keyword input:touchpad:disable_while_typing "$1" >/dev/null
}
if command -v hyprctl >/dev/null && [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
  if hyprctl getoption input:touchpad:disable_while_typing -j 2>/dev/null | grep -q '"bool": *true'; then
    set_dwt false
    trap 'set_dwt true' EXIT
  fi
fi

LD_PRELOAD="$PWD/target/release/libquake2d.so" bin/quakespasm-sym -basedir data/librequake "$@"
