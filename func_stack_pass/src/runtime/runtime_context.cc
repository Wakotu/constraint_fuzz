#include "runtime_context.h"
#include "config.h"
#include <cassert>
#include <cstddef>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <utility>

namespace fs = std::filesystem;

// definition
bool RuntimeContext::is_main = true;

void FuncStack::push(const char *func_name) {
  FuncEntry ent(func_name);
  stk.push_back(ent);
}

const FuncEntry &FuncStack::const_top_func() const {
  assert(!stk.empty() &&
         "Top func: Runtime func stack should not be empty at this procedure");
  return stk.back();
}

FuncEntry &FuncStack::top_func() {
  assert(!stk.empty() &&
         "Top func: Runtime func stack should not be empty at this procedure");
  return stk.back();
}

void FuncStack::pop() {
  assert(!stk.empty() &&
         "Pop func: Runtime func stack should not be empty at this procedure");
  stk.pop_back();
}

bool FuncStack::check_recur() const {
  std::string_view cur_func = const_top_func().get_func_name();
  int size = stk.size();
  for (int i = size - 1; i >= 0; i--) {
    std::string_view func_name = stk[i].get_func_name();
    if (func_name == cur_func) {
      return true;
    }
  }
  return false;
}

std::string_view FuncStack::top_func_name() const {
  return const_top_func().get_func_name();
}

void RecurLock::lock(std::string_view func_name, std::size_t stk_size) {
  RecurFrame frame(func_name, stk_size);
  this->value = true;
  this->frame = frame;

  // record output
}

void RecurLock::release() {
  this->value = false;
  this->frame = std::nullopt;

  // record output
}

bool RecurLock::matches(std::string_view func_name,
                        std::size_t stk_size) const {
  if (!is_locked())
    return false;
  return frame->matches(func_name, stk_size);
}

void RuntimeContext::set_recur_lock() {
  recur_lock.lock(func_stk.top_func_name(), func_stk.size());
}

void RuntimeContext::try_recur_lock() {
  bool flag = func_stk.check_recur();
  if (!flag)
    return;
}

void RuntimeContext::push_func(const char *func_name) {
  // stack update
  func_stk.push(func_name);
  // lock update
  try_recur_lock();
}

void RuntimeContext::try_recur_release() {
  if (!recur_lock.is_locked())
    return;

  std::string_view top_func = func_stk.top_func_name();
  std::size_t stk_size = func_stk.size();
  if (!recur_lock.matches(top_func, stk_size)) {
    return;
  }
  recur_lock.release();
}

bool FuncStack::top_loop_empty() const { return const_top_func().loop_emty(); }

void RuntimeContext::loop_lock_on() {
  loop_lock = true;
  // record output
}

void RuntimeContext::loop_lock_off() {
  loop_lock = false;
  // record output
}

void RuntimeContext::pop_func_impl() {
  try_recur_release();
  func_stk.pop();
}

void RuntimeContext::pop_func(const char *func_name) {
  std::string_view top_func = func_stk.top_func_name();
  assert(top_func == func_name &&
         "Pop func: func name at the top of runtime func stack does not equal "
         "to passed in func_name parameter");
  assert(func_stk.top_loop_empty() &&
         "Pop func: loop stack at top func entry should be empty");
  pop_func_impl();
}

LoopEntry *FuncEntry::top_loop() {
  if (loop_stk.empty()) {
    return nullptr;
  }
  return &loop_stk.top();
}

LoopEntry *FuncStack::top_loop() {
  // assert(!stk.empty() && "Function stack should not be empty at Loop
  // Actions");
  for (int i = stk.size() - 1; i >= 0; i--) {
    LoopEntry *top_loop = stk[i].top_loop();
    if (top_loop) {
      return top_loop;
    }
  }
  return nullptr;
}

void LoopStack::push(const char *loop_loc) { stk.emplace(LoopEntry(loop_loc)); }

void FuncEntry::push_loop(const char *loop_loc) { loop_stk.push(loop_loc); }

void FuncStack::push_loop(const char *loop_loc) {
  // assert(!stk.empty() && "Function stack should not be empty at Loop
  // Actions");
  top_func().push_loop(loop_loc);
}

std::optional<std::size_t> LoopStack::update_top() {
  if (stk.empty())
    return std::nullopt;
  LoopEntry &top_loop = stk.top();
  // update iteration count or other info here if needed
  return top_loop.update_iter();
}

std::optional<std::size_t> FuncEntry::update_top_loop() {
  return loop_stk.update_top();
}
std::optional<std::size_t> FuncStack::update_top_loop() {
  // assert(!stk.empty() && "Function stack should not be empty at Loop
  // Actions");
  return top_func().update_top_loop();
}

std::size_t RuntimeContext::push_loop(const char *loop_loc) {
  if (is_loop_locked())
    return 0;
  func_stk.push_loop(loop_loc);
  return 1;
}

