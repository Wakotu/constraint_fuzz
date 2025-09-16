import cpp

// from Struct t, Field f
// where f.getDeclaringType() = t
// select t.getQualifiedName() as sturct_name, f.getName() as field_name, f.getType() as field_type
from Class t
where exists(Class s | t.getDeclaringType() = s)
select t.getName() as struct_name, t.getLocation() as name_loc
