#!/bin/bash
## Note, this requires inkscape and imagemagick to be installed to create the various Windows icon sizes from SVG..

# Create variously sized icons for Windows..
mkdir tmp/
FILE_PATHS=""
for size in 16 32 48 128 256; do
  FILE_NAME="tmp/$size.png"
  inkscape goxlr-utility.svg --export-filename=$FILE_NAME -w $size -h $size
  FILE_PATHS="${FILE_PATHS} ${FILE_NAME}"
done
convert $FILE_PATHS goxlr-utility.ico
rm -r tmp/


# On Linux, we need to add a 20px margin..
# left -25, top -25, right: 281.0, bottom: 281.0 width:306, height: 306

# For Linux, we need to add a margin to the icon so it looks clean in things like system trays, we'll use around 10%
# We need:
# 128x128 PNG in /usr/share/pixmaps
# 48x48 PNG in /usr/share/icons/hicolor/48x48/apps/
# SVG to /usr/share/icons/hicolor/scalable/apps/
# And a 128x128 png for embedding
inkscape goxlr-utility.svg --export-filename=goxlr-utility-large.png -w 128 -h 128 --export-area=-25:-25:281:281
inkscape goxlr-utility.svg --export-filename=goxlr-utility.png -w 48 -h 48 --export-area=-25:-25:281:281
