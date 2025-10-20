import cpp

from LabelStmt s
where s.isNamed()
select s.getLocation() as label_loc, s.getName() as label_name
