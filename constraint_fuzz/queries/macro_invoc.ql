import cpp

from MacroInvocation mi
select mi, mi.getMacro().getName() as macro_name, mi.getLocation() as usage_loc
