use color_eyre::eyre::Result;
use eyre::bail;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

use crate::analysis::constraint::intra::func_src_tree::stmts::{LocParseError, QLLoc, StmtType};
use crate::deopt::utils::buffer_read_to_bytes;
use crate::deopt::Deopt;

pub mod block_query;
pub mod for_stmt;
pub mod if_query;
pub mod switch_query;
pub mod while_query;

impl Deopt {
    pub fn get_codeql_db_dir(&self) -> Result<PathBuf> {
        let lib_build_dir = self.get_library_build_dir()?;
        let res = lib_build_dir.join("codeql_db");
        Ok(res)
    }
}

struct CodeQLRunner {
    deopt: Deopt,
}

impl CodeQLRunner {
    pub fn new(deopt: Deopt) -> Self {
        Self { deopt }
    }

    pub fn get_query_path(&self, query_name: &str) -> Result<PathBuf> {
        assert!(query_name.ends_with(".ql"), "Query name must end with .ql");
        let root = Deopt::get_crate_dir()?;
        let mut query_path = PathBuf::from(root);
        query_path.push("queries");
        query_path.push(query_name);
        Ok(query_path)
    }

    fn get_csv_path(&self, query_name: &str) -> Result<PathBuf> {
        let query_path = self.get_query_path(query_name)?;
        let csv_dir = query_path.parent().ok_or_else(|| {
            eyre::eyre!(
                "Failed to get parent directory of query path: {:?}",
                query_path
            )
        })?;
        let csv_path = csv_dir.join(format!("{}.csv", query_name.trim_end_matches(".ql")));
        Ok(csv_path)
    }

    pub fn run_query(&self, query_name: &str) -> Result<Vec<u8>> {
        let query_path = self.get_query_path(query_name)?;
        let db_dir = self.deopt.get_codeql_db_dir()?;
        let bqrs_file = NamedTempFile::new()?;
        let bqrs_path = bqrs_file.path().as_os_str();
        let csv_path = self.get_csv_path(query_name)?;

        if !csv_path.is_file() {
            // run the query
            let status = Command::new("codeql")
                .arg("query")
                .arg("run")
                .arg(query_path)
                .arg("--database")
                .arg(db_dir)
                .arg("--output")
                .arg(bqrs_path)
                .status()?;
            assert!(status.success(), "CodeQL query execution failed");

            // decode
            let status = Command::new("codeql")
                .arg("bqrs")
                .arg("decode")
                .arg(bqrs_path)
                .arg("--format=csv")
                .arg("--output")
                .arg(&csv_path)
                .status()?;
            assert!(status.success(), "CodeQL bqrs decode failed");
        }

        let bytes = buffer_read_to_bytes(&csv_path)?;
        Ok(bytes)
    }

    pub fn csv_parse<T>(csv_data: &Vec<u8>) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut reader = csv::Reader::from_reader(csv_data.as_slice());
        let mut results = Vec::new();
        for result in reader.deserialize() {
            let record: T = result?;
            results.push(record);
        }
        Ok(results)
    }

    pub fn run_query_and_parse<T>(&self, query_name: &str) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let csv_data = self.run_query(query_name)?;
        let results = Self::csv_parse(&csv_data)?;
        Ok(results)
    }
}

pub struct FileFuncTable<V> {
    data: HashMap<PathBuf, HashMap<String, V>>,
}

impl<V: Default> FileFuncTable<V> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn get_value_mut(&mut self, file_path: &str, func_name: &str) -> &mut V {
        let file_path = PathBuf::from(file_path);
        self.data
            .entry(file_path)
            .or_insert_with(HashMap::new)
            .entry(func_name.to_owned())
            .or_insert_with(V::default)
    }
}

// tests
#[cfg(test)]
mod tests {
    use crate::setup_test_run_entry;
    use color_eyre::eyre::Result;

    use super::*;

    #[test]
    fn test_run_query() -> Result<()> {
        setup_test_run_entry("libaom", true)?;
        let deopt = Deopt::new("libaom")?;
        let runner = CodeQLRunner::new(deopt);

        let bytes = runner.run_query("block_stmt.ql")?;
        log::debug!("Query output:\n{}", String::from_utf8_lossy(&bytes));
        Ok(())
    }
}
