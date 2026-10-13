#!/usr/bin/env bash
# Lance Quake en vue du dessus.
# Usage : ./run.sh [options de QuakeSpasm…]   ex. : ./run.sh +skill 1 +map lq_e1m1
set -euo pipefail
cd "$(dirname "$0")"

[[ -x bin/quakespasm && -f quake2d.conf ]] || { echo "Lance d'abord ./install.sh" >&2; exit 1; }
# shellcheck source=/dev/null
source quake2d.conf

# Sous Hyprland, le pavé tactile est coupé pendant la frappe : impossible de
# viser en se déplaçant. On le réactive le temps de la partie, puis on remet
# la valeur d'origine (la configuration sur disque n'est pas modifiée).
set_dwt() {
  # Configuration Lua (Hyprland récent) : eval ; configuration classique : keyword.
  hyprctl eval "hl.config({ input = { touchpad = { disable_while_typing = $1 } } })" 2>/dev/null | grep -q '^ok' \
    || hyprctl keyword input:touchpad:disable_while_typing "$1" >/dev/null
}
if command -v hyprctl >/dev/null && [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
  if hyprctl getoption input:touchpad:disable_while_typing -j 2>/dev/null | grep -q '"bool": *true'; then
    set_dwt false
    trap 'set_dwt true' EXIT
  fi
fi

LD_PRELOAD="$PWD/target/release/libquake2d.so" bin/quakespasm -basedir "$BASEDIR" "$@"
