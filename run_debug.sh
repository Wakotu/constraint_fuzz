#!/bin/bash

export RUST_LOG=debug
export RUST_BACKTRACE=1
export COLORBT_SHOW_HIDDEN=1
export RUST_BACKTRACE=full

cargo run --bin harness -- libaom expe --cov-format json examples/libaom/example_fuzzer.cc
