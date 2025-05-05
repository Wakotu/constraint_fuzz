cmake_minimum_required(VERSION 3.20)

project(func_seq_pp)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_C_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_STANDARD_REQUIRED ON)


set(CMAKE_BUILD_TYPE Debug CACHE STRING "Build type (default Debug):")

list(APPEND CMAKE_PREIFX_PATH /usr/lib/llvm-19/lib/cmake/llvm)

find_package(LLVM REQUIRED CONFIG)

include_directories(SYSTEM "${LLVM_INCLUDE_DIRS}")
link_directories("${LLVM_LIBRARY_DIRS}")
add_definitions("${LLVM_DEFINITIONS}")

# custom global constants
include_directories("${CMAKE_CURRENT_BINARY_DIR}")

set(PLUGIN_NAME "func_stack_pass" CACHE STRING "Plugin name")
set(OUTPUT_ENV_VAR "FUNC_STACK_OUT" CACHE STRING
  "func seq output file name")

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/config.h.in"
    "${CMAKE_CURRENT_BINARY_DIR}/config.h"
    @ONLY
)

add_subdirectory(src)
