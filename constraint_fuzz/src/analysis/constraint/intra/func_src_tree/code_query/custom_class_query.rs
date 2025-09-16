use color_eyre::eyre::Result;
use eyre::bail;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use my_macros::EquivByLoc;
use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::CodeQLRunner,
    stmts::{LocParseError, QLLoc},
};

const STRUCT_FIELD_QUERY: &str = "struct_field.ql";
const ENUM_QUERY: &str = "enum.ql";

#[derive(Deserialize)]
pub struct StructFieldRec {
    struct_name: String,
    struct_loc: String,
    field_name: String,
    field_type_name: String,
    field_type_loc: String,
}

impl StructFieldRec {
    pub fn to_entry_pair(&self) -> std::result::Result<(ClassEntry, FieldEntry), LocParseError> {
        let class_entry = ClassEntry::new(&self.struct_loc, &self.struct_name)?;
        let field_entry = FieldEntry::new(
            &self.field_name,
            &self.field_type_name,
            &self.field_type_loc,
        )?;
        Ok((class_entry, field_entry))
    }
}

#[derive(Deserialize)]
pub struct EnumRec {
    enum_name: String,
    enum_loc: String,
    constant_name: String,
    constant_value: String,
}

impl EnumRec {
    pub fn to_entry_pair(&self) -> std::result::Result<(ClassEntry, EnumConstant), LocParseError> {
        let class_entry = ClassEntry::new(&self.enum_loc, &self.enum_name)?;
        // error in enum value parse is not allowed
        let enum_constant = EnumConstant::new(&self.constant_name, &self.constant_value)
            .map_err(|e| LocParseError::FormatErr(e.to_string()))?;
        Ok((class_entry, enum_constant))
    }
}

pub struct VarType {
    name: String,
    /// None for primitive types, Some for user-defined classes
    loc: Option<QLLoc>,
}

impl PartialEq for VarType {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.loc == other.loc
    }
}

impl Eq for VarType {}

impl PartialOrd for VarType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.loc.partial_cmp(&other.loc) {
            Some(std::cmp::Ordering::Equal) => self.name.partial_cmp(&other.name),
            ord => ord,
        }
    }
}

impl Ord for VarType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.loc.cmp(&other.loc) {
            std::cmp::Ordering::Equal => self.name.cmp(&other.name),
            ord => ord,
        }
    }
}

impl VarType {
    pub fn new(name: &str, loc: &str) -> std::result::Result<Self, LocParseError> {
        let loc = match QLLoc::from_str(loc) {
            Ok(l) => Some(l),
            Err(e) => match e {
                // Consider ValueError as primitive type circumstance
                LocParseError::ValueErr(_) => None,
                LocParseError::FormatErr(msg) => {
                    return Err(LocParseError::FormatErr(msg));
                }
            },
        };
        Ok(Self {
            name: name.to_owned(),
            loc,
        })
    }

    pub fn is_primitive(&self) -> bool {
        self.loc.is_none()
    }

    pub fn is_class(&self) -> bool {
        self.loc.is_some()
    }

    pub fn get_loc(&self) -> Option<&QLLoc> {
        self.loc.as_ref()
    }
}

pub struct FieldEntry {
    field_name: String,
    field_type: VarType,
}

impl FieldEntry {
    pub fn new(
        field_name: &str,
        field_type_name: &str,
        field_type_loc: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let var_type = VarType::new(field_type_name, field_type_loc)?;
        Ok(Self {
            field_name: field_name.to_owned(),
            field_type: var_type,
        })
    }

    pub fn get_type_loc(&self) -> Option<&QLLoc> {
        self.field_type.get_loc()
    }
}

impl PartialEq for FieldEntry {
    fn eq(&self, other: &Self) -> bool {
        self.field_name == other.field_name && self.field_type == other.field_type
    }
}

impl Eq for FieldEntry {}

impl PartialOrd for FieldEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.field_type.partial_cmp(&other.field_type) {
            Some(std::cmp::Ordering::Equal) => self.field_name.partial_cmp(&other.field_name),
            ord => ord,
        }
    }
}

