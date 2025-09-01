import cpp
import modules.mod

from SwitchStmt switchStmt, SwitchCase switchCase, Stmt stmt
where switchCase.getSwitchStmt() = switchStmt and switchCase.getAStmt() = stmt
//   and switchStmt.getEnclosingFunction().getName() = "blend_a64_mask_avx2"
select switchStmt.getLocation() as switch_stmt_location,
  switchStmt.getExpr().getLocation() as expr_location, switchCase.getLocation() as case_location,
  stmt.getLocation() as case_stmt_location, getStmtType(stmt) as stmt_type,
  switchStmt.getEnclosingFunction().getName(), switchStmt.getFile().getAbsolutePath() as file_path
