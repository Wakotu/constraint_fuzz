SRC = input.c
OUT = a.out

CC = clang 
CXX = clang++
CFLAGS = -fsanitize=address

.phony: all clean

all: $(OUT)

$(OUT): $(SRC)
	$(CC) $(CFLAGS)  -c $^ -o input.o -fpass-plugin=$(HOME)/.local/lib/func_seq_pass/libfunc_seq_pass.so
	$(CC) $(CFLAGS) input.o -L $(HOME)/.local/lib/func_seq_pass -lfunc_stack -lstdc++ -o $@ 

clean:
	rm -f $(OUT) *.o
