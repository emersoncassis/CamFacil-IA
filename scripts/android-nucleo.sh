#!/usr/bin/env bash
# Compila o núcleo (`camfacil`) e o ffmpeg mínimo para o app Android e coloca
# os dois em android/app/src/main/jniLibs/<abi>/ como libcamfacil.so e
# libffmpeg.so (executáveis lançados pelo ServidorService; ver ADR 0011).
#
# Uso: scripts/android-nucleo.sh [abi...]     (padrão: arm64-v8a armeabi-v7a)
# Precisa: ANDROID_NDK_HOME, `cargo install cargo-ndk` e os alvos do rustup
#   (aarch64-linux-android, armv7-linux-androideabi, x86_64-linux-android).
# CAMFACIL_FFMPEG_TRAB: pasta de trabalho/cache do ffmpeg (padrão: $TMPDIR).
set -euo pipefail

RAIZ=$(cd "$(dirname "$0")/.." && pwd)
cd "$RAIZ"
ABIS=("$@")
[ ${#ABIS[@]} -eq 0 ] && ABIS=(arm64-v8a armeabi-v7a)
: "${ANDROID_NDK_HOME:?defina ANDROID_NDK_HOME}"
ALVO_DIR=${CARGO_TARGET_DIR:-$RAIZ/target}

for ABI in "${ABIS[@]}"; do
  case "$ABI" in
    arm64-v8a)   TRIPLA=aarch64-linux-android ;;
    armeabi-v7a) TRIPLA=armv7-linux-androideabi ;;
    x86_64)      TRIPLA=x86_64-linux-android ;;
    *) echo "ABI desconhecida: $ABI" >&2; exit 1 ;;
  esac
  DEST="$RAIZ/android/app/src/main/jniLibs/$ABI"
  mkdir -p "$DEST"

  cargo ndk -t "$ABI" -P 24 build --release -p camfacil
  cp "$ALVO_DIR/$TRIPLA/release/camfacil" "$DEST/libcamfacil.so"

  if [ ! -f "$DEST/libffmpeg.so" ]; then
    scripts/android-ffmpeg.sh "$ABI" "${CAMFACIL_FFMPEG_TRAB:-${TMPDIR:-/tmp}/cfia-ffmpeg}"
    rm -rf "${CAMFACIL_FFMPEG_TRAB:-${TMPDIR:-/tmp}/cfia-ffmpeg}/build-$ABI"
  fi
  ls -la "$DEST"
done
