import cpp

from Function func
select func.getName() as func_name, func.getLocation() as name_loc,
  func.getBlock().getLocation() as body_loc, func.getType().getName() as return_type_name,
  func.getType().getLocation() as return_type_loc
