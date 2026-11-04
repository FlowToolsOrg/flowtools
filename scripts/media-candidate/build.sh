#!/usr/bin/env bash
# Explicit developer build, never called by Runtime, installation or root CI.
set -euo pipefail
test "$#" = 1
task_root=$(cd -- "$1" && pwd)
test "$task_root" != /
script_root=$(cd -- "$(dirname -- "$0")" && pwd)
for tool in python3 gpg gcc x86_64-w64-mingw32-gcc x86_64-w64-mingw32-windres make nasm pkgconf; do
  command -v "$tool" >/dev/null
done
export LC_ALL=C
python3 "$script_root/test_verify_sources.py"
python3 -O "$script_root/test_verify_sources.py"
python3 "$script_root/verify_sources.py" "$task_root" --extract > "$task_root/source-verification.json"
export SOURCE_DATE_EPOCH=1781663837
task_prefix="$task_root/prefix"
mkdir -p "$task_root/build/opus" "$task_root/build/vpx" "$task_root/build/ffmpeg" "$task_prefix"
cd "$task_root/build/opus"
"$task_root/sources/opus-1.6.1/configure" \
  --host=x86_64-w64-mingw32 --build=x86_64-linux-gnu --prefix="$task_prefix" \
  --enable-static --disable-shared --disable-doc --disable-extra-programs \
  --disable-deep-plc --disable-dred --disable-osce \
  CC=x86_64-w64-mingw32-gcc AR=x86_64-w64-mingw32-ar RANLIB=x86_64-w64-mingw32-ranlib \
  CFLAGS="-O2 -ffile-prefix-map=$task_root=/flowtools-build" LDFLAGS=-static
make -j2
make install
cd "$task_root/build/vpx"
CROSS=x86_64-w64-mingw32- "$task_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/configure" \
  --target=x86_64-win64-gcc --prefix="$task_prefix" --enable-static --disable-shared \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests \
  --disable-vp8 --enable-vp9 --disable-vp9-decoder --disable-multithread --disable-vp9-highbitdepth
# The separate C++ RTC library is unused by FFmpeg. Build the actual C codec
# target and public headers; do not patch or rewrite upstream sources.
make -j2 libvpx.a vpx.pc
mkdir -p "$task_prefix/include/vpx" "$task_prefix/lib/pkgconfig"
cp -- libvpx.a "$task_prefix/lib/"
cp -- vpx.pc "$task_prefix/lib/pkgconfig/"
cp -- "$task_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/vpx/"*.h "$task_prefix/include/vpx/"
export PKG_CONFIG_LIBDIR="$task_prefix/lib/pkgconfig"
cd "$task_root/build/ffmpeg"
"$task_root/sources/ffmpeg-verified-n8.1.2/configure" \
  --enable-cross-compile --target-os=mingw32 --arch=x86_64 --cross-prefix=x86_64-w64-mingw32- \
  --host-cc=gcc --pkg-config=pkgconf --pkg-config-flags=--static \
  --disable-autodetect --disable-network --disable-shared --enable-static \
  --disable-doc --disable-debug --disable-ffplay --disable-avdevice --disable-everything \
  --enable-ffmpeg --enable-ffprobe --enable-avfilter --enable-swscale --enable-swresample \
  --enable-libvpx --enable-libopus \
  --enable-decoder=aac,mp3,pcm_s16le,h264,vp8,vp9,opus,flac,vorbis \
  --enable-parser=aac,mpegaudio,h264,vp8,vp9,opus,flac,vorbis \
  --enable-demuxer=mov,matroska,wav,mp3,aac,flac,ogg,avi \
  --enable-muxer=wav,webm --enable-protocol=file,pipe \
  --enable-bsf=vp9_superframe \
  --enable-encoder=pcm_s16le,libvpx_vp9,libopus \
  --enable-filter=aresample,scale,format,aformat,anull,null \
  --extra-cflags="-O2 -ffile-prefix-map=$task_root=/flowtools-build" \
  --extra-ldflags="-static -static-libgcc -Wl,--nxcompat,--dynamicbase,--high-entropy-va,--no-insert-timestamp"
make -j2 ffmpeg.exe ffprobe.exe
mkdir -p "$task_root/candidate/bin" "$task_root/candidate/licenses"
cp -- ffmpeg.exe ffprobe.exe "$task_root/candidate/bin/"
cp -- "$task_root/sources/ffmpeg-verified-n8.1.2/COPYING.LGPLv2.1" "$task_root/candidate/licenses/ffmpeg-lgpl.txt"
cp -- "$task_root/sources/opus-1.6.1/COPYING" "$task_root/candidate/licenses/opus.txt"
cp -- "$task_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/LICENSE" "$task_root/candidate/licenses/libvpx.txt"
cp -- "$task_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/PATENTS" "$task_root/candidate/licenses/libvpx-patents.txt"
cd "$task_root/candidate"
sha256sum bin/*.exe > programs.sha256
printf 'Build-only media candidate created. Production review and Runtime admission remain closed.\n'
