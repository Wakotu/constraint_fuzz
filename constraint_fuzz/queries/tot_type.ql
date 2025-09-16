import cpp

// from Struct t, Field f
// where f.getDeclaringType() = t
// select t.getQualifiedName() as sturct_name, f.getName() as field_name, f.getType() as field_type
from Class c
select c.getName() as struct_name, c.getLocation() as name_loc
