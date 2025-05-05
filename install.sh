#!/bin/bash

echo "build and install function call stack plugin"
plugin_dir='./func_call_seq_pass_plugin'
pushd "$plugin_dir" || exit 1

./build.sh

popd || exit 1

echo "build and install cc wrapper"
cargo build -r

cp ./target/release/cc_wrapper ~/.local/bin/cc_wrapper
if [[ -e ~/.local/bin/cxx_wrapper ]]; then
  rm ~/.local/bin/cxx_wrapper
fi
ln -s cc_wrapper ~/.local/bin/cxx_wrapper
