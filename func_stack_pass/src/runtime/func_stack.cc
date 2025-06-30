
#include "runtime/func_stack.h"
#include "config.h"
#include "utils.h"
#include <cassert>
#include <csignal>
#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cxxabi.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stack>
#include <string>
#include <thread>
#include <unordered_map>

// #define LOG_ERR(x...) fprintf(stderr, x);

namespace fs = std::filesystem;

using Tid = std::thread::id;

static std::unordered_map<Tid, std::ofstream> of_map;

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

using LoopEntry =
    std::pair<std::string, std::size_t>; // loop location and count

static std::stack<LoopEntry> loop_stack;

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

void print_func_rec_to_file(const char *prmp, const char *func_name) {
  std::string deman = demangle(func_name);
  std::stringstream ss;
  ss << prmp << " " << deman;
  std::string rec = ss.str();
  print_rec_to_file_with_loop_guard(rec.c_str());
}

void pop_func(const char *func_name) {
  print_func_rec_to_file("return from", func_name);
}

void push_func(const char *func_name) {
  print_func_rec_to_file("enter", func_name);
}

bool exceed_loop_limit() {
  if (loop_stack.empty()) {
    return false;
  }
  auto &cur = loop_stack.top();
  auto cnt = cur.second;
  return cnt > LOOP_LIMIT;
}

void print_rec_to_file(const char *rec) {
  std::ofstream &out = get_of();
  out << rec << "\n";
}

void print_rec_to_file_with_loop_guard(const char *rec) {
  if (exceed_loop_limit()) {
    return;
  }
  print_rec_to_file(rec);
}

/**
  Loop Context Implementation
*/

void loop_hit(const char *loop_loc) {
  if (loop_stack.empty()) {
    // if the stack is empty, push a new entry
    LoopEntry lent{loop_loc, 1};
    loop_stack.push(lent);
    std::stringstream ss;
    return;
  }

  auto &cur = loop_stack.top();
  if (cur.first == loop_loc) {
    // increment the count
    cur.second++;
    auto cnt = cur.second;
    if (cnt - LOOP_LIMIT == 1) {
      std::stringstream ss;
      ss << "Loop Limit Exceed: " << loop_loc << " at count: " << cnt;
      print_rec_to_file(ss.str().c_str());
    }
  } else {
    // push a new entry
    LoopEntry lent{loop_loc, 1};
    loop_stack.push(lent);
  }
}

void loop_end(const char *loop_loc) {
  assert(!loop_stack.empty() && "Loop stack is empty");
  auto &cur = loop_stack.top();
  if (cur.first == loop_loc) {
    loop_stack.pop();
    std::stringstream ss;
    ss << "Out of Loop: " << loop_loc << " at count: " << cur.second;
    print_rec_to_file(ss.str().c_str());
  } else {
    std::cerr << "Error: Loop end called for a different loop location: "
              << loop_loc << " vs " << cur.first << "\n";
    std::exit(1);
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
