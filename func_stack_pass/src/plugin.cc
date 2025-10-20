#include "plugin.h"
#include "utils.h"
#include "llvm/Analysis/TargetLibraryInfo.h"
#include <cassert>
#include <cstddef>
#include <iostream>
#include <llvm-19/llvm/ADT/SmallVector.h>
#include <llvm-19/llvm/ADT/StringRef.h>
#include <llvm-19/llvm/ADT/Twine.h>
#include <llvm-19/llvm/Analysis/LoopAccessAnalysis.h>
#include <llvm-19/llvm/Analysis/LoopAnalysisManager.h>
#include <llvm-19/llvm/Analysis/LoopInfo.h>
#include <llvm-19/llvm/IR/Constant.h>
#include <llvm-19/llvm/IR/InstrTypes.h>
#include <llvm-19/llvm/IR/Metadata.h>
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
#include <unordered_set>
#include <vector>

// Only applied for Br Guard
std::unordered_set<SrcLoc> loc_vis;

std::optional<InstrumentationIRBuilder> get_unique_irb(Instruction *inst,
                                                       Module &M) {
  auto src_loc = get_src_loc(inst, M);
  auto it = loc_vis.find(src_loc);
  if (it == loc_vis.end()) {
    loc_vis.insert(src_loc);
    return std::optional<InstrumentationIRBuilder>(inst);
  } else {
    return std::nullopt;
  }
}

PreservedAnalyses MyPass::run(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = runOnModule(M, MAM);
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

FunctionCallee get_pop_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *pop_func_ty = FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee pop_func_cl = M.getOrInsertFunction("pop_func", pop_func_ty);
  return pop_func_cl;
}

FunctionCallee get_push_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *char_ty = Type::getInt8Ty(ctx);
  Type *chat_ptr_ty = PointerType::getUnqual(char_ty);
  FunctionType *push_func_ty = FunctionType::get(void_ty, {chat_ptr_ty}, false);
  FunctionCallee push_func_cl =
      M.getOrInsertFunction("push_func", push_func_ty);
  return push_func_cl;
}

bool is_stdlib_function(StringRef func_name, Function &F,
                        FunctionAnalysisManager &FAM) {
  const llvm::TargetLibraryInfo &TLI =
      FAM.getResult<llvm::TargetLibraryAnalysis>(F);
  llvm::LibFunc FuncID;

  if (TLI.getLibFunc(func_name, FuncID)) { // Check by Function*
    errs() << YELLOW << "[Warning] " << RESET
           << "Known StdLib function: " << func_name
           << " (LibFunc ID: " << FuncID << ")\n";
    return true;
  }
  return false;
}

bool from_stdlib(const Function *F) {
  if (auto *SP = F->getSubprogram()) {
    std::string file_path = SP->getFile()->getFilename().str();
    errs() << BLUE << "[Func Location] " << RESET
           << "Function: " << F->getName() << " in " << file_path << "\n";
    // NOTE: the filtering path may depend on the linux distros
    bool flag = file_path.find("/usr/lib/gcc") != std::string::npos;
    errs() << BLUE << "[Func Location] " << RESET << "Function " << F->getName()
           << " " << (flag ? "skipped" : "to instrument") << "\n";
    return flag;
  }
  errs() << RED << "[Error] " << RESET
         << "Function has no subprogram: " << F->getName() << "\n";
  return false;
}

bool should_skip_func(Function &F, Module &M, ModuleAnalysisManager &MAM) {
  if (F.isDeclaration() || F.isIntrinsic()) {
    return true;
  }

  FunctionAnalysisManager &FAM =
      MAM.getResult<FunctionAnalysisManagerModuleProxy>(M).getManager();
  bool flag = from_stdlib(&F) || is_stdlib_function(F.getName(), F, FAM);
  return flag;
}

