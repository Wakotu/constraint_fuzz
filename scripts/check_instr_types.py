#!/usr/bin/python3

import os

def get_type(line: str) -> str | None:
    prefix = '[Br Condition] Instruction:'
    if not line.startswith(prefix):
        return None
    instr= line[len(prefix):].strip()
    return instr.split()[2]
        

fpath = '/struct_fuzz/constraint_fuzz/data/libaom/br_conds'
if not os.path.exists(fpath):
    print(f"Path {fpath} does not exist.")
    exit(1)
    
instr_types = set()

with open(fpath, 'r') as f:
    lines = f.readlines()
    for line in lines:
        instr_type = get_type(line )
        if instr_type is None:
            continue
        if instr_type not in instr_types:
            instr_types.add(instr_type)
        
# Print the unique instruction types
print("Unique instruction types found:")
print("=================================")
for instr_type in sorted(instr_types):
    print(instr_type)