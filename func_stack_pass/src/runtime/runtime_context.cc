#include "runtime_context.h"
#include "config.h"
#include <cassert>
#include <cstddef>
#include <mutex>
#include <optional>
#include <string_view>

void FuncStack::push(const char *func_name) {
  FuncEntry ent(func_name);
  stk.push_back(ent);
}

void FuncStack::pop() { stk.pop_back(); }

bool FuncStack::check_recur() const {
  std::string_view cur_func = stk.back().get_func_name();
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
  if (stk.empty()) {
    return "";
  }
  return stk.back().get_func_name();
}

void RecurLock::lock(std::string_view func_name, std::size_t stk_size) {
  RecurFrame frame(func_name, stk_size);
  this->value = true;
  this->frame = frame;
}

void RecurLock::release() {
  this->value = false;
  this->frame = std::nullopt;
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

void RuntimeContext::pop_func(const char *func_name) {
  std::string_view top_func = func_stk.top_func_name();
  assert(top_func == func_name &&
         "Pop func: func name at the top of runtime func stack does not equal "
         "to passed in func_name parameter");
  try_recur_release();
  func_stk.pop();
}

LoopEntry *FuncEntry::top_loop() {
  if (loop_stk.empty()) {
    return nullptr;
  }
  return &loop_stk.top();
}

LoopEntry *FuncStack::top_loop() {
  assert(!stk.empty() && "Function stack should not be empty at Loop Actions");
  return stk.back().top_loop();
}

void LoopStack::push(const char *loop_loc) { stk.emplace(LoopEntry(loop_loc)); }

void FuncEntry::push_loop(const char *loop_loc) { loop_stk.push(loop_loc); }

void FuncStack::push_loop(const char *loop_loc) {
  assert(!stk.empty() && "Function stack should not be empty at Loop Actions");
  stk.back().push_loop(loop_loc);
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
  assert(!stk.empty() && "Function stack should not be empty at Loop Actions");
  return stk.back().update_top_loop();
}

void RuntimeContext::push_loop(const char *loop_loc) {
  if (is_loop_locked())
    return;
  func_stk.push_loop(loop_loc);
}

void RuntimeContext::loop_entry(const char *loop_loc) {
  // stack update
  LoopEntry *top_loop = func_stk.top_loop();
  if (!top_loop) {
    push_loop(loop_loc);
    return;
  }

  if (top_loop->matches(loop_loc)) {
    auto iter_cnt = func_stk.update_top_loop();
    assert(iter_cnt.has_value() &&
           "Loop iteration count update failed at loop entry");
    // lock update
    if (iter_cnt.value() > LOOP_LIMIT) {
      loop_lock = true;
    }
  } else {
    push_loop(loop_loc);
  }
}

void LoopStack::pop(const char *loop_loc) {
  assert(!stk.empty() && "Loop Stack should not be empty at pop procedure");
  LoopEntry &top_loop = stk.top();
  assert(top_loop.matches(loop_loc) &&
         "Pop loop: loop at the top of loop stack does not match the passed in "
         "loop_loc parameter");
  stk.pop();
}

void FuncEntry::pop_loop(const char *loop_loc) { loop_stk.pop(loop_loc); }

void FuncStack::pop_loop(const char *loop_loc) {
  assert(!stk.empty() && "Function stack should not be empty at Loop Actions");
  stk.back().pop_loop(loop_loc);
}

void RuntimeContext::loop_out(const char *loop_loc) {
  func_stk.pop_loop(loop_loc);
  LoopEntry *top_loop = func_stk.top_loop();
  if (!top_loop || top_loop->get_iter_cnt() <= LOOP_LIMIT) {
    loop_lock = false;
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

void RuntimeCtxMap::loop_entry(const char *loop_loc) {
  RuntimeContext &ctx = get_ctx();
  ctx.loop_entry(loop_loc);
}

void RuntimeCtxMap::loop_out(const char *loop_loc) {
  RuntimeContext &ctx = get_ctx();
  ctx.loop_out(loop_loc);
}
