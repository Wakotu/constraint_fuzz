import cpp

from FunctionCall call
select call.getTarget().getName() as func_name, call.getLocation() as loc
