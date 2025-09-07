use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use color_eyre::eyre::Result;
use eyre::bail;
use serde::Deserialize;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::{CodeQLRunner, FileFuncTable},
    stmts::{BlockStmt, ChildEntry, LocParseError},
};

const BLOCK_QUERY_NAME: &str = "block_stmt.ql";

#[derive(Debug, Deserialize)]
struct BlockRecord {
    block_loc: String,
    block_type: String,
    child_stmt_loc: String,
    child_stmt_type: String,
    func_name: String,
    file_path: String,
}

impl BlockRecord {
    pub fn to_entry(&self) -> std::result::Result<BlockEntry, LocParseError> {
        let block = BlockStmt::from_loc_and_type(&self.block_loc, &self.block_type)?;
        let child = ChildEntry::from_loc_and_type(&self.child_stmt_loc, &self.child_stmt_type)?;
        Ok((block, child))
    }
}

// corresponds to a function
pub struct BlockMap {
    data: HashMap<BlockStmt, HashSet<ChildEntry>>,
}

impl Default for BlockMap {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockMap {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, block: BlockStmt, child: ChildEntry) {
        self.data
            .entry(block)
            .or_insert_with(HashSet::new)
            .insert(child);
    }

    pub fn get_root_entry(&self) -> Result<Option<ChildEntry>> {
        let mut res = None;
        for block_stmt in self.data.keys() {
            if block_stmt.is_function_block() {
                if !res.is_none() {
                    bail!(
                        "Warning: Multiple function blocks found in BlockMap. Existing root: {:?}, New root: {:?}",
                        res,
                        block_stmt
                    );
                }
                res = Some(ChildEntry::from_block_stmt(block_stmt));
            }
        }
        Ok(res)
    }
}

pub type BlockEntry = (BlockStmt, ChildEntry);
pub type BlockPool = FileFuncTable<BlockMap>;

impl CodeQLRunner {
    fn get_records(&self) -> Result<Vec<BlockRecord>> {
        let records: Vec<BlockRecord> = self.run_query_and_parse(BLOCK_QUERY_NAME)?;
        Ok(records)
    }

    pub fn get_block_pool(&self) -> Result<BlockPool> {
        let records = self.get_records()?;

        let mut block_pool: FileFuncTable<BlockMap> = FileFuncTable::new();
        // let mut block_map: BlockMap = BlockMap::new();
        for record in records {
            let block_map = block_pool.get_value_mut(&record.file_path, &record.func_name);
            let entry_res = record.to_entry();
            match entry_res {
                Ok((block, child)) => {
                    block_map.insert(block, child);
                }
                Err(e) => {
                    match e {
                        LocParseError::FormatErr(msg) => {
                            bail!("Error: Failed to parse record due to format error: {}. Record: {:?}", msg, record);
                        }
                        LocParseError::ValueErr(msg) => {
                            log::warn!(
                                "Warning: Skipping record due to value error: {}. Record: {:?}",
                                msg,
                                record
                            );
                        }
                    }
                }
            }
        }

        Ok(block_pool)
    }
}
