#!/bin/bash

export RUST_LOG=debug
export RUST_BACKTRACE=1

# cargo run -r --bin harness -- libaom expe --cov-format json examples/libaom/example_fuzzer.cc
cargo run -r --bin harness -- libaom expe examples/libaom/example_fuzzer.cc
