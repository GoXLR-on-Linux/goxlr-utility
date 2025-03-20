#!/bin/bash
## Note, this requires inkscape and imagemagick to be installed to create the various Windows icon sizes from SVG..
## This also requires icnsutil (https://github.com/relikd/icnsutil) for MacOS

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

mkdir macos/
for size in 16 32 64 128 256 512 1024; do
  EXTRA=""
  FILE_NAME="macos/$size.png"

  # On a 1024x1024 SVG we need a margin of 148px on each side..
  EXTRA="--export-area=-148:-148:1172:1172"

  # Export to file..
  inkscape goxlr-utility.svg --export-filename=$FILE_NAME -w $size -h $size $EXTRA
done

# Ok, now we need to do weird shit with the filenames..
mkdir macos_final/
cp macos/16.png macos_final/icon_16x16.png
cp macos/32.png macos_final/icon_16x16@2x.png
cp macos/32.png macos_final/icon_32x32.png
cp macos/64.png macos_final/icon_32x32@2x.png
cp macos/128.png macos_final/icon_128x128.png
cp macos/256.png macos_final/icon_128x128@2x.png
cp macos/256.png macos_final/icon_256x256.png
cp macos/512.png macos_final/icon_256x256@2x.png
cp macos/512.png macos_final/icon_512x512.png
cp macos/1024.png macos_final/icon_512x512@2x.png

# Bundle all this shit together..
icnsutil c icon.icns macos_final/*.png --toc
rm -r macos/ macos_final/

# On Linux, we need to add a 100px margin..
# left -100, top -100, right: 1124, bottom: 1124

# For Linux, we need to add a margin to the icon so it looks clean in things like system trays, we'll use around 10%
# We need:
# 128x128 PNG in /usr/share/pixmaps
# 48x48 PNG in /usr/share/icons/hicolor/48x48/apps/
# SVG to /usr/share/icons/hicolor/scalable/apps/
# And a 128x128 png for embedding
inkscape goxlr-utility.svg --export-filename=goxlr-utility-large.png -w 128 -h 128 --export-area=-100:-100:1124:1124
inkscape goxlr-utility.svg --export-filename=goxlr-utility.png -w 48 -h 48 --export-area=-100:-100:1124:1124
