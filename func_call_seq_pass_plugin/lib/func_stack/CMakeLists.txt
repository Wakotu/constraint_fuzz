
file(GLOB CUR_SRC_FILES
    "*.cc" "*.c" "*.cpp"
)

add_library(func_stack STATIC "${CUR_SRC_FILES}")
