#ifndef _RUNTIME_STACK_H_
#define _RUNTIME_STACK_H_

#include <cstddef>
#include <map>
#include <mutex>
#include <optional>
#include <stack>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

class LoopEntry {
  std::string header_loc;
  std::size_t iter_cnt;

public:
  LoopEntry(const char *loop_loc) : header_loc(loop_loc), iter_cnt(1) {}
  std::size_t get_iter_cnt() const { return iter_cnt; }
  bool matches(const char *loop_loc) const { return header_loc == loop_loc; }
  std::size_t update_iter() {
    ++iter_cnt;
    return iter_cnt;
  }
};

class LoopStack {
  std::stack<LoopEntry> stk;

public:
  LoopEntry &top() { return stk.top(); }
  bool empty() const { return stk.empty(); }

  void push(const char *loop_loc);
  void pop(const char *loop_loc);

  std::optional<std::size_t> update_top();
};

class FuncEntry {
  std::string func_name;
  LoopStack loop_stk;

public:
  std::string_view get_func_name() const { return func_name; }
  FuncEntry(const char *func_name) : func_name(func_name), loop_stk() {}

  LoopEntry *top_loop();
  void push_loop(const char *loop_loc);
  void pop_loop(const char *loop_loc);
  std::optional<std::size_t> update_top_loop();
};

class FuncStack {
  std::vector<FuncEntry> stk;

public:
  bool check_recur() const;
  std::size_t size() const { return stk.size(); }
  std::string_view top_func_name() const;

  LoopEntry *top_loop();
  void push_loop(const char *loop_loc);
  void pop_loop(const char *loop_loc);

  std::optional<std::size_t> update_top_loop();

  void push(const char *func_name);
  void pop();
};

class RecurFrame {
  std::string func_name;
  std::size_t stk_size;

public:
  RecurFrame(std::string_view func_name, std::size_t stk_size)
      : func_name(func_name), stk_size(stk_size) {}

  bool matches(std::string_view func_name, std::size_t stk_size) const {
    return this->func_name == func_name && this->stk_size == stk_size;
  }
};

class RecurLock {
  bool value;
  std::optional<RecurFrame> frame;

public:
  RecurLock() : value(false), frame(std::nullopt) {}

  bool is_locked() const { return value; }

  void lock(std::string_view func_name, std::size_t stk_size);
  void release();
  bool matches(std::string_view func_name, std::size_t stk_size) const;
};

class RuntimeContext {
  // global lock state
  bool loop_lock;
  RecurLock recur_lock;

  // stack state
  FuncStack func_stk;

  void set_recur_lock();

  bool is_loop_locked() const { return loop_lock; }
  void push_loop(const char *loop_loc);

public:
  RuntimeContext() : loop_lock(false), recur_lock(), func_stk() {}
  // should be invoked after func stack push
  void try_recur_lock();
  // should be invoked before func stack pop
  void try_recur_release();

  void push_func(const char *func_name);
  void pop_func(const char *func_name);
  void loop_entry(const char *loop_loc);
  void loop_out(const char *loop_loc);
};

using Tid = std::thread::id;
class RuntimeCtxMap {
  std::map<Tid, RuntimeContext> ctx_map;
  std::mutex mtx;

  RuntimeContext &get_ctx();

public:
  void push_func(const char *func_name);
  void pop_func(const char *func_name);
  void loop_entry(const char *loop_loc);
  void loop_out(const char *loop_loc);
};

#endif // _RUNTIME_STACK_H_