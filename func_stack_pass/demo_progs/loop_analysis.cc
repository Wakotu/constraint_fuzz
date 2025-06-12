#include "llvm/Analysis/LoopAnalysis.h" // Required for LoopAnalysis (to get LoopInfo from FAM)
#include "llvm/Analysis/LoopInfo.h" // Required for LoopInfo
#include "llvm/IR/Function.h"
#include "llvm/IR/PassManager.h"
#include "llvm/Support/raw_ostream.h" // For llvm::outs()

namespace {

// Define your LLVM FunctionPass
struct MyLoopAnalysisPass : public llvm::PassInfoMixin<MyLoopAnalysisPass> {
  llvm::PreservedAnalyses run(llvm::Function &F,
                              llvm::FunctionAnalysisManager &FAM) {
    llvm::outs() << "Function: " << F.getName() << "\n";

    // 1. Get the LoopInfo analysis result for the current function
    // LoopInfo is a FunctionAnalysis, so we retrieve it from the
    // FunctionAnalysisManager.
    llvm::LoopInfo &LI = FAM.getResult<llvm::LoopAnalysis>(F);

    // 2. Iterate over all top-level loops in the function
    if (LI.empty()) {
      llvm::outs() << "  No loops found.\n";
    } else {
      llvm::outs() << "  Loops found:\n";
      for (llvm::Loop *L : LI) { // Iterate over top-level loops
        llvm::outs() << "    Loop Header: " << L->getHeader()->getName()
                     << "\n";
        llvm::outs() << "    Loop Depth: " << L->getLoopDepth() << "\n";

        // Print all blocks in the loop
        llvm::outs() << "    Loop Blocks:\n";
        for (llvm::BasicBlock *BB : L->getBlocks()) {
          llvm::outs() << "      - " << BB->getName() << "\n";
        }

        // Check for nested loops
        if (!L->getSubLoops().empty()) {
          llvm::outs() << "    Contains " << L->getSubLoops().size()
                       << " nested loop(s).\n";
          // You can recurse or iterate L->getSubLoops() to explore inner loops
        }

        // Get back-edges
        llvm::outs() << "    Back-edges from:\n";
        for (llvm::BasicBlock *Latch : L->getLoopLatches()) {
          llvm::outs() << "      - " << Latch->getName() << "\n";
        }

        // Get exiting blocks (blocks inside the loop that have an edge to
        // outside)
        llvm::outs() << "    Exiting Blocks:\n";
        llvm::SmallVector<llvm::BasicBlock *, 8> ExitingBlocks;
        L->getExitingBlocks(ExitingBlocks);
        for (llvm::BasicBlock *BB : ExitingBlocks) {
          llvm::outs() << "      - " << BB->getName() << "\n";
        }
      }
    }

    // Return PreservedAnalyses::all() if your pass only analyzes and doesn't
    // modify IR
    return llvm::PreservedAnalyses::all();
  }
};

} // end anonymous namespace

// 3. Register your pass (for use with opt)
extern "C" LLVM_ATTRIBUTE_WEAK ::llvm::PassPluginLibraryInfo
llvmGetPassPluginInfo() {
  return {LLVM_PLUGIN_API_VERSION, "MyLoopAnalysisPass", "v0.1",
          [](llvm::PassBuilder &PB) {
            PB.registerPipelineParsingCallback(
                [](llvm::StringRef Name, llvm::FunctionPassManager &FPM,
                   llvm::ArrayRef<llvm::PassBuilder::PipelineElement>) {
                  if (Name == "my-loop-analysis") {
                    FPM.addPass(MyLoopAnalysisPass());
                    return true;
                  }
                  return false;
                });
          }};
}