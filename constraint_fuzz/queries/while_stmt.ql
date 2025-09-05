import cpp
import modules.mod

class MyWhileStmt extends Stmt {
  MyWhileStmt() {
    this instanceof WhileStmt or
    this instanceof DoStmt
  }

  Expr getCondition() {
    result = this.(WhileStmt).getCondition() or
    result = this.(DoStmt).getCondition()
  }

  Stmt getStmt() {
    result = this.(WhileStmt).getStmt() or
    result = this.(DoStmt).getStmt()
  }

  string getType() { if this instanceof WhileStmt then result = "while" else result = "do-while" }
}

from MyWhileStmt whileStmt, Stmt stmt
where whileStmt.getStmt() = stmt
select whileStmt.getLocation() as while_stmt_location, whileStmt.getType() as while_type,
  whileStmt.getCondition().getLocation() as condition_location, stmt.getLocation() as body_location,
  getStmtType(stmt) as body_type, whileStmt.getEnclosingFunction().getName() as function_name,
  whileStmt.getFile().getAbsolutePath() as file_path
