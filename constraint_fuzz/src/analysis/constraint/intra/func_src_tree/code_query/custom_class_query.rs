use color_eyre::eyre::Result;
use once_cell::sync::Lazy;
use std::collections::VecDeque;

use tree_sitter::{Language, Node, Parser};

// Use the safe exported constant from tree-sitter-c
use eyre::bail;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};
use tree_sitter_c::LANGUAGE;

use my_macros::{EquivByLoc, EquivByName};
use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::CodeQLRunner,
    stmts::{LocParseError, LocTypeParseError, QLLoc},
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
    pub fn to_entry_pair(
        &self,
    ) -> std::result::Result<(ClassEntry, FieldEntry), LocTypeParseError> {
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

#[derive(Debug, Clone)]
pub enum VarTypeVariant {
    Primitive,
    CustomClass,
    Compound,
}

#[derive(Debug, Clone)]
pub struct VarTypeSegment {
    pub start_idx: usize,
    pub end_idx: usize,
}

#[derive(Debug, Clone)]
pub struct VarType {
    pub name: String,
    /// None for primitive types, Some for user-defined classes
    pub loc: Option<QLLoc>,
    pub variants: VarTypeVariant,
    pub seg_vec: Vec<VarTypeSegment>,
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

pub struct VarSegIter<'a> {
    var_type: &'a VarType,
    curr_idx: usize,
}

impl<'a> Iterator for VarSegIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr_idx >= self.var_type.seg_vec.len() {
            return None;
        }
        let seg = &self.var_type.seg_vec[self.curr_idx];
        self.curr_idx += 1;
        Some(&self.var_type.name[seg.start_idx..seg.end_idx])
    }
}

static CUSTOM_TYPE_BLACK_LIST: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut set = HashSet::new();
    // Built-in Types
    set.insert("void");
    set.insert("char");
    set.insert("short");
    set.insert("int");
    set.insert("long");
    set.insert("float");
    set.insert("double");
    set.insert("signed");
    set.insert("unsigned");
    set.insert("bool");
    set.insert("wchar_t");
    set.insert("size_t");

    // Common Specifiers
    set.insert("const");
    set.insert("volatile");
    set.insert("static");
    set.insert("extern");
    set.insert("register");
    set.insert("mutable");
    set.insert("inline");
    set.insert("virtual");
    set.insert("friend");
    set.insert("typedef");

    // Common Derived Type Modifiers
    set.insert("*");
    set.insert("[");
    set.insert("]");
    set.insert("(");
    set.insert(")");

    set
});

impl VarType {
    pub fn is_cc_seg(type_seg: &str) -> bool {
        // check type seg not in black list
        CUSTOM_TYPE_BLACK_LIST.contains(type_seg) == false
    }

    pub fn iter_type_seg(&self) -> VarSegIter {
        VarSegIter {
            var_type: self,
            curr_idx: 0,
        }
    }

    pub fn is_void(&self) -> bool {
        self.name == "void" && self.loc.is_none()
    }