bool instru_func_entry_and_exit(Module &M, ModuleAnalysisManager &MAM) {
  auto push_func_cl = get_push_func_decl(M);
  auto pop_func_cl = get_pop_func_decl(M);

  for (Function &F : M) {
    if (should_skip_func(F, M, MAM))
      continue;

    // entry insertion
    auto pt = F.getEntryBlock().getFirstInsertionPt();
    InstrumentationIRBuilder irb(&*pt);

    auto func_name_ptr = irb.CreateGlobalStringPtr(F.getName());
    irb.CreateCall(push_func_cl, {func_name_ptr});

    // exit insertion
    for (auto &bb : F) {
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

std::string get_file_path(const DebugLoc &loc, Module &M) {
  auto *scope = loc->getScope();
  if (!scope) {
    std::string src_path = get_src_path(M);
    return src_path;
  }
  auto dir = scope->getDirectory();
  auto file_name = scope->getFilename();
  return (Twine(dir) + "/" + file_name).str();
}

SrcLoc get_src_loc(Instruction *inst, Module &M) {
  SrcLoc loc;
  const DebugLoc &debug_loc = inst->getDebugLoc();
  loc.src_path = get_file_path(debug_loc, M);
  if (debug_loc) {
    loc.line = debug_loc.getLine();
    loc.col = debug_loc.getCol();
  } else {
    loc.line = std::nullopt;
    loc.col = std::nullopt;
  }
  return loc;
}

FunctionCallee get_rec_log_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *rec_log_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee rec_log_func_cl =
      M.getOrInsertFunction("print_rec_to_file_with_guard", rec_log_func_ty);
  return rec_log_func_cl;
}

FunctionCallee get_content_log_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *rec_log_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee rec_log_func_cl = M.getOrInsertFunction(
      "print_content_to_file_with_guard", rec_log_func_ty);
  return rec_log_func_cl;
}

FunctionCallee get_thread_rec_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();

  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);
  Type *opa_ptr_ty = PointerType::getUnqual(ctx);
  FunctionType *thread_guard_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty, opa_ptr_ty}, false);
  FunctionCallee thread_guard_func_cl =
      M.getOrInsertFunction("thread_rec", thread_guard_func_ty);
  return thread_guard_func_cl;
}

FunctionCallee get_ubv_rec_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);
  FunctionType *ubv_rec_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty, i8_ty}, false);
  FunctionCallee ubv_rec_func_cl =
      M.getOrInsertFunction("ubv_rec", ubv_rec_func_ty);
  return ubv_rec_func_cl;
}

FunctionCallee get_sel_rec_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();

  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);
  FunctionType *sel_rec_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty, i8_ty}, false);
  FunctionCallee sel_rec_func_cl =
      M.getOrInsertFunction("select_rec", sel_rec_func_ty);
  return sel_rec_func_cl;
}

/**
  Br Instruction operations
*/

Instruction *get_cond_inst_from_br(BranchInst *br_inst) {
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
  Instruction *cond_inst = get_cond_inst_from_br(br_inst);
  if (!cond_inst) {
    return false; // no condition instruction
  }
  return isa<PHINode>(cond_inst);
}

/**
  Instrumentation operations
*/

// Instruction *get_first_inst_with_srcloc(BasicBlock &BB, Module &M) {
//   for (Instruction &I : BB) {
//     SrcLoc loc = get_src_loc(&I, M);
//     if (loc.is_valid()) {
//       return &I; // return the first instruction with a valid source location
//     }
//   }
//   // fallback to the first non-PHI instruction
//   return BB.getFirstNonPHI();
// }

void insert_dest_guard_for_jump_inst(Module &M, BasicBlock *dest_bb,
                                     bool br_val) {
  // get dest instruction in dest bb

  Instruction *dest_inst = &*dest_bb->getFirstInsertionPt();

  SrcLoc dest_loc = get_src_loc(dest_inst, M);
  while (!dest_loc.is_valid() && dest_inst->getNextNode()) {
    // try to get the next instruction if the first one has no debug location
    dest_inst = dest_inst->getNextNode();
    dest_loc = get_src_loc(dest_inst, M);
  }

  // dest loc error handle
  if (!dest_loc.has_value()) {
    errs() << RED << "[Error] " << RESET
           << "Destination block has no debug location: ";
    dest_inst->print(errs());
    errs() << "\n";
    // return;
  }

  // dest record part construction
  std::stringstream ss;
  ss << br_val << " " << dest_loc << "\n";
  std::string dest_rec = ss.str();

  // Guard  insert
  FunctionCallee content_log_func_cl = get_content_log_func_decl(M);
  auto irb = get_unique_irb(dest_inst, M);
  if (irb.has_value()) {
    // create global string
    auto rec_str_ptr = irb->CreateGlobalStringPtr(dest_rec);
    // insert invocation
    irb->CreateCall(content_log_func_cl, {rec_str_ptr});
  }
}

