#!/usr/bin/env bash

cd $1
source venv/bin/activate

files=( $(ls))
for file in "${files[@]}"; do
  extension="${file##*.}"
  if [ "$extension" == "yml" ] || [ "$extension" == "yaml" ]; then
    python3 sigma_generate.py "$file"
    echo "$file successfully parsed"
  fi
done
