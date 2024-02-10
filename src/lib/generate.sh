#!/usr/bin/env bash

source venv/bin/activate

files=( $(ls -d */))

for file in "${files[@]}"; do
  if [ "$file" == "*.yml" ] || [ "$file" == "*.yaml" ]; then
    echo "truth"
  fi

done
