# Constraint Fuzz

## Instrumentation Part

The instrumentation part of this project was written based on LLVM/Clang Infrastructure.
We split the instrumentation functionality into 2 subprojects: pass plugin and cc wrapper.

- `pass plugin` implements instrumentation logic: where and what to instrument.
- `cc wrapper` is responsible for handling clang options when applying customized pass plugin to target project.

### Usage

You need to run `./install.sh` to install `cc_wrapper` and pass plugin lib to specified location so that `cc_wrapper` utility can be invoked at command line directly.

While compile-time options and flags are handled by `cc_wrapper`, we exposed some envrironmental variables to control runtime behavior of the instrumented target project.

- `FUNC_STACK_OUT`: specifies to which folder the program outputs its execution record

## Constraint Inference Part

To be continued...
