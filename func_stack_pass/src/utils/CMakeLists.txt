file(GLOB CUR_SRC "*.c" "*.cc" "*.cpp")

add_library(utils OBJECT "${CUR_SRC}")

target_include_directories(utils INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")