/**
  Param:
  - prompt: no trailing ": "
*/
void insert_from_guard_for_jump_inst(Module &M, Instruction *jmp_inst,
                                     const char *prompt) {

  SrcLoc from_loc = get_src_loc(jmp_inst, M);
  if (!from_loc.has_value()) {
    errs() << RED << "[Error] " << RESET
           << "Conditional instruction has no debug location: ";
    jmp_inst->print(errs());
    errs() << "\n";
    // return;
  }

  std::stringstream ss;
  ss << prompt << ": " << from_loc << " ";

  std::string rec = ss.str();

  // add declaration of logging function
  FunctionCallee content_log_func_cl = get_content_log_func_decl(M);
  InstrumentationIRBuilder irb(jmp_inst);
  // create global string
  auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
  // insert invocation
  irb.CreateCall(content_log_func_cl, {rec_str_ptr});
}

// void instr_branch_dest_guard(Module &M, Instruction *jmp_inst,
//                              BasicBlock *dest_bb, bool br_val,
//                              const char *prmpt, bool is_br) {
//   // collect message: br src location , dest src location
//   std::string src_path = get_src_path(M);

//   SrcLoc from_loc = get_src_loc_with_path(jmp_inst, src_path);
//   if (!from_loc.has_value()) {
//     errs() << RED << "[Error] " << RESET
//            << "Conditional instruction has no debug location: ";
//     jmp_inst->print(errs());
//     errs() << "\n";
//     // return;
//   }
//   Instruction *dest_inst = dest_bb->getFirstNonPHI();

//   SrcLoc dest_loc = get_src_loc_with_path(dest_inst, src_path);
//   while (!dest_loc.is_valid() && dest_inst->getNextNode()) {
//     // try to get the next instruction if the first one has no debug location
//     dest_inst = dest_inst->getNextNode();
//     dest_loc = get_src_loc_with_path(dest_inst, src_path);
//   }

//   if (!dest_loc.has_value()) {
//     errs() << RED << "[Error] " << RESET
//            << "Destination block has no debug location: ";
//     dest_inst->print(errs());
//     errs() << "\n";
//     // return;
//   }

//   // format rec message
//   std::stringstream ss;
//   ss << prmpt << ": ";
//   // cond instruction location
//   if (is_br) {

//     BranchInst *br_inst = dyn_cast<BranchInst>(jmp_inst);
//     if (!br_inst) {
//       errs() << RED << "[Error] " << RESET << "jmp_inst is not a BranchInst:
//       "; jmp_inst->print(errs()); errs() << "\n"; ss << "NullLoc "; goto
//       br_rec; // skip the condition location if not a branch instruction
//     }

//     // offer value location
//     Instruction *cond_val_inst = get_cond_inst_from_br(br_inst);
//     if (!cond_val_inst) {
//       ss << "NullLoc ";
//       goto br_rec; // skip the condition location if no condition instruction
//     }
//     // assert(cond_inst && "Condition instruction should not be null");

//     if (!isa<PHINode>(cond_val_inst)) {
//       SrcLoc cond_val_loc = get_src_loc_with_path(cond_val_inst, src_path);
//       ss << cond_val_loc << " ";
//     }
//   }
// br_rec:
//   ss << from_loc << " " << br_val << " " << dest_loc;
//   std::string rec = ss.str();

//   // add declaration of logging function
//   FunctionCallee rec_log_func_cl = get_rec_log_func_decl(M);
//   InstrumentationIRBuilder irb(dest_inst);
//   // create global string
//   auto rec_str_ptr = irb.CreateGlobalStringPtr(rec);
//   // insert invocation
//   irb.CreateCall(rec_log_func_cl, {rec_str_ptr});
// }

// void output_cond_instruction(BranchInst *br_inst, Module &M) {
//   Value *cond = br_inst->getCondition();
//   assert(cond && "Branch instruction has no condition");
//   if (Instruction *I = dyn_cast<Instruction>(cond)) {
//     if (isa<ICmpInst>(I)) {
//       return;
//     }

//     I->print(errs());
//   } else {

//     errs() << "Not an instruction: ";
//     cond->print(errs());
//   }
//   errs() << "\n";
// }

