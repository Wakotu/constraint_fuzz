#include <stdio.h>
#include <stdlib.h>

int add(int a, int b) {
  if (a + b < 20) {
    return a + b;
  } else {
    return a - b;
  }
}

int main(int argc, char *argv[]) {

  if (3 != argc) {
    fprintf(stderr, "Usage: %s <a> <b>\n", argv[0]);
    exit(1);
  }

  int a = atoi(argv[1]);
  int b = atoi(argv[2]);
  add(a, b);
  return 0;
}
