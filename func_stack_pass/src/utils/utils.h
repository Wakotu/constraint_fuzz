#ifndef _UTILS_H
#define _UTILS_H

// #include "llvm-19/llvm/Support/raw_os_ostream.h"
#include "llvm/Support/raw_ostream.h"
#include <optional>
#include <ostream>
#include <string>

std::string demangle(const char *mangled_name);

struct SrcLoc {
  std::string src_path;
  std::optional<unsigned int> line;
  std::optional<unsigned int> col;

  SrcLoc() = default;
  SrcLoc(const char *path, unsigned int line, unsigned int col)
      : src_path(path), line(line), col(col) {}
  bool is_valid() const;
  friend std::ostream &operator<<(std::ostream &os, const SrcLoc &loc);
  friend llvm::raw_ostream &operator<<(llvm::raw_ostream &os,
                                       const SrcLoc &loc);
};

inline std::ostream &operator<<(std::ostream &os, const SrcLoc &loc) {
  if (loc.is_valid()) {
    os << loc.src_path;
    if (loc.line.has_value()) {
      os << ":" << loc.line.value();
      if (loc.col.has_value()) {
        os << ":" << loc.col.value();
      }
    }
  } else {
    os << "NullLoc";
  }
  return os;
}

inline llvm::raw_ostream &operator<<(llvm::raw_ostream &os, const SrcLoc &loc) {
  if (loc.is_valid()) {
    os << loc.src_path;
    if (loc.line.has_value()) {
      os << ":" << loc.line.value();
      if (loc.col.has_value()) {
        os << ":" << loc.col.value();
      }
    }
  } else {
    os << "NullLoc";
  }
  return os;
}

inline bool operator==(const SrcLoc &lhs, const SrcLoc &rhs) {
  // Compare all fields that make a SrcLoc unique
  return lhs.src_path == rhs.src_path &&
         lhs.line == rhs.line && // std::optional has its own operator==
         lhs.col == rhs.col;     // std::optional has its own operator==
}

// define hash specialization for SrcLoc
namespace std {
template <> struct hash<SrcLoc> {
  size_t operator()(const SrcLoc &t) const {
    size_t h1 = std::hash<std::string>{}(t.src_path);
    size_t h2 = std::hash<unsigned int>{}(t.line.value());
    size_t h3 = std::hash<unsigned int>{}(t.col.value());

    // A common and effective hash combination pattern (similar to Boost's
    // hash_combine) This aims to mix the bits from each component's hash value.
    // The magic number 0x9e3779b9 is derived from the golden ratio and is
    // a commonly used prime for hashing.
    size_t seed = 0;
    seed ^= h1 + 0x9e3779b9 + (seed << 6) + (seed >> 2);
    seed ^= h2 + 0x9e3779b9 + (seed << 6) + (seed >> 2);
    seed ^= h3 + 0x9e3779b9 + (seed << 6) + (seed >> 2);

    return seed;
  }
};
} // namespace std

#endif // !DEBUG
