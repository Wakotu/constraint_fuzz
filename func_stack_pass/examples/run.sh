#!/bin/bash

export LD_LIBRARY_PATH=${HOME}/.local/lib/func_seq_pass:$LD_LIBRARY_PATH

make

for i in {1..5}; do
  FUNC_STACK_OUT="func_stack.out.${i}" ./a.out 2 3 &
done