impl Ord for FieldEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.field_type.cmp(&other.field_type) {
            std::cmp::Ordering::Equal => self.field_name.cmp(&other.field_name),
            ord => ord,
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct EnumConstant {
    name: String,
    value: i64,
}

impl EnumConstant {
    pub fn new(name: &str, val_str: &str) -> Result<Self> {
        let val: i64 = val_str.parse()?;
        Ok(Self {
            name: name.to_owned(),
            value: val,
        })
    }
}

impl PartialOrd for EnumConstant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.value.partial_cmp(&other.value) {
            Some(std::cmp::Ordering::Equal) => self.name.partial_cmp(&other.name),
            ord => ord,
        }
    }
}

impl Ord for EnumConstant {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.value.cmp(&other.value) {
            std::cmp::Ordering::Equal => self.name.cmp(&other.name),
            ord => ord,
        }
    }
}

pub enum CustomClassVariant {
    Struct { fields: Vec<FieldEntry> },
    Enum { constants: Vec<EnumConstant> },
}

#[derive(EquivByLoc)]
pub struct ClassEntry {
    loc: QLLoc,
    name: String,
}

impl ClassEntry {
    pub fn new(loc: &str, name: &str) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc)?;
        Ok(Self {
            loc,
            name: name.to_owned(),
        })
    }
}

#[derive(EquivByLoc)]
pub struct CustomClass {
    loc: QLLoc,
    name: String,
    variants: CustomClassVariant,
}

impl Borrow<QLLoc> for CustomClass {
    fn borrow(&self) -> &QLLoc {
        &self.loc
    }
}

pub type CustomClassSet = HashSet<CustomClass>;

impl CodeQLRunner {
    pub fn get_custom_class_set(&self) -> Result<CustomClassSet> {
        let sf_rec_vec: Vec<StructFieldRec> = self.run_query_and_parse(STRUCT_FIELD_QUERY)?;
        let enum_rec_vec: Vec<EnumRec> = self.run_query_and_parse(ENUM_QUERY)?;
        let mut cc_set: CustomClassSet = HashSet::new();

        let mut struct_map: HashMap<ClassEntry, Vec<FieldEntry>> = HashMap::new();
        for rec in sf_rec_vec.into_iter() {
            let (class_entry, field_entry) = match rec.to_entry_pair() {
                Ok(p) => p,
                Err(e) => match e {
                    LocParseError::FormatErr(msg) => {
                        bail!(
                            "Error parsing location in struct field query result: {}",
                            msg
                        );
                    }
                    LocParseError::ValueErr(_) => {
                        continue;
                    }
                },
            };
            struct_map
                .entry(class_entry)
                .or_insert_with(Vec::new)
                .push(field_entry);
        }

        let mut enum_map: HashMap<ClassEntry, Vec<EnumConstant>> = HashMap::new();
        for rec in enum_rec_vec.into_iter() {
            let (class_entry, enum_constant) = match rec.to_entry_pair() {
                Ok(p) => p,
                Err(e) => match e {
                    LocParseError::FormatErr(msg) => {
                        bail!("Error parsing location in enum query result: {}", msg);
                    }
                    LocParseError::ValueErr(_) => {
                        continue;
                    }
                },
            };
            enum_map
                .entry(class_entry)
                .or_insert_with(Vec::new)
                .push(enum_constant);
        }

        for (class_entry, mut fields) in struct_map.into_iter() {
            fields.sort();
            let cc = CustomClass {
                loc: class_entry.loc.clone(),
                name: class_entry.name,
                variants: CustomClassVariant::Struct { fields },
            };
            cc_set.insert(cc);
        }
        for (class_entry, constants) in enum_map.into_iter() {
            let cc = CustomClass {
                loc: class_entry.loc.clone(),
                name: class_entry.name,
                variants: CustomClassVariant::Enum { constants },
            };
            cc_set.insert(cc);
        }
        Ok(cc_set)
    }
}
