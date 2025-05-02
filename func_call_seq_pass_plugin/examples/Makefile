SRC = input.c
OUT = a.out

CC = clang 
CXX = clang++
CFLAGS = -fsanitize=address

.phony: all clean

all: $(OUT)

$(OUT): $(SRC)
	$(CC) $(CFLAGS) -fpass-plugin='../lib/libfunc_seq_pass.so' -c $^ -o input.o 
	$(CXX) $(CFLAGS) input.o -L ../lib -lfunc_stack -o $@

clean:
	rm -f $(OUT) *.o