std::size_t RuntimeContext::loop_entry(const char *loop_loc) {
  // stack update
  LoopEntry *top_loop = func_stk.top_loop();
  if (!top_loop) {
    return push_loop(loop_loc);
  }

  if (top_loop->matches(loop_loc)) {
    auto iter_cnt = func_stk.update_top_loop();
    assert(iter_cnt.has_value() &&
           "Loop iteration count update failed at loop entry");
    // lock update
    if (iter_cnt.value() == LOOP_LIMIT + 1) {
      loop_lock = true;
    }
    return iter_cnt.value();
  } else {
    return push_loop(loop_loc);
  }
}

// Pop loop invocation at empty loop stack is allowed
LoopPopResult LoopStack::pop(const char *loop_loc) {
  // assert(!stk.empty() && "Loop Stack should not be empty at pop procedure");
  if (stk.empty()) {
    return {false, 0};
  }
  LoopEntry &top_loop = stk.top();
  if (!top_loop.matches(loop_loc)) {
    return {false, 0};
  }
  // assert(top_loop.matches(loop_loc) &&
  //        "Pop loop: loop at the top of loop stack does not match the passed
  //        in " "loop_loc parameter");
  std::size_t iter_cnt = top_loop.get_iter_cnt();
  stk.pop();
  return {true, iter_cnt};
}

LoopPopResult FuncEntry::pop_loop(const char *loop_loc) {
  return loop_stk.pop(loop_loc);
}

LoopPopResult FuncStack::pop_loop(const char *loop_loc) {
  // assert(!stk.empty() && "Function stack should not be empty at Loop
  // Actions");
  return top_func().pop_loop(loop_loc);
}

void RuntimeContext::update_loop_lock() {
  LoopEntry *top_loop = func_stk.top_loop();
  if (!top_loop || top_loop->get_iter_cnt() <= LOOP_LIMIT) {
    loop_lock = false;
  }
}

LoopPopResult RuntimeContext::loop_out(const char *loop_loc) {
  LoopPopResult res = func_stk.pop_loop(loop_loc);
  update_loop_lock();
  return res;
}

RuntimeContext::RuntimeContext() : loop_lock(false), recur_lock(), func_stk() {
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
  Tid tid = std::this_thread::get_id();
  ss << tid;
  if (is_main) {
    ss << "_main";
  }
  std::string fname_str = ss.str();

  // std::cerr << "fname: " << fname_str << "\n";

  fs::path fname(fname_str);
  fs::path fpath = out_dir / fname;

  out_f.open(fpath, std::ios::out);
  if (!out_f.is_open()) {
    std::cerr << "Failed to open file: " << fpath << "\n";
    std::exit(1);
  }

  is_main = false;
}

void RuntimeContext::close_outf() {
  if (out_f.is_open()) {
    out_f.close();
  }
}

RuntimeContext &RuntimeCtxMap::get_ctx() {
  Tid tid = std::this_thread::get_id();

  std::lock_guard<std::mutex> lock(mtx);
  auto it = ctx_map.find(tid);
  if (it != ctx_map.end()) {
    // use iterator found
    return it->second;
  } else {
    // insert a new RuntimeContext
    return ctx_map[tid];
  }
}

void RuntimeCtxMap::push_func(const char *func_name) {
  RuntimeContext &ctx = get_ctx();
  ctx.push_func(func_name);
}

void RuntimeCtxMap::pop_func(const char *func_name) {
  RuntimeContext &ctx = get_ctx();
  ctx.pop_func(func_name);
}

std::size_t RuntimeCtxMap::loop_entry(const char *loop_loc) {
  RuntimeContext &ctx = get_ctx();
  return ctx.loop_entry(loop_loc);
}

LoopPopResult RuntimeCtxMap::loop_out(const char *loop_loc) {
  RuntimeContext &ctx = get_ctx();
  return ctx.loop_out(loop_loc);
}

bool RuntimeCtxMap::is_recur_locked() {
  RuntimeContext &ctx = get_ctx();
  return ctx.is_recur_locked();
}

bool RuntimeContext::is_locked() {
  return is_loop_locked() || recur_lock.is_locked();
}

bool RuntimeCtxMap::is_locked() {
  RuntimeContext &ctx = get_ctx();
  return ctx.is_locked();
}

std::size_t RuntimeCtxMap::get_func_stack_size() {
  RuntimeContext &ctx = get_ctx();
  return ctx.get_func_stack_size();
}

std::string RuntimeContext::func_unwind() {
  // clone the current function name before popping
  std::string cur_func(func_stk.top_func_name());
  pop_func_impl();
  // update loop lock
  update_loop_lock();
  return cur_func;
}

void RuntimeContext::func_clear_loops() {
  func_stk.top_func().clear_loops();
  update_loop_lock();
}

std::string RuntimeCtxMap::func_unwind() {
  RuntimeContext &ctx = get_ctx();
  return ctx.func_unwind();
}

void RuntimeCtxMap::close_all_outf() {
  std::lock_guard<std::mutex> lock(mtx);
  for (auto &pair : ctx_map) {
    pair.second.close_outf();
  }
}