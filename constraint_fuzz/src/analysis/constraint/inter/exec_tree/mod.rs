use std::{collections::HashMap, fs::read_dir, path::Path};

use color_eyre::eyre::Result;

use crate::analysis::constraint::inter::exec_tree::thread_tree::{ThreadExecTree, THCPMAPPING};

pub mod action;
pub mod analyze;
pub mod thread_tree;

pub struct ExecForest {
    thcp_mapping: THCPMAPPING,
    thread_tree_list: Vec<ThreadExecTree>,
    main_idx: usize,
}

impl ExecForest {
    fn is_main_guard<P: AsRef<Path>>(guard_fpath: P) -> Result<bool> {
        const MAIN_SUFFIX: &str = "_main";
        let fname = guard_fpath
            .as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                eyre::eyre!(
                    "Faile to get file name from path {:?}",
                    guard_fpath.as_ref()
                )
            })?;
        Ok(fname.ends_with(MAIN_SUFFIX))
    }

    pub fn from_guard_dir<P: AsRef<Path>>(guard_dir: P) -> Result<Self> {
        assert!(guard_dir.as_ref().is_dir());
        let mut tree_list = vec![];
        let mut thcp_mapping = HashMap::new();
        let mut idx = 0;

        for ent_res in read_dir(guard_dir)? {
            let ent = ent_res?;
            let guard_fpath = ent.path();

            if Self::is_main_guard(&guard_fpath)? {
                idx = tree_list.len();
            }

            let (tree, sub_mapping) = ThreadExecTree::from_guard_file(&guard_fpath)?;
            tree_list.push(tree);
            thcp_mapping.extend(sub_mapping);
        }
        Ok(Self {
            thcp_mapping,
            thread_tree_list: tree_list,
            main_idx: idx,
        })
    }

    pub fn iter_trees(&self) -> impl Iterator<Item = &ThreadExecTree> {
        self.thread_tree_list.iter()
    }

    pub fn len(&self) -> usize {
        self.thread_tree_list.len()
    }
}
