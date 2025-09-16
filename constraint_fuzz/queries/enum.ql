import cpp

from Enum e, EnumConstant ec
where e.getAnEnumConstant() = ec
select e.getName() as enum_name, e.getLocation() as enum_loc, ec.getName() as enumerator_name,
  ec.getValue() as enumerator_value
