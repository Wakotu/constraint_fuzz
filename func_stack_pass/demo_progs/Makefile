SRC_FILES = $(wildcard *_test.c)
EXES = $(SRC_FILES:.c=.out)

.PHONY: all clean

all: $(EXES)

$(EXES): %.out: %.c
	$(CC) $(CFLAGS) -o $@ $<


clean:
	rm -f $(EXES)