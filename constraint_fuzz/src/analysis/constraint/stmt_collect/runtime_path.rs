use std::collections::HashMap;

use crate::analysis::constraint::stmt_collect::ProcessUnit;

pub type Tid = usize;

pub struct PathRoot {
    pub tid: Tid,
    pub len: usize,
}

pub struct ThreadRuntimePath {
    tid: Tid,
    root: Option<PathRoot>,
    pu_vec: Vec<ProcessUnit>,
}

impl ThreadRuntimePath {
    pub fn main_path_construct(tid: Tid, pu_vec: Vec<ProcessUnit>) -> Self {
        ThreadRuntimePath {
            tid,
            root: None,
            pu_vec,
        }
    }

    pub fn thread_path_construct(tid: Tid, root: PathRoot, pu_vec: Vec<ProcessUnit>) -> Self {
        ThreadRuntimePath {
            tid,
            root: Some(root),
            pu_vec,
        }
    }
}

pub struct RuntimePathTree {
    data: HashMap<Tid, ThreadRuntimePath>,
    main_tid: Tid,
}

impl RuntimePathTree {
    pub fn from_path_vec(path_vec: Vec<ThreadRuntimePath>, main_tid: Tid) -> Self {
        let mut data = HashMap::new();
        for path in path_vec {
            data.insert(path.tid, path);
        }
        RuntimePathTree { data, main_tid }
    }
}
