#!/usr/bin/env bash

cd $1 || echo "error when changing dir" && exit
source venv/bin/activate

files=( $(ls))
for file in "${files[@]}"; do
  extension="${file##*.}"
  if [ "$extension" == "yml" ] || [ "$extension" == "yaml" ]; then
    python3 sigma_generate.py "$file"
    echo "$file successfully parsed"
  fi
done
