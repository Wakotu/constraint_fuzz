#!/bin/bash

export FSP_INSTALL_PREFIX="$HOME"/.local/lib
export FSP_NAME="func_stack_pass"
export FSP_PLUGIN_LIB='func_stack_plugin'
export FSP_IMPL_LIB='func_stack'
export LOOP_LIMIT=3

if [[ ! -d "$FSP_INSTALL_PREFIX" ]]; then
  mkdir -p "$FSP_INSTALL_PREFIX"
fi

echo "build and install function call stack plugin"
plugin_dir='./func_stack_pass'
pushd "$plugin_dir" || exit 1

rm -rf build
./build.sh

popd || exit 1

echo "build and install cc wrapper"
cargo build -r

cp ./target/release/cc_wrapper ~/.local/bin/cc_wrapper
if [[ -e ~/.local/bin/cxx_wrapper ]]; then
  rm ~/.local/bin/cxx_wrapper
fi
ln -s cc_wrapper ~/.local/bin/cxx_wrapper
