#!/usr/bin/env bash
# Mesure timedemo sous Xvfb (OpenGL logiciel), avec ou sans le mod.
# Usage : ./bench.sh [mod|base] LARGEUR HAUTEUR démo...
set -uo pipefail
cd "$(dirname "$0")"
# shellcheck source=/dev/null
source quake2d.conf
mode=$1 w=$2 h=$3; shift 3
export SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy
preload=""; [[ $mode == mod ]] && preload="$PWD/target/release/libquake2d.so"
for demo in "$@"; do
  log=$(mktemp)
  xvfb-run -a -s "-screen 0 1600x1000x24" env LD_PRELOAD="$preload" stdbuf -oL \
    bin/quakespasm -basedir "$BASEDIR" -window -width "$w" -height "$h" -nosound +timedemo "$demo" >"$log" 2>&1 &
  pid=$!
  # timedemo ne quitte pas le jeu : on l'arrête dès que le résultat s'affiche.
  until grep -q 'seconds' "$log" || ! kill -0 $pid 2>/dev/null; do sleep 1; done
  echo "$mode ${w}x${h} $demo : $(grep -E 'frames .* seconds' "$log" | tail -1)"
  pkill -P $pid 2>/dev/null; kill $pid 2>/dev/null; wait $pid 2>/dev/null
  pkill -x quakespasm 2>/dev/null
  rm -f "$log"
done
