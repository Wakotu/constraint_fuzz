import cpp

from Variable v, Function f
where v.getParentScope() = f
select v.getName() as var_name, v.getLocation() as var_loc, v.getType().getName() as var_type_name,
  v.getType().getLocation() as var_type_loc, f.getName() as func_name
