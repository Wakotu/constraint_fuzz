import cpp
import modules.mod

string getBlockTypeString(BlockStmt b) {
  if exists(IfStmt ifStmt | b = ifStmt.getThen())
  then result = "IfBlock"
  else
    if exists(IfStmt ifStmt | b = ifStmt.getElse())
    then result = "ElseBlock"
    else
      if exists(SwitchStmt s | b = s.getStmt())
      then result = "SwitchBlock"
      else
        if exists(ForStmt s | b = s.getStmt())
        then result = "ForBlock"
        else
          if exists(WhileStmt s | b = s.getStmt())
          then result = "WhileBlock"
          else
            if exists(DoStmt s | b = s.getStmt())
            then result = "DoBlock"
            else
              if exists(Function f | b = f.getBlock())
              then result = "FunctionBlock"
              else result = "ScopedBlock" // A generic, nested block.
}

from BlockStmt block, Stmt childStmt, string blockType
where
  getBlockTypeString(block) = blockType and
  blockType != "SwitchBlock" and
  childStmt.getParent() = block
// and block.getEnclosingFunction().getName() = "set_error"
select block.getLocation() as block_location, blockType,
  childStmt.getLocation() as child_stmt_location, getStmtType(childStmt),
  block.getEnclosingFunction().getName(), block.getFile().getAbsolutePath() as file_path
