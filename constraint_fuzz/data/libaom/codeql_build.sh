#!/bin/bash

source ../common.sh

PROJECT_NAME=libaom
STALIB_NAME=libaom.a
DYNLIB_NAME=libaom.so
DIR=$(pwd)

function build_codeql(){
  unset CFLAGS
  unset CXXFLAGS
  COVERAGE_FLAGS="-g -fsanitize=fuzzer-no-link -fno-sanitize=undefined -fprofile-instr-generate -fcoverage-mapping -Wl,--no-as-needed -Wl,-ldl -Wl,-lm -Wno-unused-command-line-argument -DFUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION "
  export CFLAGS="${CFLAGS:-} $COVERAGE_FLAGS"
  export CXXFLAGS="${CXXFLAGS:-} $COVERAGE_FLAGS"

  unset CC
  unset CXX
  export CC=clang
  export CXX=clang++


  rm -rf $LIB_STORE_DIR
  
  pushd "${SRC}/${PROJECT_NAME}"
  CODEQL_BUILD_DIR='.ql_build'
  
  if [[ $CFLAGS = *sanitize=memory* ]]; then
    extra_c_flags='-DAOM_MAX_ALLOCABLE_MEMORY=536870912'
  else
    extra_c_flags='-DAOM_MAX_ALLOCABLE_MEMORY=1073741824'
  fi
  # Also, enable DO_RANGE_CHECK_CLAMP to suppress the noise of integer overflows
  # in the transform functions.
  extra_c_flags+=' -DDO_RANGE_CHECK_CLAMP=1'

  extra_cmake_flags=
  # MemorySanitizer requires that all program code is instrumented. Therefore we
  # need to replace all inline assembly code that writes to memory with pure C
  # code. Disable all assembly code for MemorySanitizer.
  if [[ $CFLAGS = *sanitize=memory* ]]; then
    extra_cmake_flags+="-DAOM_TARGET_CPU=generic"
  fi

  # Add -DSANITIZE cmake flag to avoid the undefined symbol error.
  if [[ $CFLAGS = *sanitize=address* ]]; then
    extra_cmake_flags+="-DSANITIZE=address"
  else
    extra_cmake_flags+="-DSANITIZE=fuzzer-no-link"
  fi

  cmake -S . -B $CODEQL_BUILD_DIR -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_FLAGS_RELEASE='-O3 -g' \
    -DCMAKE_CXX_FLAGS_RELEASE='-O3 -g' -DCONFIG_PIC=1 -DFORCE_HIGHBITDEPTH_DECODING=0 \
    -DENABLE_EXAMPLES=0 -DENABLE_DOCS=0 -DENABLE_TESTS=0 \
    -DCONFIG_SIZE_LIMIT=1 -DDECODE_HEIGHT_LIMIT=12288 -DDECODE_WIDTH_LIMIT=12288 \
    -DAOM_EXTRA_C_FLAGS="${extra_c_flags}" -DENABLE_TOOLS=0 \
    -DAOM_EXTRA_CXX_FLAGS="${extra_c_flags}" ${extra_cmake_flags} -DBUILD_SHARED_LIBS=1 \
    -G Ninja

  codeql database create .ql_db \
  --language=cpp \
  --command="cmake --build $CODEQL_BUILD_DIR" \
  --source-root=.
  
  mv .ql_db $CODEQL_DB_DIR

  popd
}

init && download && build_codeql

