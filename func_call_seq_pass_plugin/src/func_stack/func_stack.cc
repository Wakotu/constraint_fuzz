#include "func_stack.h"
#include "config.h"
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

static std::vector<std::string> func_stack;

static FILE *fp = nullptr;

void setup_fp() {
  if (fp) {
    return;
  }

  const char *fpath = std::getenv(OUTPUT_ENV_VAR);
  fp = fopen(fpath, "w");
}

// #define LOG_ERR(x...) fprintf(stderr, x);

#define LOG_FILE(fmt...)                                                       \
  do {                                                                         \
    setup_fp();                                                                \
    fprintf(fp, fmt);                                                          \
  } while (0)

void print_func_stack_rev() {
  for (auto it = func_stack.rbegin(); it != func_stack.rend(); it++) {
    // fprintf(stderr, "%s\n", it->c_str());
    LOG_FILE("%s\n", it->c_str());
  }
}

void pop_func() {
  assert(!func_stack.empty() && "Function stack is empty before pop_func()");

  std::string func_name = func_stack.back();
  LOG_FILE("return from %s\n", func_name.c_str());
  func_stack.pop_back();
  print_func_stack_rev();
  LOG_FILE(" \n");

  if (func_stack.empty()) {
    fflush(fp);
    fclose(fp);
    fp = nullptr;
  }
}

void push_func(const char *func_name) {
  std::string func(func_name);
  func_stack.push_back(func);
  LOG_FILE("enter %s\n", func_name);
  print_func_stack_rev();
  LOG_FILE("\n");
}
