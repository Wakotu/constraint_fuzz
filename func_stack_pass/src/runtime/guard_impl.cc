
#include "runtime/guard_impl.h"
#include "config.h"
#include "runtime_context.h"
#include "utils.h"
#include <algorithm>
#include <cassert>
#include <csignal>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cxxabi.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <mutex>
#include <optional>
#include <sstream>
#include <stack>
#include <string>
#include <string_view>
#include <thread>
#include <unordered_map>
#include <vector>

// #define LOG_ERR(x...) fprintf(stderr, x);

namespace fs = std::filesystem;

void print_func_rec_to_file(const char *prmp, const char *func_name);
using Tid = std::thread::id;

static std::unordered_map<Tid, std::ofstream> of_map;
std::mutex of_map_mutex;

void sig_handler(int sig) {
  if (sig == SIGINT) {
    for (auto &it : of_map) {
      auto &out = it.second;
      out.close();
    }
    std::exit(sig);
  }
}

void __attribute__((constructor)) setup_sig_handler() {
  signal(SIGINT, sig_handler);
}

std::ofstream &create_of(const Tid &tid) {
  // std::cerr << "creating fp\n";
  static bool first = true;

  const char *out_str = std::getenv(OUTPUT_ENV_VAR);
  if (!out_str) {
    out_str = "func_stack_logs";
  }
  fs::path out_dir(out_str);

  if (!fs::is_directory(out_dir)) {
    if (fs::is_regular_file(out_dir)) {
      fs::remove(out_dir);
    }
    try {

      bool flag = fs::create_directories(out_dir);
      if (!flag) {
        std::cerr << "Failed to create directory: " << out_dir << "\n";
        exit(1);
      }
    } catch (const std::filesystem::filesystem_error &e) {
      std::cerr << "Error: " << e.what() << "\n";
      exit(1);
    }
  }

  std::stringstream ss;

  // actually output dir
  ss << tid;
  if (first) {
    ss << "_main";
  }
  std::string fname_str = ss.str();

  // std::cerr << "fname: " << fname_str << "\n";

  fs::path fname(fname_str);
  fs::path fpath = out_dir / fname;

  std::ofstream out(fpath);
  if (!out.is_open()) {
    std::cerr << "Failed to open file: " << fpath << "\n";
    std::exit(1);
  }

  of_map[tid] = std::move(out);

  first = false;

  return of_map[tid];
}

std::ofstream &get_of() {

  Tid tid = std::this_thread::get_id();

  std::lock_guard<std::mutex> lock(of_map_mutex);
  auto it = of_map.find(tid);
  if (it != of_map.end()) {
    return it->second;
  }
  return create_of(tid);
}

// #define LOG_FILE(fmt...)                                                       \
//   do {                                                                         \
//     FILE *fp = get_fp();                                                       \
//     fprintf(fp, fmt);                                                          \
//   } while (0)

// void print_func_stack_rev() {
//   for (auto it = func_stack.rbegin(); it != func_stack.rend(); it++) {
//     // fprintf(stderr, "%s\n", it->c_str());
//     LOG_FILE("%s\n", it->c_str());
//   }
// }

// Runtime Context definition
RuntimeCtxMap ctx_map;

void print_func_rec_to_file(const char *prmp, const char *func_name) {
  std::string deman = demangle(func_name);
  std::stringstream ss;
  ss << prmp << " " << deman;
  std::string rec = ss.str();
  print_rec_to_file_with_lockcheck(rec.c_str());
}

/**
Function Instrument Guard Implementation
*/

// void pop_func_impl(const char *func_name, FuncStack &func_stack,

//                    const char *prompt, bool unwind = false) {
//   recur_release(func_name, func_stack);
//   // enable unwind output
//   print_func_rec_to_file(prompt, func_name);
//   func_stack.pop_back();
// }

void pop_func(const char *func_name) {
  ctx_map.pop_func(func_name);
  print_func_rec_to_file("return from", func_name);
}

void push_func(const char *func_name) {
  // output -> try_lock -> push to stack
  print_func_rec_to_file("enter", func_name);
  ctx_map.push_func(func_name);
}

/**
Output Guard Implementation
*/

/**
Output with No Guard Version
*/
void print_content_to_file(const char *content) {
  std::ofstream &out = get_of();
  out << content;
}

void print_rec_to_file_with_recur_check(const char *rec) {
  if (ctx_map.is_recur_locked()) {
    // if recursion is locked, do not print
    return;
  }
  std::stringstream ss;
  ss << rec << "\n";
  print_content_to_file(ss.str().c_str());
}

