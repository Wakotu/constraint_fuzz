import cpp

from Struct s, Field f
where s.getAField() = f
select s.getName() as struct_name, s.getLocation() as struct_loc, f.getName() as field_name,
  f.getType().getName() as field_type_name, f.getType().getLocation() as field_type_loc
