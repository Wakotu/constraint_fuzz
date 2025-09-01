import cpp

string getForType(ForStmt forStmt) {
  if exists(Stmt stmt | stmt = forStmt.getInitialization())
  then result = "InitFor"
  else result = "NoInitFor"
}

string getIfType(IfStmt ifStmt) { if ifStmt.hasElse() then result = "If-Else" else result = "If" }

string getStmtType(Stmt stmt) {
  //   result = "IfStmt" and stmt instanceof IfStmt
  //   or
  //   result = "ForStmt" and stmt instanceof ForStmt
  //   or
  //   result = "WhileStmt" and stmt instanceof WhileStmt
  //   or
  //   result = "SwitchStmt" and stmt instanceof SwitchStmt
  //   or
  //   result = "DoStmt" and stmt instanceof DoStmt
  if stmt instanceof IfStmt
  then result = "IfStmt"
  else
    if stmt instanceof ForStmt
    then result = "ForStmt"
    else
      if stmt instanceof WhileStmt
      then result = "WhileStmt"
      else
        if stmt instanceof SwitchStmt
        then result = "SwitchStmt"
        else
          if stmt instanceof DoStmt
          then result = "DoStmt"
          else
            if stmt instanceof BlockStmt // Added to distinguish nested blocks from other statements.
            then result = "BlockStmt"
            else
              if stmt instanceof DeclStmt
              then result = "DeclStmt"
              else
                if stmt instanceof ExprStmt
                then result = "ExprStmt"
                else
                  if stmt instanceof ReturnStmt
                  then result = "ReturnStmt"
                  else result = "OtherStmt"
}
