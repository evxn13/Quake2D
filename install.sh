#!/usr/bin/env bash
# Installe Quake2D : compile QuakeSpasm 0.96.3 et le mod, et récupère les
# données du jeu (LibreQuake, libre et gratuit, ou votre copie de Quake).
#
# Usage : ./install.sh [--deps] [--data DOSSIER]
#   --deps          installe les dépendances système manquantes (via sudo)
#   --data DOSSIER  dossier de jeu existant contenant id1/pak0.pak (Quake
#                   d'origine, par exemple) au lieu de télécharger LibreQuake
set -euo pipefail
cd "$(dirname "$0")"

QUAKESPASM_TAG=quakespasm-0.96.3
QUAKESPASM_REPO=https://github.com/sezero/quakespasm.git
LIBREQUAKE_URL=https://github.com/lavenderdotpet/LibreQuake/releases/download/v0.09-beta/full.zip
LIBREQUAKE_SHA256=623e463b35811216244f9ba15e0c45abc1765288650b5e3d36a899b35e4bcf3d

say()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m==>\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; exit 1; }

install_deps=0
basedir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --deps) install_deps=1 ;;
    --data) basedir="$(realpath "$2")"; shift ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) die "option inconnue : $1 (voir --help)" ;;
  esac
  shift
done

[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] \
  || die "seul Linux x86_64 est pris en charge (le mod lit la mémoire du jeu au format 64 bits)"
if [[ -n "$basedir" && ! -f "$basedir/id1/pak0.pak" ]]; then
  die "$basedir/id1/pak0.pak introuvable : --data doit désigner le dossier qui contient id1/"
fi

# --- Dépendances ---------------------------------------------------------------

missing=()
for tool in git make cc pkg-config curl unzip sha256sum; do
  command -v "$tool" >/dev/null || missing+=("$tool")
done
for lib in sdl2 vorbisfile gl; do
  pkg-config --exists "$lib" 2>/dev/null || missing+=("$lib")
done

if [[ ${#missing[@]} -gt 0 ]]; then
  if command -v pacman >/dev/null; then
    cmd=(sudo pacman -S --needed git base-devel pkgconf curl unzip sdl2 libvorbis mesa)
  elif command -v apt-get >/dev/null; then
    cmd=(sudo apt-get install -y git build-essential pkg-config curl unzip libsdl2-dev libvorbis-dev libgl-dev)
  elif command -v dnf >/dev/null; then
    cmd=(sudo dnf install -y git gcc make pkgconf-pkg-config curl unzip SDL2-devel libvorbis-devel mesa-libGL-devel)
  else
    die "dépendances manquantes : ${missing[*]} — installe-les avec le gestionnaire de paquets de ta distribution"
  fi
  if [[ $install_deps == 1 ]]; then
    say "Installation des dépendances : ${cmd[*]}"
    "${cmd[@]}"
  else
    die "dépendances manquantes : ${missing[*]}
    Installe-les avec :  ${cmd[*]}
    ou relance :         ./install.sh --deps"
  fi
fi

command -v cargo >/dev/null || die "Rust (cargo) est introuvable — installe-le via https://rustup.rs puis relance ce script"

# --- QuakeSpasm ----------------------------------------------------------------

mkdir -p build bin
if [[ ! -d build/quakespasm ]]; then
  say "Téléchargement de QuakeSpasm ($QUAKESPASM_TAG)"
  git clone -q --depth 1 --branch "$QUAKESPASM_TAG" "$QUAKESPASM_REPO" build/quakespasm
fi

say "Compilation de QuakeSpasm (avec ses symboles, dont le mod a besoin)"
log=build/quakespasm.log
# -g passe par l'environnement : en argument de make, il écraserait les options du Makefile.
( cd build/quakespasm/Quake &&
  CFLAGS="-Wall -Wno-trigraphs -MMD -g" make -j"$(nproc)" USE_SDL2=1 USE_CODEC_MP3=0 STRIP=true ) >"$log" 2>&1 \
  || { tail -20 "$log" >&2; die "échec de la compilation de QuakeSpasm (journal : $log)"; }
cp build/quakespasm/Quake/quakespasm bin/quakespasm

# --- Mod -----------------------------------------------------------------------

say "Compilation du mod"
cargo build --release --quiet

# --- Données du jeu ------------------------------------------------------------

if [[ -z "$basedir" ]]; then
  basedir="$PWD/data/librequake"
  if [[ ! -f "$basedir/id1/pak0.pak" ]]; then
    say "Téléchargement de LibreQuake v0.09-beta (116 Mo)"
    mkdir -p data
    curl -fL --progress-bar -o data/librequake.zip "$LIBREQUAKE_URL"
    echo "$LIBREQUAKE_SHA256  data/librequake.zip" | sha256sum -c --quiet \
      || die "l'archive téléchargée est corrompue (somme SHA-256 différente)"
    rm -rf data/librequake.tmp
    unzip -q data/librequake.zip -d data/librequake.tmp
    mv data/librequake.tmp/full "$basedir"
    rm -rf data/librequake.tmp data/librequake.zip
  fi
fi

cat > quake2d.conf <<EOF
# Généré par install.sh — dossier de jeu utilisé par run.sh
BASEDIR="$basedir"
EOF

say "Données du jeu : $basedir"
say "Installation terminée. Lance le jeu avec : ./run.sh"
