file(GLOB CUR_SRC_FILES
    "*.cc" "*.c" "*.cpp"
)

include_directories("${CMAKE_CURRENT_SOURCE_DIR}")

add_library("${CMAKE_PROJECT_NAME}" SHARED "${CUR_SRC_FILES}")
add_subdirectory(func_stack)
