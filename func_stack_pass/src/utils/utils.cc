
#include "utils.h"
#include <cxxabi.h>

std::string demangle(const char *mangled_name) {
  int status = -4; // Status value to track success/failure of demangling
  char *demangled_name =
      abi::__cxa_demangle(mangled_name, nullptr, nullptr, &status);
  if (status == 0 && demangled_name != nullptr) {
    std::string result(demangled_name);
    std::free(demangled_name); // Remember to free the allocated memory
    return result;
  } else {
    return mangled_name; // Return the original mangled name if demangling fails
  }
}
