#!/bin/bash

build_dir='build'

# if [[ -d "$build_dir" ]]; then
#   rm -rf "$build_dir"
# fi

cmake -S . -B "$build_dir" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_EXPORT_COMPILE_COMMANDS=ON

cmake --build "$build_dir"
