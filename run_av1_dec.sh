#!/bin/bash

export RUST_LOG=debug
export RUST_BACKTRACE=1

cargo run --bin harness -- libaom expe output/build/libaom/src/libaom/examples/av1_dec_fuzzer.cc
