#include "plugin.h"
#include "utils.h"
#include <llvm-19/llvm/ADT/StringRef.h>
#include <optional>
#include <sstream>

#include "color.h"
#include "config.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Passes/PassPlugin.h"
#include "llvm/Support/FileSystem.h" // Required for make_absolute or real_path
#include <llvm-19/llvm/IR/Attributes.h>
#include <llvm-19/llvm/IR/BasicBlock.h>
#include <llvm-19/llvm/IR/DebugLoc.h>
#include <llvm-19/llvm/IR/DerivedTypes.h>
#include <llvm-19/llvm/IR/Function.h>
#include <llvm-19/llvm/IR/Instruction.h>
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
#include <string>
#include <system_error>

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
    errs() << BLUE << "[Func Instrument] " << RESET
           << "Function: " << f.getName() << " in " << file_path << "\n";
    // NOTE: the filtering path may depend on the linux distros
    bool flag = file_path.find("/usr/lib/gcc") != std::string::npos;
    errs() << BLUE << "[Func Instrument] " << RESET << "Function "
           << f.getName() << " " << (flag ? "skipped" : "instrumented") << "\n";
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

bool insert_func(Module &m, ModuleAnalysisManager &mam) {
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

SrcLoc get_src_loc(Instruction *inst, StringRef src_path) {
  SrcLoc loc;
  loc.src_path = src_path;
  const DebugLoc &debug_loc = inst->getDebugLoc();
  if (debug_loc) {
    loc.line = debug_loc.getLine();
    loc.col = debug_loc.getCol();
  } else {
    loc.line = std::nullopt;
    loc.col = std::nullopt;
  }
  return loc;
}

FunctionCallee get_rec_log_func_decl(Module &m) {
  LLVMContext &ctx = m.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *rec_log_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee rec_log_func_cl =
      m.getOrInsertFunction("print_rec_to_file", rec_log_func_ty);
  return rec_log_func_cl;
}

std::string get_src_path(Module &M) {
  std::string rela_path = M.getSourceFileName();
  llvm::SmallString<256> abs_path(rela_path); // Choose a reasonable size

  // Attempt to make it an absolute path
  // Option 1: make_absolute (might not resolve '..' components)
  auto err_code = llvm::sys::fs::make_absolute(abs_path);
  if (err_code) {
    errs() << RED << "[Error] " << RESET
           << "Failed to make absolute path: " << err_code.message() << "\n";
  }
  return abs_path.str().str();
}

void insert_sel_dest_guard(Module &M, BranchInst *br, BasicBlock *dest,
                           bool br_val) {
  // collect message: br src location , dest src location
  std::string src_path = get_src_path(M);
  SrcLoc br_loc = get_src_loc(br, src_path);
  if (!br_loc.is_valid()) {
    errs() << RED << "[Error] " << RESET
           << "Branch instruction has no debug location.\n";
    return;
  }
  Instruction *dest_inst = dest->getFirstNonPHI();
  SrcLoc dest_loc = get_src_loc(dest_inst, src_path);
  if (!dest_loc.is_valid()) {
    errs() << RED << "[Error] " << RESET
           << "Destination block has no debug location.\n";
    return;
  }

  // format rec message
  std::stringstream ss;
  ss << "Selection: " << br_loc << " " << br_val << " " << dest_loc;
  std::string rec = ss.str();

  // add declaration of logging function
  FunctionCallee rec_log_func_cl = get_rec_log_func_decl(M);
  InstrumentationIRBuilder irb(dest_inst);
  // create global string
  auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
  // insert invocation
  irb.CreateCall(rec_log_func_cl, {rec_str_ptr});
}

bool insert_selection(Module &m, ModuleAnalysisManager &mam) {
  bool flag = false;
  for (Function &F : m) {
    for (auto &BB : F) {
      Instruction *term = BB.getTerminator();
      if (BranchInst *br_inst = dyn_cast<BranchInst>(term)) {
        if (br_inst->isConditional()) {
          // locate a conditional br instruction
          flag = true;
          // insert at each destination
          BasicBlock *true_dest = br_inst->getSuccessor(0);
          BasicBlock *false_dest = br_inst->getSuccessor(1);
          insert_sel_dest_guard(m, br_inst, true_dest, true);
          insert_sel_dest_guard(m, br_inst, false_dest, false);
        }
      }
    }
  }
  return flag;
}

bool insert_loop(Module &m, ModuleAnalysisManager &mam) {
  // TODO: Recognize Loop Structure
}

bool MyPass::runOnModule(Module &m, ModuleAnalysisManager &mam) {
  // auto printf_cl = add_printf_decl(m);
  // modification already
  bool flag = false;

  flag |= insert_func(m, mam);
  flag |= insert_selection(m, mam);
  flag |= insert_loop(m, mam);
  return flag;
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
