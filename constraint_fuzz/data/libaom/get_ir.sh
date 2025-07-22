#!/bin/bash


/lib/llvm-19/bin/clang \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/work/build \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/apps \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/common \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/examples \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/stats \
-I/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/third_party/libyuv/include \
-g -emit-llvm -S \
/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/av1/common/reconinter.c \
-o reconinter.ll

opt -O3 -S reconinter.ll -o reconinter_opt.ll