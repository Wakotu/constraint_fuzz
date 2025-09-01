import cpp
import modules.mod

from IfStmt ifStmt, Stmt thenStmt
where ifStmt.getThen() = thenStmt
select ifStmt.getLocation() as if_stmt_location, getIfType(ifStmt),
  ifStmt.getCondition().getLocation() as condition_location,
  thenStmt.getLocation() as then_stmt_location, getStmtType(thenStmt) as then_stmt_type,
  ifStmt.getEnclosingFunction().getName() as function,
  ifStmt.getFile().getAbsolutePath() as file_path
