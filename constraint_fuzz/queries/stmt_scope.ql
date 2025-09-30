import cpp

from Variable v, Stmt s
where v.getParentScope() = s
select v.getName() as var_name, v.getLocation() as var_loc, v.getType().getName() as var_type_name,
  v.getType().getLocation() as var_type_loc, s.getLocation() as stmt_loc
