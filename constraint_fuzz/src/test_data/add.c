#include <stdio.h>

#define TEST(a, b) (a + b < 20)
#define SHOW(a, b)                                                             \
  if (a + b < 20)                                                              \
    puts("Add");                                                               \
  else                                                                         \
    puts("Subtract");

void foo();

int add(int a, int b) {
  foo();
  SHOW(1, 2);
  SHOW(a, b);
  if (TEST(a, b)) {
    return a + b;
  } else {
    return a - b;
  }
}

void foo() {
  SHOW(1, 2);
  printf("Foo instrument\n");
}
