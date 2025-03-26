! cargo build
file ./target/debug/harness
set environment RUST_LOG=debug
set environment RUST_BACKTRACE=1
tty /dev/pts/5
start libaom expe --cov-format json examples/libaom/example_fuzzer.cc
layout src
b prompt_fuzz::execution::Executor::expe_cov_collect

