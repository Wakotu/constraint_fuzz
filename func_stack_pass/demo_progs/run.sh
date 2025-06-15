#!/bin/bash

export CC=cc_wrapper
export CXX=cxx_wrapper
export LD_LIBRARY_PATH=~/.local/lib/func_stack_pass/:$LD_LIBRARY_PATH
LOG_DIR=logs

if [[ ! -d $LOG_DIR ]]; then
    mkdir $LOG_DIR
fi

make clean
make -j "$(nproc)"

# search all files ending with .out in the current directory
for file in *.out; do
    if [[ -f $file ]]; then
        echo "Running $file"
        out_dir="$LOG_DIR/${file%.out}.log" 
        if [[ -d "$out_dir" ]]; then 
            rm -rf "$out_dir"
        fi
        
        FUNC_STACK_OUT="$out_dir" ./$file 3
    fi
done



