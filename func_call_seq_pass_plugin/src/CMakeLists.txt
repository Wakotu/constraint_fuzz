file(GLOB CUR_SRC_FILES
    "*.cc" "*.c" "*.cpp"
)

include_directories("${CMAKE_CURRENT_SOURCE_DIR}")

add_library("${PLUGIN_NAME}" SHARED "${CUR_SRC_FILES}")

install(TARGETS "${PLUGIN_NAME}" DESTINATION lib)
add_subdirectory(func_stack)
