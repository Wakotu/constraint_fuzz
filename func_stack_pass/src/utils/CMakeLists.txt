file(GLOB CUR_SRC "*.c" "*.cc" "*.cpp")

add_library(utils OBJECT "${CUR_SRC}")

target_include_directories(utils INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")

target_include_directories(utils
  PUBLIC "${LLVM_INCLUDE_DIRS}")
target_link_directories(utils PUBLIC "${LLVM_LIBRARY_DIRS}")