bool instru_for_br_inst(Instruction *term, Module &M) {
  if (BranchInst *br_inst = dyn_cast<BranchInst>(term)) {

    // only instruments at conditional branch instructions
    if (br_inst->isConditional()) {
      // output_cond_instruction(br_inst, M);
      SrcLoc br_loc = get_src_loc(br_inst, M);
      errs() << BLUE << "[Br Instrument] " << RESET
             << "Branch Location: " << br_loc << "\n";
      // locate a conditional br instruction

      // handle From part action record logging
      std::stringstream ss;
      std::string src_path = get_src_path(M);

      // differ at merge and normar br
      bool is_merge = is_merge_br(br_inst);
      const char *prompt;

      // account for prompt and val loc construction
      if (is_merge) {
        prompt = "Merge Br Guard";
        ss << prompt << ": ";
      } else {
        prompt = "Br Guard";
        ss << prompt << ": ";
        // get value instruction location
        Instruction *cond_val_inst = get_cond_inst_from_br(br_inst);
        if (!cond_val_inst) {
          ss << "NullLoc ";
        }

        assert(!isa<PHINode>(cond_val_inst)); // may be omitted
        SrcLoc cond_val_loc = get_src_loc(cond_val_inst, M);
        ss << cond_val_loc << " ";
      }

      // from loc construction
      SrcLoc from_loc = get_src_loc(br_inst, M);
      if (!from_loc.has_value()) {
        errs() << RED << "[Error] " << RESET
               << "Conditional instruction has no debug location: ";
        br_inst->print(errs());
        errs() << "\n";
        // return;
      }

      ss << from_loc << " ";
      std::string from_rec = ss.str();

      // IR insertion
      FunctionCallee content_log_func_cl = get_content_log_func_decl(M);

      InstrumentationIRBuilder irb(br_inst);
      Constant *rec_str_ptr = irb.CreateGlobalStringPtr(from_rec);
      irb.CreateCall(content_log_func_cl, {rec_str_ptr});

      BasicBlock *true_dest = br_inst->getSuccessor(0);
      BasicBlock *false_dest = br_inst->getSuccessor(1);
      insert_dest_guard_for_jump_inst(M, true_dest, true);
      insert_dest_guard_for_jump_inst(M, false_dest, false);
      return true;
    }
  }
  return false;
}

bool instru_for_switch_inst(Instruction *term, Module &M) {
  if (SwitchInst *switch_inst = dyn_cast<SwitchInst>(term)) {
    // logs
    SrcLoc switch_loc = get_src_loc(switch_inst, M);
    errs() << BLUE << "[Switch Instrument] " << RESET
           << "Switch Location: " << switch_loc << "\n";

    insert_from_guard_for_jump_inst(M, switch_inst, "Switch Guard");

    BasicBlock *default_dest = switch_inst->getDefaultDest();
    insert_dest_guard_for_jump_inst(M, default_dest, true);
    for (auto case_it = switch_inst->case_begin();
         case_it != switch_inst->case_end(); ++case_it) {
      BasicBlock *dest = case_it->getCaseSuccessor();
      insert_dest_guard_for_jump_inst(M, dest, true);
    }
    return true;
  }
  return false;
}

