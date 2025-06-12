
file(GLOB CUR_SRC_FILES
    "*.cc" "*.c" "*.cpp"
)

add_library("${FSP_IMPL_LIB}" SHARED "${CUR_SRC_FILES}")

install(TARGETS "${FSP_IMPL_LIB}" DESTINATION "${FSP_NAME}")
