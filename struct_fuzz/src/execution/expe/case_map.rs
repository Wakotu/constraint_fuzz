use color_eyre::eyre::Result;
use eyre::bail;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
/**
This file implements mapping between case path and record names
*/
use std::sync::LazyLock;
use std::sync::RwLock;

use crate::program::gadget::func_gadget;

// record name -> case path
pub type RecordName = String;

static RECORD_NAME_TO_CASE_PATH: LazyLock<RwLock<HashMap<RecordName, PathBuf>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// md5 hashing from path to record name
pub fn get_rec_name_from_case_path(case_path: &Path) -> Result<RecordName> {
    // generates rec name
    let path_str = case_path
        .to_str()
        .unwrap_or_else(|| panic!("case path is not valid utf-8: {:?}", case_path));
    let dig = md5::compute(path_str);
    let rec_name = format!("{:x}", dig);

    // save reverse mapping
    let mut map = RECORD_NAME_TO_CASE_PATH.write().unwrap();
    if map.contains_key(&rec_name) {
        bail!("Unexpected: record name already exists: {}", rec_name);
    }
    map.insert(rec_name.clone(), case_path.to_path_buf());
    Ok(rec_name)
}

// get case path from record name
pub fn get_case_path_from_rec_name(rec_name: &RecordName) -> Result<PathBuf> {
    let map = RECORD_NAME_TO_CASE_PATH.read().unwrap();
    if let Some(path) = map.get(rec_name) {
        return Ok(path.clone());
    }
    bail!(
        "Unexpected: case path not found for record name: {}",
        rec_name
    )
}
