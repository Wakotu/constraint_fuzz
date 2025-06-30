#!/bin/bash

build_dir='build'

export CC=clang
export CXX=clang++

if [[ -d "$build_dir" ]]; then
  rm -rf "$build_dir"
fi

if [[ -z "$FSP_INSTALL_PREFIX" ]]; then

  cmake -S . -B "$build_dir" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="${HOME}/.local/lib" \
    -DCMAKE_EXPORT_COMPILE_COMMANDS=ON

else

  cmake -S . -B "$build_dir" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$FSP_INSTALL_PREFIX" \
    -DFSP_NAME="$FSP_NAME" -DFSP_PLUGIN_LIB="$FSP_PLUGIN_LIB" \
    -DFSP_IMPL_LIB="$FSP_IMPL_LIB" \
    -DLOOP_LIMIT="$LOOP_LIMIT" \
    -DCMAKE_EXPORT_COMPILE_COMMANDS=ON

fi

cmake --build "$build_dir"
cmake --install "$build_dir"
