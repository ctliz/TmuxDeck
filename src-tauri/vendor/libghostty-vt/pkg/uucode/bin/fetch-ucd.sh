#!/bin/bash

version="17.0.0"

base_url="https://www.unicode.org/Public/zipped/latest"
emoji_url="https://www.unicode.org/Public/emoji/latest"

mv ucd/.gitignore ucd-gitignore
rm -rf ucd
mkdir -p ucd/Unihan
mv ucd-gitignore ucd/.gitignore

cd ucd
curl -o ucd.zip "${base_url}/UCD.zip"
unzip ucd.zip
rm ucd.zip

cd emoji
curl -o emoji-sequences.txt "${emoji_url}/emoji-sequences.txt"
curl -o emoji-test.txt "${emoji_url}/emoji-test.txt"
curl -o emoji-zwj-sequences.txt "${emoji_url}/emoji-zwj-sequences.txt"
cd ..

cd Unihan
curl -o unihan.zip "${base_url}/Unihan.zip"
unzip unihan.zip
rm unihan.zip
cd ..

cd ..

echo
echo "########################################################################"
echo
echo "Done fetching UCD files"
echo
echo "Explicitly add any new files to start parsing to the list of .gitignore"
echo "exceptions. Add a '#' to comment them out, appending '(used)' at the end."
echo
echo "Next, flip the 'is_updating_ucd' flag in 'src/config.zig' to true, and"
echo "'zig build test' once, updating the 'default' config if it needs"
echo "changing, before flipping 'is_updating_ucd' back to false."
echo
