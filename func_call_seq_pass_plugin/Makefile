SRC = input.c
OUT = a.out

CFLAGS = -fsanitize=address

.phony: all clean

all: $(OUT)

$(OUT): $(SRC)
	clang $(CFLAGS) -fpass-plugin='./build/lib/libfunc_seq_pp.so' -c $^ -o input.o 
	clang $(CFLAGS) input.o ./build/lib/func_stack/libfunc_stack.a -o $@

clean:
	rm -f $(OUT) *.o
