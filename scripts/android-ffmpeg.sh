#!/usr/bin/env bash
# Compila um ffmpeg mínimo (LGPL, sem --enable-gpl) para o app Android (ADR 0010).
#
# Só o que o núcleo usa: RTSP/RTP/HTTP → H.264/H.265/MJPEG → fps+scale → MJPEG no
# stdout, mais o lavfi da cena de demonstração. O resultado vira
# android/app/src/main/jniLibs/<abi>/libffmpeg.so (executável, não biblioteca:
# é o truque padrão para rodar binário nativo a partir do nativeLibraryDir).
#
# Uso: scripts/android-ffmpeg.sh <abi> [pasta-de-trabalho]
#   abi: arm64-v8a | armeabi-v7a | x86_64
#   ANDROID_NDK_HOME precisa apontar para o NDK (r26 ou mais novo).
set -euo pipefail

VERSAO=7.1.2
SHA256=089bc60fb59d6aecc5d994ff530fd0dcb3ee39aa55867849a2bbc4e555f9c304
API=24

ABI=${1:?informe a ABI: arm64-v8a | armeabi-v7a | x86_64}
TRAB=${2:-${TMPDIR:-/tmp}/cfia-ffmpeg}
RAIZ=$(cd "$(dirname "$0")/.." && pwd)
NDK=${ANDROID_NDK_HOME:?defina ANDROID_NDK_HOME}
TC="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"

case "$ABI" in
  arm64-v8a)   ARCH=aarch64; CPU=armv8-a; CC="$TC/aarch64-linux-android$API-clang"; EXTRA=() ;;
  armeabi-v7a) ARCH=arm;     CPU=armv7-a; CC="$TC/armv7a-linux-androideabi$API-clang"; EXTRA=(--enable-neon) ;;
  x86_64)      ARCH=x86_64;  CPU=x86-64;  CC="$TC/x86_64-linux-android$API-clang"; EXTRA=(--disable-x86asm) ;;
  *) echo "ABI desconhecida: $ABI" >&2; exit 1 ;;
esac

mkdir -p "$TRAB"
cd "$TRAB"
if [ ! -f "ffmpeg-$VERSAO.tar.xz" ]; then
  curl -sSfLO "https://ffmpeg.org/releases/ffmpeg-$VERSAO.tar.xz"
fi
echo "$SHA256  ffmpeg-$VERSAO.tar.xz" | sha256sum -c -
rm -rf "build-$ABI" && mkdir "build-$ABI"
tar -xf "ffmpeg-$VERSAO.tar.xz" -C "build-$ABI" --strip-components=1
cd "build-$ABI"

./configure \
  --target-os=android --arch="$ARCH" --cpu="$CPU" --enable-cross-compile \
  --cc="$CC" --cxx="$CC++" --ar="$TC/llvm-ar" --nm="$TC/llvm-nm" \
  --ranlib="$TC/llvm-ranlib" --strip="$TC/llvm-strip" \
  --extra-ldflags="-Wl,-z,max-page-size=16384" \
  --enable-pic --enable-small --disable-debug --disable-doc \
  --disable-autodetect --disable-everything \
  --disable-ffplay --disable-ffprobe --enable-ffmpeg \
  --disable-postproc --disable-swresample \
  --enable-avdevice --enable-indev=lavfi \
  --enable-network \
  --enable-protocol=file,pipe,tcp,udp,rtp,http \
  --enable-demuxer=rtsp,rtp,sdp,mov,h264,hevc,mjpeg,mpjpeg \
  --enable-parser=h264,hevc,mjpeg \
  --enable-decoder=h264,hevc,mjpeg,wrapped_avframe \
  --enable-encoder=mjpeg --enable-muxer=mjpeg \
  --enable-filter=buffer,buffersink,format,fps,scale,color,drawbox,overlay,null \
  "${EXTRA[@]}"

make -j"$(nproc)" ffmpeg
"$TC/llvm-strip" ffmpeg

DEST="$RAIZ/android/app/src/main/jniLibs/$ABI"
mkdir -p "$DEST"
cp ffmpeg "$DEST/libffmpeg.so"
ls -la "$DEST/libffmpeg.so"
