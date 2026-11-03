#!/usr/bin/env bash
set -euo pipefail
# Explicit developer build only; no Runtime, root CI or installer entry.
test "$#" = 2
task_root=$(cd -- "$1" && pwd)
toolchain_root=$(cd -- "$2" && pwd)
test "$task_root" != / && test "$toolchain_root" != /
script_root=$(cd -- "$(dirname -- "$0")" && pwd)
export PATH="$toolchain_root/bin:$PATH"
export LC_ALL=C
python3 "$script_root/test_verify_sources.py"
python3 -O "$script_root/test_verify_sources.py"
python3 "$script_root/verify_official_source.py" "$task_root" "$toolchain_root" > "$task_root/official-source-verification.json"
export SOURCE_DATE_EPOCH=1789951930
export PKG_CONFIG_LIBDIR="$toolchain_root/prefix/lib/pkgconfig"
mkdir -p "$task_root/build/ffmpeg" "$task_root/candidate/bin" "$task_root/candidate/licenses"
cd "$task_root/build/ffmpeg"
"$task_root/sources/ffmpeg-8.1.3/configure" \
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
cp -- ffmpeg.exe ffprobe.exe "$task_root/candidate/bin/"
cp -- "$task_root/sources/ffmpeg-8.1.3/COPYING.LGPLv2.1" "$task_root/candidate/licenses/ffmpeg-lgpl.txt"
cp -- "$toolchain_root/sources/opus-1.6.1/COPYING" "$task_root/candidate/licenses/opus.txt"
cp -- "$toolchain_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/LICENSE" "$task_root/candidate/licenses/libvpx.txt"
cp -- "$toolchain_root/sources/libvpx-6df3ec34557879fff673706f4a1d9fbd0f3a6f0e/PATENTS" "$task_root/candidate/licenses/libvpx-patents.txt"
cd "$task_root/candidate"
sha256sum bin/*.exe > programs.sha256
printf 'Official signed-source minimal static pair built.\n'
