import cpp
import modules.mod

from ForStmt forStmt
where "InitFor" = getForType(forStmt)
select forStmt.getLocation() as for_stmt_location,
  forStmt.getInitialization().getLocation() as init_location,
  forStmt.getEnclosingFunction().getName() as function,
  forStmt.getFile().getAbsolutePath() as file_path
