
#include "runtime/func_stack.h"
#include "config.h"
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
/**
  Loop Context Implementation
*/

using LoopEntry =
    std::pair<std::string, std::size_t>; // loop location and count

using LoopStack = std::stack<LoopEntry>;
// static std::stack<LoopEntry> loop_stack;

std::map<Tid, LoopStack> loop_stack_map;
std::mutex loop_stack_mutex;

LoopStack &get_loop_stack() {
  Tid tid = std::this_thread::get_id();

  std::lock_guard<std::mutex> lock(loop_stack_mutex);
  auto it = loop_stack_map.find(tid);
  if (it != loop_stack_map.end()) {
    return it->second;
  }
  // create a new stack for this thread
  LoopStack &new_stack = loop_stack_map[tid];
  return new_stack;
}

/**
Function Stack Data Structure
*/

using FuncStack = std::vector<std::string>;
std::map<Tid, FuncStack> func_stack_map;
std::mutex func_stack_mutex;

FuncStack &get_func_stack() {
  Tid tid = std::this_thread::get_id();

  std::lock_guard<std::mutex> lock(func_stack_mutex);
  auto it = func_stack_map.find(tid);
  if (it != func_stack_map.end()) {
    return it->second;
  }
  // create a new stack for this thread
  FuncStack &new_stack = func_stack_map[tid];
  return new_stack;
}

// check recursion
bool check_recur(const char *func_name, const FuncStack &func_stack) {
  auto it = std::find(func_stack.rbegin(), func_stack.rend(), func_name);
  return it != func_stack.rend();
}

/**
Recur Lock Data Structure
*/

struct RecurFrame {
  std::string func_name;
  std::size_t idx;

  RecurFrame(std::string_view name, std::size_t idx)
      : func_name(name), idx(idx) {}

  bool matches(std::string_view name, std::size_t idx) const {
    return func_name == name && this->idx == idx;
  }
};

struct RecurLock {
  bool value;
  std::optional<RecurFrame> frame;

  // Initialize the lock with no frame
  RecurLock() : value(false), frame(std::nullopt) {}

  bool is_locked() const { return value; }

  bool lock(const char *func_name, std::size_t idx) {
    if (is_locked()) {
      // already locked which means in nested recursion -> do not update
      return false;
    }
    print_rec_to_file_with_guard("Recur Lock locked");
    // update
    value = true;
    frame = RecurFrame(func_name, idx);
    return true;
  }

  // try to lock the Recursion Loc, return true if successful
  bool try_lock(const char *func_name, const FuncStack &func_stack) {
    if (!check_recur(func_name, func_stack)) {
      return false;
    }
    std::size_t idx = func_stack.size();
    return lock(func_name, idx);
  }

  void release() {
    value = false;
    frame.reset(); // reset the frame
    print_rec_to_file_with_guard("Recur Lock released");
  }

  // invoked before pop
  bool try_release(const FuncStack &func_stack) {
    if (!is_locked()) {
      // not locked, cannot release
      return false;
    }
    std::string_view func_name = func_stack.back();
    std::size_t idx = func_stack.size() - 1;
    if (!frame.value().matches(func_name, idx)) {
      // not matching the current frame, cannot release
      return false;
    }

    release();
    return true;
  }
};

