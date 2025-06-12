#include "func_stack.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define INIT_SIZE 8

typedef struct {
  char *data;
  int len;
} FixStr;

typedef struct {
  FixStr *data;
  int len;
  int cap;
} StrVec;

static StrVec v;
static int inited = 0;

/* FixStr methods */
FixStr new_str(const char *lit) {
  int len = strlen(lit);
  char *data = (char *)malloc(len + 1);
  strncpy(data, lit, len);
  data[len] = 0;

  FixStr s;
  s.data = data;
  s.len = len;
  return s;
}

void str_free(FixStr *s) { free(s->data); }

void str_move(FixStr src, FixStr *dst) {
  dst->len = src.len;
  dst->data = src.data;
}

/* StrVec methods */
void init_vec(StrVec *v) {
  FixStr *data = (FixStr *)malloc(INIT_SIZE * (sizeof(FixStr)));
  v->data = data;
  v->cap = INIT_SIZE;
  v->len = 0;
}

FixStr *vec_str(StrVec *v, int idx) {
  FixStr *s = v->data + idx;
  return s;
}

void free_vec_str(StrVec *v, int idx) {
  FixStr *s = vec_str(v, idx);
  str_free(s);
}

FixStr *vec_cur(StrVec *v) {
  FixStr *cur = v->data + v->len - 1;
  return cur;
}

void free_vec_cur(StrVec *v) { free_vec_str(v, v->len - 1); }

void vec_push_back(StrVec *v, const char *lit) {
  // oncstructs new FixStr
  FixStr s = new_str(lit);
  v->len = v->len + 1;
  if (v->len > v->cap) {
    v->cap = v->cap * 2;
    v->data = realloc(v->data, v->cap);
  }
  assert(v->cap >= v->len);

  FixStr *cur = vec_cur(v);
  str_move(s, cur);
}

int vec_empty(StrVec *v) { return v->len == 0; }

int vec_pop_back(StrVec *v) {
  if (vec_empty(v))
    return 0;

  /* free string */
  free_vec_cur(v);
  v->len = v->len - 1;

  return 1;
}

void free_vec(StrVec *v) {
  for (int i = 0; i < v->len; i++) {
    free_vec_str(v, i);
  }
  free(v->data);
}

void vec_output_rev(StrVec *v) {
  for (int i = v->len - 1; i >= 0; i--) {
    FixStr *s = vec_str(v, i);
    printf("%s\n", s->data);
  }
}

/* exported functions */
void pop_func() {
  assert(inited && "Function name stack not inited before calling pop_func()");
  assert(!vec_empty(&v) && "Function name stack is empty");

  FixStr *s = vec_cur(&v);
  printf("return from %s\n", s->data);
  vec_pop_back(&v);
  vec_output_rev(&v);
  printf("\n");
}

void push_func(const char *func_name) {
  if (!inited) {
    init_vec(&v);
    inited = 1;
  }

  printf("enter function: %s\n", func_name);
  vec_push_back(&v, func_name);
  vec_output_rev(&v);
  printf("\n");
}