    fn get_type_seg_vec(type_name: &str) -> Vec<VarTypeSegment> {
        let mut parser = Parser::new();
        parser.set_language(&LANGUAGE.into()).unwrap();

        let tree = parser.parse(type_name, None).unwrap();
        let root = tree.root_node();

        let mut q: VecDeque<Node> = VecDeque::new();
        q.push_back(root);

        let mut seg_vec: Vec<VarTypeSegment> = vec![];

        while !q.is_empty() {
            let node = q.pop_front().unwrap();
            if node.child_count() == 0 {
                seg_vec.push(VarTypeSegment {
                    start_idx: node.start_byte(),
                    end_idx: node.end_byte(),
                });
            }

            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                q.push_back(child);
            }
        }
        seg_vec
    }

    pub fn new(name: &str, loc: &str) -> std::result::Result<Self, LocTypeParseError> {
        let mut var_variant = VarTypeVariant::CustomClass;
        let mut seg_vec = vec![];
        let loc = match QLLoc::from_str(loc) {
            Ok(l) => Some(l),
            Err(e) => match e {
                // Consider ValueError as primitive type circumstance
                LocParseError::ValueErr(_) => None,
                LocParseError::FormatErr(msg) => {
                    return Err(LocTypeParseError::FormatErr(msg));
                }
                LocParseError::ZeroErr => {
                    seg_vec = Self::get_type_seg_vec(name);
                    if seg_vec.len() > 1 {
                        var_variant = VarTypeVariant::Compound;
                    } else {
                        var_variant = VarTypeVariant::Primitive;
                    }
                    None
                }
            },
        };
        Ok(Self {
            name: name.to_owned(),
            loc,
            variants: var_variant,
            seg_vec,
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
    ) -> std::result::Result<Self, LocTypeParseError> {
        let var_type = VarType::new(field_type_name, field_type_loc)?;
        Ok(Self {
            field_name: field_name.to_owned(),
            field_type: var_type,
        })
    }

    pub fn get_type_loc(&self) -> Option<&QLLoc> {
        self.field_type.get_loc()
    }

    pub fn get_type(&self) -> &VarType {
        &self.field_type
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

#[derive(EquivByName)]
pub struct CustomClass {
    loc: QLLoc,
    name: String,
    variants: CustomClassVariant,
}

// pub type CustomClassSet = HashSet<CustomClass>;

pub struct CustomClassSet {
    data: HashSet<CustomClass>,
}

impl CustomClassSet {
    pub fn new() -> Self {
        Self {
            data: HashSet::new(),
        }
    }

    pub fn insert(&mut self, cc: CustomClass) -> bool {
        self.data.insert(cc)
    }

    pub fn contains(&self, cc: &CustomClass) -> bool {
        self.data.contains(cc)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CustomClass> {
        self.data.iter()
    }

    fn search_by_name(&self, name: &str) -> Vec<&CustomClass> {
        let cc = match self.data.get(name) {
            None => return vec![],
            Some(cc) => cc,
        };

        match &cc.variants {
            CustomClassVariant::Enum { .. } => vec![cc],
            CustomClassVariant::Struct { fields } => {
                let mut cc_vec = vec![cc];
                for field in fields.iter() {
                    let sub_cc_vec = self.search(&field.get_type());
                    cc_vec.extend(sub_cc_vec);
                }
                cc_vec
            }
        }
    }

    pub fn search(&self, var_type: &VarType) -> Vec<&CustomClass> {
        match var_type.variants {
            VarTypeVariant::Primitive => vec![],
            VarTypeVariant::Compound => {
                let mut cc_vec = vec![];
                // collect all valid type seg
                for type_seg in var_type.iter_type_seg() {
                    if VarType::is_cc_seg(type_seg) {
                        let seg_cc_vec = self.search_by_name(type_seg);
                        cc_vec.extend(seg_cc_vec);
                    }
                }
                cc_vec
            }
            VarTypeVariant::CustomClass => self.search_by_name(&var_type.name),
        }
    }
}

impl CodeQLRunner {
    pub fn get_custom_class_set(&self) -> Result<CustomClassSet> {
        let sf_rec_vec: Vec<StructFieldRec> = self.run_query_and_parse(STRUCT_FIELD_QUERY)?;
        let enum_rec_vec: Vec<EnumRec> = self.run_query_and_parse(ENUM_QUERY)?;
        let mut cc_set: CustomClassSet = CustomClassSet::new();

        let mut struct_map: HashMap<ClassEntry, Vec<FieldEntry>> = HashMap::new();
        for rec in sf_rec_vec.into_iter() {
            let (class_entry, field_entry) = match rec.to_entry_pair() {
                Ok(p) => p,
                Err(e) => match e {
                    LocTypeParseError::FormatErr(msg) => {
                        bail!(
                            "Error parsing location in struct field query result: {}",
                            msg
                        );
                    }
                    LocTypeParseError::ValueErr(_) => {
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
                    LocParseError::ValueErr(_) | LocParseError::ZeroErr => {
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
