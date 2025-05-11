#ifndef _PLUGIN_H
#define _PLUGIN_H

#include "llvm/IR/Module.h"
#include "llvm/IR/PassManager.h"
#include "llvm/Pass.h"
#include <llvm-19/llvm/IR/Analysis.h>

using namespace llvm;

class MyPass : public PassInfoMixin<MyPass> {
public:
  PreservedAnalyses run(Module &m, ModuleAnalysisManager &mam);
  bool runOnModule(Module &m, ModuleAnalysisManager &mam);
  static bool isRequired();
};

#endif