bool instru_for_indirectbr_inst(Instruction *term, Module &M) {
  if (IndirectBrInst *indirect_br_inst = dyn_cast<IndirectBrInst>(term)) {
    // locate an indirect br instruction
    SrcLoc indirect_br_loc = get_src_loc(indirect_br_inst, M);
    errs() << BLUE << "[IndirectBr Instrument] " << RESET
           << "Indirect Branch Location: " << indirect_br_loc << "\n";

    insert_from_guard_for_jump_inst(M, indirect_br_inst, "IndirectBr Guard");
    for (BasicBlock *dest : indirect_br_inst->successors()) {
      insert_dest_guard_for_jump_inst(M, dest, true);
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

static std::unordered_set<SrcLoc> ubr_loc_seen;

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

    // errs() << "\n";
    // errs() << GREEN << "[Phi Node Instrument] " << RESET << "Incoming Value:
    // "; val_inst->print(errs()); errs() << "\nIncoming block:";
    // incoming_bb->print(errs());
    // errs() << "phi instruction: ";
    // phi_node->print(errs());
    // errs() << "at pair " << i + 1 << " of " << num_incoming << "\n";

    // assert(val_inst && "Incoming value is not an instruction");
    if (PHINode *sub_node = dyn_cast<PHINode>(val_inst)) {
      errs() << YELLOW << "[Warning] " << RESET
             << "Phi node found in incoming value, recursing into it: ";
      val_inst->print(errs());

      errs() << "\n";
      instr_from_phi_node(sub_node, M);
    } else if (val_inst->getType()->isIntegerTy(1)) {
      SrcLoc val_loc = get_src_loc(val_inst, M);

      auto it = ubr_loc_seen.find(val_loc);

      if (it != ubr_loc_seen.end()) {
        // already seen this location, skip
        continue;
      }
      ubr_loc_seen.insert(val_loc);
      flag = true;

      errs() << BLUE << "[Unconditional Br Value Instrument] " << RESET
             << "Location: " << val_loc << "\n";

      std::stringstream ss;
      ss << val_loc;
      std::string rec = ss.str();

      FunctionCallee ubv_rec_cl = get_ubv_rec_func_decl(M);
      InstrumentationIRBuilder irb(val_inst);

      // NOTE: line break at the end
      // Constant *fmt_str = irb.CreateGlobalStringPtr("UBV Value: %d\n");
      // create global string
      auto loc_str_ptr = irb.CreateGlobalStringPtr(rec);
      // insert invocation
      irb.CreateCall(ubv_rec_cl, {loc_str_ptr, val_inst});
    }
  }

  return flag;
}

// br -> PHI Node -> UBV value starting from phi node
bool instr_unconditional_br_value(Instruction *term, Module &M) {

  BranchInst *br_inst = dyn_cast<BranchInst>(term);
  if (!br_inst) {
    return false; // not a branch instruction
  }
  if (!br_inst->isConditional()) {
    return false;
  }
  Instruction *cond_inst = get_cond_inst_from_br(br_inst);
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

bool instru_for_select_inst(Instruction *inst, Module &M) {
  if (SelectInst *sel_inst = dyn_cast<SelectInst>(inst)) {
    Value *cond_val = sel_inst->getCondition();
    if (Instruction *cond_inst = dyn_cast<Instruction>(cond_val)) {
      if (cond_inst->getType()->isIntegerTy(1)) {
        if (PHINode *phi_node = dyn_cast<PHINode>(cond_inst)) {
          return instr_from_phi_node(phi_node, M);
        }
        SrcLoc cond_loc = get_src_loc(cond_inst, M);

        std::stringstream ss;
        ss << cond_loc;
        std::string cond_loc_str = ss.str();

        FunctionCallee sel_rec_func_cl = get_sel_rec_func_decl(M);

        InstrumentationIRBuilder irb(sel_inst);
        Constant *cond_loc_str_ptr = irb.CreateGlobalStringPtr(cond_loc_str);
        irb.CreateCall(sel_rec_func_cl, {cond_loc_str_ptr, cond_inst});
        return true;
      }
    }
  }
  return false;
}

/**
  Instrumentation at select control flow instructions
  - Branch Instruction
  - Switch Instruction
  - Indirect Branch Instruction:
  - select instruction
*/
bool instru_at_selections(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;
  for (Function &F : M) {
    for (auto &BB : F) {
      Instruction *term = BB.getTerminator();
      flag |= instru_for_br_inst(term, M);
      flag |= instru_for_switch_inst(term, M);
      flag |= instru_for_indirectbr_inst(term, M);
    }
  }

  for (Function &F : M) {
    for (BasicBlock &BB : F) {
      for (Instruction &I : BB) {
        flag |= instru_for_select_inst(&I, M);
      }
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

FunctionCallee get_loop_entry_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *loop_hit_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty}, false);
  FunctionCallee loop_hit_func_cl =
      M.getOrInsertFunction("loop_entry", loop_hit_func_ty);
  return loop_hit_func_cl;
}

FunctionCallee get_loop_end_func_decl(Module &M) {
  LLVMContext &ctx = M.getContext();
  Type *void_ty = Type::getVoidTy(ctx);
  Type *i8_ty = Type::getInt8Ty(ctx);
  Type *i8_ptr_ty = PointerType::getUnqual(i8_ty);

  FunctionType *loop_end_func_ty =
      FunctionType::get(void_ty, {i8_ptr_ty, i8_ptr_ty}, false);
  FunctionCallee loop_end_func_cl =
      M.getOrInsertFunction("loop_end", loop_end_func_ty);
  return loop_end_func_cl;
}

bool instru_at_loop_entry_and_exit(Loop *L, Module &M) {
  errs() << "\n";
  // instrument at loop entry
  BasicBlock *header = L->getHeader();
  if (!header) {
    return false; // no header, nothing to instrument
  }

  // get the first instruction in the header
  Instruction *header_inst = &*header->getFirstInsertionPt();
  if (!header_inst) {
    return false; // no instruction to instrument
  }

  SrcLoc header_loc = get_src_loc(header_inst, M);
  errs() << BLUE << "[Loop Instrument] " << RESET
         << "Loop Header Location: " << header_loc << "\n";
  std::stringstream ss;
  ss << header_loc;

  std::string loop_loc = ss.str();

  /**
  Instrumentation count for specific loop location
  */

  // if (loop_loc ==
  // "/struct_fuzz/constraint_fuzz/output/build/libaom/src/libaom/"
  //                 "av1/common/cdef.c:135:5") {
  //   errs() << YELLOW << "[REPEAT INSTRUMENT] " << RESET
  //          << "Loop Location: " << loop_loc << "\n";
  // }

  /**
  End of instrumentation count for specific loop location
   */

  // create instrumentation IR builder
  InstrumentationIRBuilder irb(header_inst);

  // create a call to loop_hit function with loop location
  auto loop_loc_str = irb.CreateGlobalStringPtr(loop_loc);
  FunctionCallee loop_hit_cl = get_loop_entry_func_decl(M);
  irb.CreateCall(loop_hit_cl, {loop_loc_str});

  // instrument at loop exit
  SmallVector<BasicBlock *, 4> exit_blocks;
  L->getExitBlocks(exit_blocks);
  if (exit_blocks.empty()) {
    return false; // no exit blocks, nothing to instrument
  }

  // instrument each exit block
  for (BasicBlock *exit_block : exit_blocks) {
    // get the first instruction in the exit block
    Instruction *first_inst = &*exit_block->getFirstInsertionPt();
    if (!first_inst) {
      continue; // no instruction to instrument
    }
    // errs() << GREEN << "[Loop Instrument] " << RESET
    //        << "Loop Exit Block: " << exit_block->getName();
    // exit_block->print(errs());
    // errs() << GREEN << "[Loop Instrument] " << RESET
    //        << "First Instruction in Exit Block: ";
    // first_inst->print(errs());
    // errs() << "\n";

    SrcLoc out_loc = get_src_loc(first_inst, M);
    errs() << BLUE << "[Loop Instrument] " << RESET
           << "Loop Exit Location: " << out_loc << "\n";
    std::stringstream ss;
    ss << out_loc;
    std::string out_loc_str = ss.str();

    // create instrumentation IR builder
    InstrumentationIRBuilder irb(first_inst);
    Constant *out_loc_str_ptr = irb.CreateGlobalStringPtr(out_loc_str);
    // create a call to loop_end function
    FunctionCallee loop_end_cl = get_loop_end_func_decl(M);
    irb.CreateCall(loop_end_cl, {loop_loc_str, out_loc_str_ptr});
  }

  return true;
}

using LoopList = std::vector<Loop *>;

LoopList collect_loop_instr_recur(Loop *L) {
  LoopList loops = {L};
  // errs() << GREEN << "[Loop Instrument] " << RESET
  //        << "Collecting loop: " << L->getHeader()->getName() << "\n";

  for (Loop *sub_loop : L->getSubLoops()) {
    // errs() << GREEN << "[Loop Instrument] " << RESET
    //        << "Collecting sub-loop: " << sub_loop->getHeader()->getName()
    //        << "\n";
    LoopList sub_loops = collect_loop_instr_recur(sub_loop);
    // errs() << GREEN << "[Loop Instrument] " << RESET << "Collected "
    //        << sub_loops.size() << " sub-loops, inserting.\n";
    loops.insert(loops.end(), sub_loops.begin(), sub_loops.end());
  }
  return loops;
}

bool instru_for_loop_context(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;
  FunctionAnalysisManager &FAM =
      MAM.getResult<FunctionAnalysisManagerModuleProxy>(M).getManager();

  // errs() << GREEN << "[Loop Instrument] " << RESET
  //        << "Collecting loops in the module...\n";
  LoopList loops;
  for (Function &F : M) {
    if (should_skip_func(F, M, MAM)) {
      continue; // skip declarations
    }
    LoopInfo &LI = FAM.getResult<LoopAnalysis>(F);
    if (LI.empty()) {
      errs() << YELLOW << "[Loop Instrument] " << RESET
             << "No loops found in function: " << F.getName() << "\n";
      continue; // no loops in this function
    }
    for (Loop *L : LI) {
      LoopList sub_loops = collect_loop_instr_recur(L);
      loops.insert(loops.end(), sub_loops.begin(), sub_loops.end());
    }
  }

  errs() << GREEN << "[Loop Instrument] " << RESET << "Found " << loops.size()
         << " loops in the module.\n";

  if (loops.empty()) {
    errs() << YELLOW << "[Loop Instrument] " << RESET
           << "No loops found in the module.\n";
    return false; // no loops to instrument
  }
  for (Loop *L : loops) {
    flag |= instru_at_loop_entry_and_exit(L, M);
  }
  return flag;
}

bool call_inst_should_skip(CallBase *call_inst, Function &F,
                           FunctionAnalysisManager &FAM) {

  Function *called_func = call_inst->getCalledFunction();
  if (!called_func) {
    return false; // not a direct call, e.g., indirect calls
  }

  errs() << BLUE << "[Func Invocation Instrument] " << RESET
         << "Called Function: " << called_func->getName() << "\n";

  // skip standard library functions
  if (called_func->isDeclaration()) {

    if (is_stdlib_function(called_func->getName(), F,
                           FAM)) { // Check by Function*
      return true;

    } else if (called_func->isIntrinsic()) {

      // Check if the function name is a known built-in function
      errs() << YELLOW << "[Warning] " << RESET
             << "Skipping intrinsic function: " << called_func->getName()
             << "\n";
      return true; // skip intrinsic functions

      // Already handled intrinsics if desired
    }
  }

  return false;
}

bool instru_at_func_invocations(Module &M, ModuleAnalysisManager &MAM) {

  FunctionAnalysisManager &FAM =
      MAM.getResult<FunctionAnalysisManagerModuleProxy>(M).getManager();
  bool flag = false;
  for (Function &F : M) {
    if (should_skip_func(F, M, MAM)) {
      continue; // skip declarations and stdlib functions
    }

    // instrument function calls
    for (auto &BB : F) {
      for (Instruction &I : BB) {
        if (CallBase *call_inst = dyn_cast<CallBase>(&I)) {
          if (call_inst_should_skip(call_inst, F, FAM)) {
            continue; // skip declarations and stdlib functions
          }

          flag = true; // found a function call to instrument
          errs() << GREEN << "[Func Invocation Instrument] " << RESET
                 << "Function Call Instruction: ";
          call_inst->print(errs());
          errs() << "\n";

          // instrument the call instruction
          SrcLoc call_loc = get_src_loc(&I, M);
          errs() << BLUE << "[Func Invocation Instrument] " << RESET
                 << "Function Call Location: " << call_loc << "\n";
          errs() << "\n";

          std::stringstream ss;
          ss << "Function Invocation: " << call_loc << " ";
          std::string rec = ss.str();

          // create instrumentation IR builder
          InstrumentationIRBuilder irb(&I);
          auto invoc_rec_str = irb.CreateGlobalStringPtr(rec.c_str());
          auto content_log_func_cl = get_content_log_func_decl(M);
          irb.CreateCall(content_log_func_cl, {invoc_rec_str});
        }
      }
    }
  }
  return flag;
}

bool instru_for_thread_creation(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;
  for (Function &F : M) {
    for (BasicBlock &BB : F) {
      std::vector<CallBase *> thread_create_calls;
      // collect
      for (Instruction &I : BB) {
        if (CallBase *call_inst = dyn_cast<CallBase>(&I)) {
          // check if the call instruction is a thread creation function
          if (call_inst->getCalledFunction() &&
              call_inst->getCalledFunction()->getName() == "pthread_create") {
            flag = true; // found a thread creation call
            thread_create_calls.push_back(call_inst);
          }
        }
      }

      // instrument
      for (CallBase *I : thread_create_calls) {
        errs() << GREEN << "[Thread Creation Instrument] " << RESET
               << "Thread Creation Call Instruction: ";
        I->print(errs());
        errs() << "\n";

        SrcLoc call_loc = get_src_loc(I, M);
        errs() << BLUE << "[Thread Creation Instrument] " << RESET
               << "Thread Creation Location: " << call_loc << "\n";

        Value *tid_ptr = I->getArgOperand(0);

        std::stringstream ss;
        ss << call_loc;
        std::string loc = ss.str();

        // create instrumentation IR builder
        InstrumentationIRBuilder irb(I->getNextNonDebugInstruction());
        auto loc_str = irb.CreateGlobalStringPtr(loc.c_str());
        auto thread_guard_func_cl = get_thread_rec_func_decl(M);
        auto inst = irb.CreateCall(thread_guard_func_cl, {loc_str, tid_ptr});
        errs() << GREEN << "[Thread Creation Instrument] " << RESET
               << "Thread Guard Function Call Instruction: ";
        inst->print(errs());
        errs() << "\n";
      }
    }
  }

  return flag;
}

bool instru_for_longjmp_invocation(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;
  for (Function &F : M) {
    for (BasicBlock &BB : F) {
      std::vector<CallBase *> longjmp_calls;

      // collect
      for (Instruction &I : BB) {
        if (CallBase *call_inst = dyn_cast<CallBase>(&I)) {
          // check if the call instruction is a longjmp function
          if (call_inst->getCalledFunction() &&
              call_inst->getCalledFunction()->getName() == "longjmp") {
            flag = true; // found a longjmp call
            longjmp_calls.push_back(call_inst);
          }
        }
      }

      // instrument
      for (CallBase *I : longjmp_calls) {
        errs() << GREEN << "[Longjmp Invocation Instrument] " << RESET
               << "Longjmp Call Instruction: ";
        I->print(errs());
        errs() << "\n";

        SrcLoc call_loc = get_src_loc(I, M);
        errs() << BLUE << "[Longjmp Invocation Instrument] " << RESET
               << "Longjmp Call Location: " << call_loc << "\n";

        std::stringstream ss;
        ss << "Longjmp Invocation: " << call_loc;
        std::string rec = ss.str();

        // create instrumentation IR builder
        InstrumentationIRBuilder irb(I);
        auto invoc_rec_str = irb.CreateGlobalStringPtr(rec.c_str());
        auto content_log_func_cl = get_content_log_func_decl(M);
        irb.CreateCall(content_log_func_cl, {invoc_rec_str});
      }
    }
  }

  return flag;
}

bool instru_for_setjmp_invocations(Module &M, ModuleAnalysisManager &MAM) {
  bool flag = false;

  for (Function &F : M) {
    for (BasicBlock &BB : F) {
      std::vector<CallBase *> setjmp_calls;

      // collect
      for (Instruction &I : BB) {
        if (CallBase *call_inst = dyn_cast<CallBase>(&I)) {
          // check if the call instruction is a setjmp function
          if (call_inst->getCalledFunction() &&
              call_inst->getCalledFunction()->getName() == "setjmp") {

            flag = true; // found a setjmp call
            setjmp_calls.push_back(call_inst);
          }
        }
      }

      // instrument
      for (CallBase *I : setjmp_calls) {
        // compile time logs
        errs() << GREEN << "[Setjmp Invocation Instrument] " << RESET
               << "Setjmp Call Instruction: ";
        I->print(errs());
        errs() << "\n";

        // parameter value colletion
        SrcLoc call_loc = get_src_loc(I, M);
        errs() << BLUE << "[Setjmp Invocation Instrument] " << RESET
               << "Setjmp Call Location: " << call_loc << "\n";

        std::stringstream ss;
        ss << "Setjmp Invocation: " << call_loc;
        std::string rec = ss.str();

        // create instrumentation IR builder
        InstrumentationIRBuilder irb(I);
        auto invoc_rec_str = irb.CreateGlobalStringPtr(rec.c_str());
        auto content_log_func_cl = get_content_log_func_decl(M);
        irb.CreateCall(content_log_func_cl, {invoc_rec_str});
      }
    }
  }
  return flag;
}

bool MyPass::runOnModule(Module &M, ModuleAnalysisManager &MAM) {
  // auto printf_cl = add_printf_decl(m);
  // modification already
  bool flag = false;

  // invocation instrumentation should be done first
  flag |= instru_at_func_invocations(M, MAM);
  flag |= instru_func_entry_and_exit(M, MAM);
  // flag |= instr_bool_value(M);
  flag |= instru_for_loop_context(M, MAM);
  flag |= instru_for_thread_creation(M, MAM);
  flag |= instru_for_setjmp_invocations(M, MAM);
  flag |= instru_for_longjmp_invocation(M, MAM);
  flag |= instru_at_selections(M, MAM);
  return flag;
}

bool MyPass::isRequired() { return true; }

// registry function
extern "C" LLVM_ATTRIBUTE_WEAK ::llvm::PassPluginLibraryInfo
llvmGetPassPluginInfo() {
  return {LLVM_PLUGIN_API_VERSION, PLUGIN_NAME, "v0.1", [](PassBuilder &PB) {
            PB.registerOptimizerEarlyEPCallback(
                [](ModulePassManager &MPM, OptimizationLevel) {
                  MPM.addPass(MyPass());
                });
          }};
}
