#!/bin/sh
# dbx runs this in the extension's folder with DBX_INPUT (what was typed)
# and DBX_SETTING_FILE (the "file" setting) set.
file=$(printf '%s' "$DBX_SETTING_FILE" | sed "s|^~|$HOME|")

if [ -z "$DBX_INPUT" ]; then
    echo "nothing to add" >&2   # last stderr line = error shown in dbx
    exit 1
fi

echo "- [ ] $DBX_INPUT" >> "$file"
echo "added to $DBX_SETTING_FILE"   # last stdout line = status message
