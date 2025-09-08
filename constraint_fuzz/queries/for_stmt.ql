import cpp
import modules.mod

from ForStmt forStmt, Stmt bodyStmt
where forStmt.getStmt() = bodyStmt
select forStmt.getLocation() as for_stmt_location, bodyStmt.getLocation() as body_location,
  getStmtType(bodyStmt) as body_stmt_type,
  forStmt.getEnclosingFunction().getName() as function_name,
  forStmt.getFile().getAbsolutePath() as file_path
