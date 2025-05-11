#include "plugin.h"

#include "color.h"
#include "config.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Passes/PassPlugin.h"
#include <llvm-19/llvm/IR/Attributes.h>
#include <llvm-19/llvm/IR/DerivedTypes.h>
#include <llvm-19/llvm/IR/Function.h>
#include <llvm-19/llvm/IR/Instructions.h>
#include <llvm-19/llvm/IR/LLVMContext.h>
#include <llvm-19/llvm/IR/Module.h>
#include <llvm-19/llvm/IR/PassManager.h>
#include <llvm-19/llvm/IR/Type.h>
#include <llvm-19/llvm/Pass.h>
#include <llvm-19/llvm/Passes/OptimizationLevel.h>
#include <llvm-19/llvm/Support/Casting.h>
#include <llvm-19/llvm/Support/raw_ostream.h>
#include <llvm-19/llvm/Transforms/Instrumentation.h>

PreservedAnalyses MyPass::run(Module &m, ModuleAnalysisManager &mam) {
  bool flag = runOnModule(m, mam);
  if (flag) {
    return PreservedAnalyses::none();
  } else {
    return PreservedAnalyses::all();
  }
}

// FunctionCallee add_printf_decl(Module &m) {
//   LLVMContext &ctx = m.getContext();
//   Type *i8_ty = Type::getInt8Ty(ctx);
//   Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);
//   Type *i32_ty = Type::getInt32Ty(ctx);
//
//   FunctionType *printf_ty = FunctionType::get(i32_ty, {i8_ptr_ty}, true);
//   FunctionCallee printf_cl = m.getOrInsertFunction("printf", printf_ty);
//   Function *printf_fn = dyn_cast<Function>(printf_cl.getCallee());
//   printf_fn->setDoesNotThrow();
//   printf_fn->addParamAttr(0, Attribute::NoCapture);
//   printf_fn->addParamAttr(0, Attribute::ReadOnly);
//   return printf_cl;
// }

FunctionCallee get_pop_func_decl(Module &m) {
  LLVMContext &ctx = m.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *pop_func_ty = FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee pop_func_cl = m.getOrInsertFunction("pop_func", pop_func_ty);
  return pop_func_cl;
}

FunctionCallee get_push_func_decl(Module &m) {
  LLVMContext &ctx = m.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *char_ty = Type::getInt8Ty(ctx);
  Type *chat_ptr_ty = PointerType::getUnqual(char_ty);
  FunctionType *push_func_ty = FunctionType::get(void_ty, {chat_ptr_ty}, false);
  FunctionCallee push_func_cl =
      m.getOrInsertFunction("push_func", push_func_ty);
  return push_func_cl;
}

bool from_stdlib(const Function &f) {
  if (auto *SP = f.getSubprogram()) {
    std::string file_path = SP->getFile()->getFilename().str();
    errs() << BLUE << "[Instrument] " << RESET << "Function: " << f.getName()
           << " in " << file_path << "\n";
    // NOTE: the filtering path may depend on the linux distros
    bool flag = file_path.find("/usr/lib/gcc") != std::string::npos;
    errs() << BLUE << "[Instrument] " << RESET << "Function " << f.getName()
           << " " << (flag ? "skipped" : "instrumented") << "\n";
    return flag;
  }
  return false;
}

bool should_skip_func(const Function &f) {
  if (f.isDeclaration()) {
    return true;
  }

  return from_stdlib(f);
}

bool MyPass::runOnModule(Module &m, ModuleAnalysisManager &mam) {
  // auto printf_cl = add_printf_decl(m);
  auto push_func_cl = get_push_func_decl(m);
  auto pop_func_cl = get_pop_func_decl(m);

  for (Function &f : m) {
    if (should_skip_func(f))
      continue;

    // entry insertion
    auto pt = f.getEntryBlock().getFirstInsertionPt();
    InstrumentationIRBuilder irb(&*pt);

    auto func_name_ptr = irb.CreateGlobalStringPtr(f.getName());
    irb.CreateCall(push_func_cl, {func_name_ptr});

    // exit insertion
    for (auto &bb : f) {
      if (ReturnInst *ret_inst = dyn_cast<ReturnInst>(bb.getTerminator())) {
        InstrumentationIRBuilder irb(ret_inst);
        irb.CreateCall(pop_func_cl, {func_name_ptr});
      }
    }
  }

  return true;
}

bool MyPass::isRequired() { return true; }

// registry function
extern "C" LLVM_ATTRIBUTE_WEAK ::llvm::PassPluginLibraryInfo
llvmGetPassPluginInfo() {
  return {LLVM_PLUGIN_API_VERSION, PLUGIN_NAME, "v0.1", [](PassBuilder &PB) {
            PB.registerOptimizerEarlyEPCallback(
                [](ModulePassManager &mpm, OptimizationLevel) {
                  mpm.addPass(MyPass());
                });
          }};
}
