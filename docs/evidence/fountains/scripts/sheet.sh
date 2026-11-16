#!/bin/bash
# sheet.sh OUT.jpg WIDTH CROP(geometry or "-") IMG:LABEL ... : labelled tiles, 2 per row
out=$1; w=$2; crop=$3; shift 3
tiles=()
for a in "$@"; do
  img=${a%%:*}; lab=${a#*:}
  t=$(mktemp --suffix=.png -p ${TMPDIR:-${WORK:?the work dir}/tmp})
  if [ "$crop" = - ]; then magick "$img" -alpha off -resize ${w}x "$t"; else magick "$img" -alpha off -crop "$crop" +repage -resize ${w}x "$t"; fi
  magick "$t" -gravity northwest -fill white -undercolor '#000a' -pointsize 22 -annotate +6+6 " $lab " "$t"
  tiles+=("$t")
done
magick montage "${tiles[@]}" -tile 2x -geometry +2+2 -background black -quality 88 "$out"
rm -f "${tiles[@]}"