/**
Output with Guard Version
*/
void print_content_to_file_with_lockcheck(const char *content) {
  if (ctx_map.is_locked()) {
    // if guard is locked, do not print
    return;
  }
  print_content_to_file(content);
}

void print_rec_to_file_with_lockcheck(const char *rec) {
  std::stringstream ss;
  ss << rec << "\n";
  print_content_to_file_with_lockcheck(ss.str().c_str());
}

/**
Loop Instrument Guard Implementation
*/
void loop_entry(const char *loop_loc) {
  std::size_t iter_cnt = ctx_map.loop_entry(loop_loc);
  if (iter_cnt == LOOP_LIMIT + 1) {

    std::stringstream ss;
    ss << "Loop Limit Exceed: " << loop_loc << " at count " << iter_cnt;
    // set lock moment without loop lock check
    print_rec_to_file_with_recur_check(ss.str().c_str());
  } else {
    std::stringstream ss;
    ss << "Loop Hit: " << loop_loc << " at count " << iter_cnt;
    // normal status with loop lock check
    print_rec_to_file_with_lockcheck(ss.str().c_str());
  }
}

void loop_out(const char *header_loc, const char *out_loc) {
  LoopPopResult res = ctx_map.loop_out(header_loc);
  std::size_t iter_cnt = res.get_iter_count();
  if (res.is_poped()) {
    std::stringstream ss;
    ss << "Out of Loop: " << header_loc << " " << out_loc << " at count "
       << iter_cnt;
    print_rec_to_file_with_lockcheck(ss.str().c_str());
  } else {
    std::stringstream ss;
    ss << "Loop end without loop start: " << header_loc << " " << out_loc;
    print_rec_to_file_with_lockcheck(ss.str().c_str());
  }
}

// thread creation instrumentation
void thread_rec(const char *loc, void *tid_ptr) {
  pthread_t tid = *(pthread_t *)tid_ptr;
  std::stringstream ss;
  ss << "Thread Creation: " << loc << " " << tid;
  // consider lock check
  print_rec_to_file_with_lockcheck(ss.str().c_str());
}

// static std::unordered_map<std::size_t, unsigned int> loop_counter;

// unsigned int get_loop_count(const SrcLoc &loc) {
//   size_t hash = std::hash<SrcLoc>()(loc);
//   auto it = loop_counter.find(hash);
//   if (it != loop_counter.end()) {
//     return ++it->second;
//   } else {
//     loop_counter[hash] = 1;
//     return 1;
//   }
// }

// void record_loop(const char *src_path, unsigned int line, unsigned int col) {
//   SrcLoc loc(src_path, line, col);
//   auto count = get_loop_count(loc);

//   std::stringstream ss;
//   ss << "Loop: " << loc << " " << count;
//   print_rec_to_file(ss.str().c_str());
// }

/**
  Unconditional Branch Value record output
*/

void ubv_rec(const char *loc, bool val) {
  std::stringstream ss;
  ss << "Unconditional Branch Value: " << loc << " " << val;
  print_rec_to_file_with_lockcheck(ss.str().c_str());
}

/**
  Selection instruction record output
*/

void select_rec(const char *loc, bool val) {
  std::stringstream ss;
  ss << "Select Guard: " << loc << " " << val;
  print_rec_to_file_with_lockcheck(ss.str().c_str());
}

/**
  Setjmp and Longjmp handle
*/

void stack_rollback(const char *func_name, std::size_t stk_size) {
  RuntimeContext &ctx = ctx_map.get_ctx();
  while (ctx.get_func_stack_size() > stk_size) {
    std::string cur_func = ctx.func_unwind();
    print_func_rec_to_file("Longjmp Unwind: ", cur_func.c_str());
  }
  // func clear loop stack
  ctx.func_clear_loops();
}

void setjmp_guard(int ret_val, const char *func_name, const char *loc) {
  static std::size_t stk_size = 0;

  if (ret_val == 0) {
    // pre setjmp
    stk_size = ctx_map.get_func_stack_size();
    assert(stk_size > 0 && "Current stack size should not be 0");
  } else {
    assert(stk_size > 0 && "Setjmp stack size should not be 0");
    // post setjmp: func stack rollback handle
    stack_rollback(func_name, stk_size);
  }

  // action record output
  std::stringstream ss;
  if (ret_val == 0) {
    ss << "Pre-long Setjmp: ";
  } else {
    ss << "Post-long Setjmp: ";
  }
  ss << func_name << " " << stk_size << " " << loc;
  print_rec_to_file_with_lockcheck(ss.str().c_str());
}