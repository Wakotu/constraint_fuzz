#ifndef _FUNC_STACK_H
#define _FUNC_STACK_H

#ifdef __cplusplus
extern "C" {
#endif

void pop_func(const char *func_name);

void push_func(const char *func_name);
void print_rec_to_file_with_loop_guard(const char *rec);

void loop_hit(const char *loop_loc);

void loop_end(const char *loop_loc);

#ifdef __cplusplus
}
#endif

#endif
