#ifndef _FUNC_STACK_H
#define _FUNC_STACK_H

#ifdef __cplusplus
extern "C" {
#endif

void pop_func(const char *func_name);

void push_func(const char *func_name);
void print_content_to_file_with_guard(const char *rec);
void print_rec_to_file_with_guard(const char *rec);

void loop_entry(const char *loop_loc);

void loop_end(const char *loop_loc, const char *out_loc);

void thread_rec(const char *loc, void *tid_ptr);

#ifdef __cplusplus
}
#endif

#endif
