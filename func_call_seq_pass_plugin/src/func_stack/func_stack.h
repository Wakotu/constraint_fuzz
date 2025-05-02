#ifndef _FUNC_STACK_H
#define _FUNC_STACK_H

// extern StrVec v;
#ifdef __cplusplus
extern "C" {
#endif

void pop_func();

void push_func(const char *func_name);

#ifdef __cplusplus
}
#endif

#endif
