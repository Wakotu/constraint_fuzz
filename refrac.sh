#!/bin/bash

dest_dir="./struct_fuzz"
self_file="./refrac.sh"
git_dir='./.git'

if [[ ! -d "$dest_dir" ]]; then
  echo "$dest_dir not found, creating it"
  exit 1
fi

find . -mindepth 1 -maxdepth 1 | while read -r entry; do
  if [[ "$entry" == "$dest_dir" || "$entry" == "$self_file" || "$entry" == "$git_dir" ]]; then
    echo "$entry skipped"
    continue
  fi

  echo "Move $entry inside $dest_dir"
  mv "$entry" "$dest_dir"
done
