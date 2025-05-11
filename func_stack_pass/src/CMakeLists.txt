file(GLOB CUR_SRC_FILES
    "*.cc" "*.c" "*.cpp"
)

include_directories("${CMAKE_CURRENT_SOURCE_DIR}")

add_library("${FSP_PLUGIN_LIB}" SHARED "${CUR_SRC_FILES}")

target_include_directories("${FSP_PLUGIN_LIB}"
  SYSTEM PRIVATE "${LLVM_INCLUDE_DIRS}")
target_link_directories("${FSP_PLUGIN_LIB}" PRIVATE "${LLVM_LIBRARY_DIRS}")
add_definitions("${LLVM_DEFINITIONS}")

install(TARGETS "${FSP_PLUGIN_LIB}" DESTINATION "${FSP_NAME}")
add_subdirectory(utils)
add_subdirectory(func_stack)

target_link_libraries("${FSP_IMPL_LIB}" PUBLIC utils)
# target_link_libraries("${FSP_PLUGIN_LIB}" PUBLIC utils)

