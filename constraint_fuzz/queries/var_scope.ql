import cpp

from Variable v, Element e
where v.getParentScope() = e
select v.getName() as var_name, v.getType().getName() as var_type, v.getLocation() as var_loc,
  e.getLocation() as scope_loc
