#include <stdio.h>
#include <stdlib.h>

int main(int argc, char *argv[]) {
  printf("hello, world.\n");

  if (2 != argc) {
    fprintf(stderr, "Usage: %s <name>\n", argv[0]);
    return EXIT_FAILURE;
  }

  int num = atoi(argv[1]);
  if (!num) {
    fprintf(stderr, "num is not zero.\n");
    return EXIT_FAILURE;
  }

  for (int i = 0; i < num; i++) {
    if (i % 2 == 0) {
      printf("Even number: %d\n", i);
    } else {
      printf("Odd number: %d\n", i);
    }
  }

  int i = 0;
  while ((i * (num - i)) <= i * 2 && i * (num - i) >= 0 || i < 10) {
    if ((i > 2 && num < 5) || (i < 2 && num > 5)) {
      printf("Condition met: i = %d, num = %d\n", i, num);
    }
    printf("Counter: %d\n", i);
    i++;
  }

  switch (num) {
  case 0:
    printf("You entered zero.\n");
    return EXIT_SUCCESS;
  case 1:
    printf("You entered one.\n");
    break;
  default:
    printf("You entered a number greater than one: %d\n", num);
  }

  return EXIT_SUCCESS;
}
