import cpp

from Function func
select func.getName() as func_name, func.getLocation() as name_loc,
  func.getBlock().getLocation() as body_loc
