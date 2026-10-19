#!/usr/bin/env bash
set -euo pipefail
script_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
acceptance_root=${ACCEPTANCE_ROOT:-/tmp/media88-native-acceptance-data}
repo_root=${MEDIA_SIMILARITY_ROOT:-$PWD}
: "${JFK_WAV:?Set JFK_WAV to the public whisper.cpp JFK sample WAV}"
mkdir -p "$acceptance_root/sources"
cp "$script_root/fixtures/budget.png" "$acceptance_root/sources/budget.png"
cp "$JFK_WAV" "$acceptance_root/sources/jfk.wav"
cp "${OBAMA_SOURCE_JPG:-$repo_root/sample-images/quality/sources/barack-obama-president.jpg}" "$acceptance_root/sources/obama.jpg"
cp "${OBAMA_QUERY_JPG:-$repo_root/sample-images/quality/queries/barack-obama-query.jpg}" "$acceptance_root/query-obama.jpg"
ffmpeg -y -v error -f lavfi -i 'color=c=blue:s=640x240:d=1,drawtext=text=REVENUE 2027:fontsize=52:fontcolor=white:x=(w-tw)/2:y=(h-th)/2' -frames:v 1 "$acceptance_root/revenue.png"
ffmpeg -y -v error -loop 1 -i "$acceptance_root/sources/budget.png" -loop 1 -i "$acceptance_root/revenue.png" -i "$acceptance_root/sources/jfk.wav" -filter_complex '[0:v]fps=12,trim=duration=6,setpts=PTS-STARTPTS[v0];[1:v]fps=12,trim=duration=5,setpts=PTS-STARTPTS[v1];[v0][v1]concat=n=2:v=1:a=0,format=yuv420p[v]' -map '[v]' -map 2:a -t 11 -c:v libx264 -c:a aac -movflags +faststart "$acceptance_root/sources/context.mp4"