std::unordered_map<Tid, RecurLock> recur_lock_map;
std::mutex recur_lock_mutex;
RecurLock &get_recur_lock() {
  Tid tid = std::this_thread::get_id();

  std::lock_guard<std::mutex> lock(recur_lock_mutex);
  auto it = recur_lock_map.find(tid);
  if (it != recur_lock_map.end()) {
    return it->second;
  }
  // create a new lock for this thread
  RecurLock &new_lock = recur_lock_map[tid];
  return new_lock;
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

void print_func_rec_to_file(const char *prmp, const char *func_name) {
  std::string deman = demangle(func_name);
  std::stringstream ss;
  ss << prmp << " " << deman;
  std::string rec = ss.str();
  print_rec_to_file_with_guard(rec.c_str());
}

/**
Function Instrument Guard Implementation
*/

// invoked before function entry was pushed to the stack
bool recur_lock(const char *func_name, FuncStack &func_stack) {
  RecurLock &recur_lock = get_recur_lock();
  return recur_lock.try_lock(func_name, func_stack);
}

bool recur_release(const char *func_name, const FuncStack &func_stack) {
  RecurLock &recur_lock = get_recur_lock();
  return recur_lock.try_release(func_stack);
}

void pop_func_impl(const char *func_name, FuncStack &func_stack,
                   const char *prompt) {
  recur_release(func_name, func_stack);
  print_func_rec_to_file(prompt, func_name);
  func_stack.pop_back();
}

void pop_func(const char *func_name) {
  FuncStack &func_stack = get_func_stack();
  // check unexpected pop
  assert(!func_stack.empty() && "Function stack is empty, cannot pop function");

  if (func_name == func_stack.back()) {
    // if the function name matches the top of the stack, pop it
    pop_func_impl(func_name, func_stack, "return from");
  } else {
    while (func_name != func_stack.back()) {
      pop_func_impl(func_stack.back().c_str(), func_stack, "unwind from");
    }
    pop_func_impl(func_name, func_stack, "return from");
  }
}

void push_func(const char *func_name) {
  // output -> try_lock -> push to stack
  print_func_rec_to_file("enter", func_name);
  FuncStack &func_stack = get_func_stack();
  recur_lock(func_name, func_stack);
  func_stack.push_back(func_name);
}

/**
Output Guard Implementation
*/

bool exceed_loop_limit() {
  LoopStack &loop_stack = get_loop_stack();
  if (loop_stack.empty()) {
    return false;
  }
  auto &cur = loop_stack.top();
  auto cnt = cur.second;
  return cnt > LOOP_LIMIT;
}

bool is_recur_locked() {
  RecurLock &recur_lock = get_recur_lock();
  return recur_lock.is_locked();
}

/**
Output with No Guard Version
*/
void print_content_to_file(const char *content) {
  std::ofstream &out = get_of();
  out << content;
}

void print_rec_to_file_with_recur_guard(const char *rec) {
  if (is_recur_locked()) {
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
void print_content_to_file_with_guard(const char *content) {
  if (exceed_loop_limit()) {
    return;
  }
  if (is_recur_locked()) {
    return;
  }
  print_content_to_file(content);
}

void print_rec_to_file_with_guard(const char *rec) {
  std::stringstream ss;
  ss << rec << "\n";
  print_content_to_file_with_guard(ss.str().c_str());
}

void push_new_entry_to_loop_stack(const char *loop_loc, LoopStack &loop_stack) {
  LoopEntry lent{loop_loc, 1};
  loop_stack.push(lent);

  std::stringstream ss;
  ss << "Loop Hit: " << loop_loc << " at count " << 1;
  print_rec_to_file_with_recur_guard(ss.str().c_str());
}

/**
Loop Instrument Guard Implementation
*/
void loop_entry(const char *loop_loc) {
  LoopStack &loop_stack = get_loop_stack();
  if (loop_stack.empty()) {
    // hit at new loop without nesting
    // if the stack is empty, push a new entry
    push_new_entry_to_loop_stack(loop_loc, loop_stack);
    return;
  }

  auto &cur = loop_stack.top();
  if (cur.first == loop_loc) {
    // hit at current loop
    // increment the count
    cur.second++;
    auto cnt = cur.second;
    if (cnt <= LOOP_LIMIT) {
      // Repeated hit for current loop entry
      std::stringstream ss;
      ss << "Loop Hit: " << loop_loc << " at count " << cnt;
      //
      print_rec_to_file_with_recur_guard(ss.str().c_str());
    } else if (cnt - LOOP_LIMIT == 1) {
      // Loop Entry Exceed
      std::stringstream ss;
      ss << "Loop Limit Exceed: " << loop_loc << " at count " << cnt;
      print_rec_to_file_with_recur_guard(ss.str().c_str());
    }
  } else {
    // hit at nested loop
    // push a new entry

    auto parent_count = cur.second;
    if (parent_count > LOOP_LIMIT) {
      // if parent loop is already exceeding limit, do not push new entry
      return;
    }

    push_new_entry_to_loop_stack(loop_loc, loop_stack);
  }
}

void loop_end(const char *header_loc, const char *out_loc) {
  LoopStack &loop_stack = get_loop_stack();
  if (loop_stack.empty()) {
    // if the stack is empty, this is an error
    std::stringstream ss;
    ss << "Loop end without loop start: " << header_loc << " " << out_loc;
    print_rec_to_file_with_recur_guard(ss.str().c_str());
    return;
  }
  // consider reasonable to be hit without passing loop entry
  auto &cur = loop_stack.top();
  if (cur.first == header_loc) {
    loop_stack.pop();
    // Loop End Out
    std::stringstream ss;
    ss << "Out of Loop: " << header_loc << " " << out_loc << " at count "
       << cur.second;
    print_rec_to_file_with_recur_guard(ss.str().c_str());
  } else {
    // this is an error, loop end without loop start
    // Loop End Without Start

    // check if parent loop is exceeding limit
    auto parent_count = cur.second;
    if (parent_count > LOOP_LIMIT) {
      return;
    }

    std::stringstream ss;
    ss << "Loop end without loop start: " << header_loc << " " << out_loc;
    print_rec_to_file_with_recur_guard(ss.str().c_str());
  }
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
