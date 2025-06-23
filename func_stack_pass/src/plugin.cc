#include "plugin.h"
#include "utils.h"
#include <cassert>
#include <iostream>
#include <llvm-19/llvm/ADT/StringRef.h>
#include <llvm-19/llvm/IR/Constant.h>
#include <llvm-19/llvm/IR/Value.h>
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

SrcLoc get_src_loc(Instruction *inst, Module &M) {
  std::string src_path = get_src_path(M);
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
SrcLoc get_src_loc_with_path(Instruction *inst, StringRef src_path) {
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

/**
  Br Instruction operations
*/

Instruction *get_cond_instr_from_br(BranchInst *br_inst) {
  Value *cond = br_inst->getCondition();
  if (!cond) {
    errs() << RED << "[Error] " << RESET
           << "Branch instruction has no condition: ";
    br_inst->print(errs());
    errs() << "\n";
    return nullptr; // no condition instruction
  }

  if (Instruction *I = dyn_cast<Instruction>(cond)) {
    return I;
  }
  // if the condition is not an instruction, return null
  errs() << RED << "[Error] " << RESET
         << "Branch condition is not an instruction\n";
  return nullptr;
}

bool is_merge_br(BranchInst *br_inst) {
  Instruction *cond_inst = get_cond_instr_from_br(br_inst);
  if (!cond_inst) {
    return false; // no condition instruction
  }
  return isa<PHINode>(cond_inst);
}

/**
  Instrumentation operations
*/

void instr_branch_dest_guard(Module &M, Instruction *jmp_inst, BasicBlock *dest,
                             bool br_val, const char *prmpt, bool is_br) {
  // collect message: br src location , dest src location
  std::string src_path = get_src_path(M);

  SrcLoc br_loc = get_src_loc_with_path(jmp_inst, src_path);
  if (!br_loc.is_valid()) {
    errs() << RED << "[Error] " << RESET
           << "Conditional instruction has no debug location: ";
    jmp_inst->print(errs());
    errs() << "\n";
    // return;
  }
  Instruction *dest_inst = dest->getFirstNonPHI();

  SrcLoc dest_loc = get_src_loc_with_path(dest_inst, src_path);
  while (!dest_loc.is_valid() && dest_inst->getNextNode()) {
    // try to get the next instruction if the first one has no debug location
    dest_inst = dest_inst->getNextNode();
    dest_loc = get_src_loc_with_path(dest_inst, src_path);
  }

  if (!dest_loc.is_valid()) {
    errs() << RED << "[Error] " << RESET
           << "Destination block has no debug location: ";
    dest_inst->print(errs());
    errs() << "\n";
    // return;
  }

  // format rec message
  std::stringstream ss;
  ss << prmpt << ": ";
  // cond instruction location
  if (is_br) {

    BranchInst *br_inst = dyn_cast<BranchInst>(jmp_inst);
    if (!br_inst) {
      errs() << RED << "[Error] " << RESET << "jmp_inst is not a BranchInst: ";
      jmp_inst->print(errs());
      errs() << "\n";
      ss << "NullLoc ";
      goto br_rec; // skip the condition location if not a branch instruction
    }
    Instruction *cond_inst = get_cond_instr_from_br(br_inst);
    if (!cond_inst) {
      ss << "NullLoc ";
      goto br_rec; // skip the condition location if no condition instruction
    }
    // assert(cond_inst && "Condition instruction should not be null");

    if (!isa<PHINode>(cond_inst)) {
      SrcLoc cond_loc = get_src_loc_with_path(cond_inst, src_path);
      ss << cond_loc << " ";
    }
  }
br_rec:
  ss << br_loc << " " << br_val << " " << dest_loc;
  std::string rec = ss.str();

  // add declaration of logging function
  FunctionCallee rec_log_func_cl = get_rec_log_func_decl(M);
  InstrumentationIRBuilder irb(dest_inst);
  // create global string
  auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
  // insert invocation
  irb.CreateCall(rec_log_func_cl, {rec_str_ptr});
}

// void output_cond_instruction(BranchInst *br_inst, Module &M) {
//   Value *cond = br_inst->getCondition();
//   assert(cond && "Branch instruction has no condition");
//   if (Instruction *I = dyn_cast<Instruction>(cond)) {
//     if (isa<ICmpInst>(I)) {
//       return;
//     }

//     errs() << GREEN << "[Br Condition] " << RESET << "Instruction: ";
//     I->print(errs());
//   } else {

//     errs() << GREEN << "[Br Condition] " << RESET;
//     errs() << "Not an instruction: ";
//     cond->print(errs());
//   }
//   errs() << "\n";
// }

bool instr_br_inst(Instruction *term, Module &M) {
  if (BranchInst *br_inst = dyn_cast<BranchInst>(term)) {
    if (br_inst->isConditional()) {
      // output_cond_instruction(br_inst, M);
      SrcLoc br_loc = get_src_loc(br_inst, M);
      errs() << BLUE << "[Br Instrument] " << RESET
             << "Branch Location: " << br_loc << "\n";
      // locate a conditional br instruction

      const char *prmpt = is_merge_br(br_inst) ? "Merge Br Guard" : "Br Guard";

      BasicBlock *true_dest = br_inst->getSuccessor(0);
      BasicBlock *false_dest = br_inst->getSuccessor(1);
      instr_branch_dest_guard(M, br_inst, true_dest, true, prmpt, true);
      instr_branch_dest_guard(M, br_inst, false_dest, false, prmpt, true);
      return true;
    }
  }
  return false;
}

bool instr_switch_inst(Instruction *term, Module &M) {
  if (SwitchInst *switch_inst = dyn_cast<SwitchInst>(term)) {
    // locate a switch instruction
    SrcLoc switch_loc = get_src_loc(switch_inst, M);
    errs() << BLUE << "[Switch Instrument] " << RESET
           << "Switch Location: " << switch_loc << "\n";
    BasicBlock *default_dest = switch_inst->getDefaultDest();
    instr_branch_dest_guard(M, switch_inst, default_dest, false, "Switch Guard",
                            false);
    for (auto case_it = switch_inst->case_begin();
         case_it != switch_inst->case_end(); ++case_it) {
      BasicBlock *dest = case_it->getCaseSuccessor();
      instr_branch_dest_guard(M, switch_inst, dest, true, "Switch Guard",
                              false);
    }
    return true;
  }
  return false;
}

bool instr_indirectbr_inst(Instruction *term, Module &M) {
  if (IndirectBrInst *indirect_br_inst = dyn_cast<IndirectBrInst>(term)) {
    // locate an indirect br instruction
    SrcLoc indirect_br_loc = get_src_loc(indirect_br_inst, M);
    errs() << BLUE << "[IndirectBr Instrument] " << RESET
           << "Indirect Branch Location: " << indirect_br_loc << "\n";
    for (BasicBlock *dest : indirect_br_inst->successors()) {
      instr_branch_dest_guard(M, indirect_br_inst, dest, true,
                              "IndirectBr Guard", false);
    }
    return true;
  }
  return false;
}

bool is_bool_value(Instruction *I) {
  // check if the instruction is a phi instruction
  if (isa<PHINode>(I)) {
    return false; // skip phi nodes
  }
  Type *ty = I->getType();
  // Check if the type is a boolean type
  if (ty->isIntegerTy(1)) {
    return true;
  }
  return false;
}

// static std::unordered_set<SrcLoc> bool_loc_seen;

// bool instr_bool_value(Module &M) {
//   bool flag = false;
//   for (Function &F : M) {
//     for (auto &BB : F) {
//       for (Instruction &I : BB) {
//         if (is_bool_value(&I)) {

//           flag = true;
//           // errs() << GREEN << "Before Loc Get" << RESET << "\n";
//           SrcLoc loc = get_src_loc(&I, M);

//           auto it = bool_loc_seen.find(loc);
//           if (it != bool_loc_seen.end()) {
//             // already seen this location, skip
//             continue;
//           }
//           bool_loc_seen.insert(loc);

//           errs() << BLUE << "[Bool Value Instrument] " << RESET
//                  << "Boolean Value Location: " << loc << ", "
//                  << "Instruction: ";
//           I.print(errs());
//           errs() << "\n";
//           // Here you can add instrumentation logic for boolean values

//           // construct rec string

//           std::stringstream ss;
//           ss << "Boolean Value: " << loc;
//           std::string rec = ss.str();

//           FunctionCallee rec_log_func_cl = get_rec_log_func_decl(M);
//           InstrumentationIRBuilder irb(&I);
//           // create global string
//           errs() << GREEN << "Before Instrumentation" << RESET << "\n";
//           LLVM_DEBUG(dbgs() << "My debug message\n");
//           auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
//           // insert invocation
//           irb.CreateCall(rec_log_func_cl, {rec_str_ptr});
//         }
//       }
//     }
//   }
//   return flag;
// }

bool is_unconditional_br(Instruction *I) {
  BranchInst *br_inst = dyn_cast<BranchInst>(I);
  if (!br_inst) {
    return false; // not a branch instruction
  }
  return !br_inst->isConditional(); // true if it's an unconditional branch
}

bool instr_from_phi_node(PHINode *phi_node, Module &M) {
  bool flag = false;

  unsigned num_incoming = phi_node->getNumIncomingValues();
  for (int i = 0; i < num_incoming; i++) {
    Value *incoming_val = phi_node->getIncomingValue(i);
    BasicBlock *incoming_bb = phi_node->getIncomingBlock(i);

    Instruction *bb_term = incoming_bb->getTerminator();
    if (!is_unconditional_br(bb_term)) {
      continue; // not an unconditional branch
    }
    if (isa<Constant>(incoming_val)) {
      continue;
    }

    Instruction *val_inst = dyn_cast<Instruction>(incoming_val);
    if (!val_inst) {
      errs() << RED << "[Error] " << RESET
             << "Incoming value is not an instruction: ";
      incoming_val->print(errs());
      errs() << "\nIncoming block:";
      incoming_bb->print(errs());
      errs() << "phi instruction: ";
      phi_node->print(errs());
      errs() << "\n";
      errs() << "pair " << i + 1 << " of " << num_incoming << "\n";
      errs() << "\n";
      continue; // skip if the incoming value is not an instruction
    }
    // assert(val_inst && "Incoming value is not an instruction");
    if (PHINode *sub_node = dyn_cast<PHINode>(val_inst)) {
      errs() << YELLOW << "[Warning] " << RESET
             << "Phi node found in incoming value, recursing into it: ";
      val_inst->print(errs());
      errs() << "\n";
      instr_from_phi_node(sub_node, M);
    } else {
      flag = true;
      SrcLoc val_loc = get_src_loc(val_inst, M);

      /** message for debug */
      errs() << GREEN
             << "Before Unconditional Br Value Instrumentation: " << RESET;
      // output val_inst text
      val_inst->print(errs());
      errs() << "\n";

      errs() << BLUE << "[Unconditional Br Value Instrument] " << RESET
             << "Location: " << val_loc << "\n";

      std::stringstream ss;
      ss << "Unconditional Branch Value: " << val_loc;
      std::string rec = ss.str();

      FunctionCallee rec_log_func_cl = get_rec_log_func_decl(M);
      InstrumentationIRBuilder irb(val_inst);
      // create global string
      auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
      // insert invocation
      irb.CreateCall(rec_log_func_cl, {rec_str_ptr});
    }
  }

  return flag;
}

bool instr_unconditional_br_value(Instruction *term, Module &M) {

  BranchInst *br_inst = dyn_cast<BranchInst>(term);
  if (!br_inst) {
    return false; // not a branch instruction
  }
  if (!br_inst->isConditional()) {
    return false;
  }
  Instruction *cond_inst = get_cond_instr_from_br(br_inst);
  if (!cond_inst) {
    return false;
  }
  PHINode *phi_node = dyn_cast<PHINode>(cond_inst);
  if (!phi_node) {
    return false; // not a phi node
  }
  bool flag = instr_from_phi_node(phi_node, M);
  return flag; // return the result of phi node instrumentation
}

/**
  Distinguish guard with different instructions: br, switch, indirectbr
  add bool value instrumentation
*/

bool insert_branches(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;
  for (Function &F : M) {
    for (auto &BB : F) {
      Instruction *term = BB.getTerminator();
      flag |= instr_br_inst(term, M);
      flag |= instr_switch_inst(term, M);
      flag |= instr_indirectbr_inst(term, M);
    }
  }

  for (Function &F : M) {
    for (auto &BB : F) {
      Instruction *term = BB.getTerminator();
      flag |= instr_unconditional_br_value(term, M);
    }
  }
  return flag;
}

// bool insert_loop(Module &m, ModuleAnalysisManager &mam) {
// }

bool MyPass::runOnModule(Module &M, ModuleAnalysisManager &mam) {
  // auto printf_cl = add_printf_decl(m);
  // modification already
  bool flag = false;

  flag |= insert_func(M, mam);
  flag |= insert_branches(M, mam);
  // flag |= instr_bool_value(M);
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
