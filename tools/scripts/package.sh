#!/usr/bin/env bash
# Builds a portable release folder and tarball of TD2192-Modern for Linux x64 (M13-01, M13-03;
# decision D6: Windows x64 and Linux x64, unsigned).
#
# 1. cargo build --release -p td2192 (and tdtool unless --no-tools). SDL3 is built from source
#    and linked statically, so the build needs cmake, a C/C++ compiler and SDL3's Linux build
#    dependencies (https://wiki.libsdl.org/SDL3/README-linux; on Ubuntu 22.04 the -dev packages for
#    ALSA, PulseAudio, JACK, sndio, X11 (Xext, Xrandr, Xcursor, Xfixes, Xi, Xss, Xtst, xkbcommon),
#    DRM, GBM, EGL/GLES, D-Bus, IBus, udev, PipeWire, Wayland and libdecor).
# 2. Copies the binaries, LICENSE, README.md, THIRD_PARTY.md and the player guide
#    (docs/player/README.md, as PLAYER-GUIDE.md) into <out>/TD2192-Modern-<version>-linux-x64/,
#    and generates THIRD_PARTY_NOTICES.txt with tools/scripts/gen-notices.py.
# 3. Packs the folder as <name>.tar.gz next to it.
#
# The package contains no game data: players supply their own copy of Traffic Department 2192
# (ADR-0003). The output folder must be outside the repository.
#
# Usage: tools/scripts/package.sh --out /path/to/dist [--no-tools] [--target-dir DIR]
#   --target-dir  Cargo target directory (default: the repository's target/). Useful on WSL to
#                 keep Linux build output off the Windows drive.
#   CMAKE         If a machine-wide Cargo config points CMAKE at a Windows cmake (as on the
#                 project workstation), run with CMAKE=/usr/bin/cmake.
set -euo pipefail

out=""
tools=1
target_dir=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --out) out="$2"; shift 2 ;;
        --no-tools) tools=0; shift ;;
        --target-dir) target_dir="$2"; shift 2 ;;
        -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
[[ -n "$out" ]] || { echo "--out is required" >&2; exit 2; }

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
mkdir -p "$out"
out_full="$(cd "$out" && pwd -P)"
case "$out_full/" in
    "$repo"/*) echo "Refusing to write the package inside the repository ($repo)." >&2; exit 1 ;;
esac

cd "$repo"
version="$(cargo metadata --format-version 1 --no-deps \
    | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="td2192"))')"
packages=(-p td2192)
[[ $tools -eq 1 ]] && packages+=(-p tdtool)
if [[ -n "$target_dir" ]]; then
    export CARGO_TARGET_DIR="$target_dir"
fi
cargo build --release "${packages[@]}"
bin_dir="${CARGO_TARGET_DIR:-$repo/target}/release"

name="TD2192-Modern-$version-linux-x64"
# Stage on a Linux filesystem so the tarball keeps the right modes even when --out is a
# Windows drive (WSL's /mnt/* shows every file as 777).
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
dir="$stage/$name"
mkdir -p "$dir"
install -m 755 "$bin_dir/td2192" "$dir/"
[[ $tools -eq 1 ]] && install -m 755 "$bin_dir/tdtool" "$dir/"
install -m 644 LICENSE README.md THIRD_PARTY.md "$dir/"
install -m 644 docs/player/README.md "$dir/PLAYER-GUIDE.md"
python3 tools/scripts/gen-notices.py --out "$dir/THIRD_PARTY_NOTICES.txt" --target x86_64-unknown-linux-gnu
chmod 644 "$dir/THIRD_PARTY_NOTICES.txt"

tarball="$out_full/$name.tar.gz"
rm -f "$tarball"
tar -C "$stage" --owner=0 --group=0 --numeric-owner -czf "$tarball" "$name"
rm -rf "${out_full:?}/$name"
cp -r "$dir" "$out_full/"
tar -tvzf "$tarball"
echo "Package: $tarball"
