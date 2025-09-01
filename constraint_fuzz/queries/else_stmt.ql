import cpp
import modules.mod

from IfStmt ifStmt, Stmt elseStmt
where
  ifStmt.hasElse() and
  ifStmt.getElse() = elseStmt
select ifStmt.getLocation() as if_stmt_location, elseStmt.getLocation() as else_stmt_location,
  getStmtType(elseStmt) as else_stmt_type, ifStmt.getEnclosingFunction().getName() as function,
  ifStmt.getFile().getAbsolutePath() as file_path
