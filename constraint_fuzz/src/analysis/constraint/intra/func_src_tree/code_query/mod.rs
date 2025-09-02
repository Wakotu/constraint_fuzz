use color_eyre::eyre::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

use crate::analysis::constraint::intra::func_src_tree::{LocParseError, QLLoc, StmtType};
use crate::deopt::Deopt;

pub mod block_query;

impl Deopt {
    pub fn get_codeql_db_dir(&self) -> Result<PathBuf> {
        let lib_build_dir = self.get_library_build_dir()?;
        let res = lib_build_dir.join("codeql_db");
        Ok(res)
    }
}

#[derive(PartialEq, Eq, Hash)]
struct ChildEnty {
    loc: QLLoc,
    stmt_type: StmtType,
}

impl ChildEnty {
    pub fn from_loc_and_type(
        loc_str: &str,
        type_str: &str,
    ) -> std::result::Result<Self, LocParseError> {
        let loc = QLLoc::from_str(loc_str)?;
        let stmt_type = StmtType::from_str(type_str);
        Ok(Self { loc, stmt_type })
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

    pub fn run_query<P: AsRef<Path>>(&self, query_path: P) -> Result<Vec<u8>> {
        let db_dir = self.deopt.get_codeql_db_dir()?;
        let bqrs_file = NamedTempFile::new()?;
        let bqrs_path = bqrs_file.path().as_os_str();

        // run the query
        let status = Command::new("codeql")
            .arg("query")
            .arg("run")
            .arg(query_path.as_ref())
            .arg("--database")
            .arg(db_dir)
            .arg("--output")
            .arg(bqrs_path)
            .status()?;

        assert!(status.success(), "CodeQL query execution failed");

        // decode
        let output = Command::new("codeql")
            .arg("bqrs")
            .arg("decode")
            .arg(bqrs_path)
            .arg("--format=csv")
            .output()?;
        Ok(output.stdout)
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
        let query_path = self.get_query_path(query_name)?;
        let csv_data = self.run_query(query_path)?;
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

    pub fn get_value_mut(&mut self, file_path: PathBuf, func_name: &str) -> &mut V {
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

        let bytes = runner.run_query("/struct_fuzz/constraint_fuzz/queries/block_stmt.ql")?;
        log::debug!("Query output:\n{}", String::from_utf8_lossy(&bytes));
        Ok(())
    }
}
