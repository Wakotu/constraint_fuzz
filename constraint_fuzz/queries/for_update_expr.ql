import cpp

from ForStmt forStmt
select forStmt.getLocation() as for_stmt_location,
  forStmt.getUpdate().getLocation() as update_location,
  forStmt.getEnclosingFunction().getName() as function,
  forStmt.getFile().getAbsolutePath() as file_path
