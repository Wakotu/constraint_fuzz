#ifndef _PLUGIN_H
#define _PLUGIN_H

#include "utils.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/PassManager.h"
#include "llvm/Pass.h"
#include <llvm-19/llvm/IR/Analysis.h>

using namespace llvm;

SrcLoc get_src_loc(Instruction *inst, Module &M);
// Running context: A compilation unit => may contains multiple
class MyPass : public PassInfoMixin<MyPass> {

public:
  PreservedAnalyses run(Module &M, ModuleAnalysisManager &MAM);
  bool runOnModule(Module &M, ModuleAnalysisManager &MAM);
  static bool isRequired();
};

#endif
