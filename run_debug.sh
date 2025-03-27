#!/bin/bash

export RUST_LOG=debug
export RUST_BACKTRACE=1

cargo run --bin harness -- libaom expe --cov-format json examples/libaom/example_fuzzer.cc
