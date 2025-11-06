use std::collections::HashMap;

use crate::analysis::constraint::stmt_collect::ProcessUnit;

pub type Tid = usize;

pub struct PathRoot {
    tid: Tid,
    len: usize,
}

pub struct ThreadRuntimePath {
    tid: Tid,
    root: Option<PathRoot>,
    pu_vec: Vec<ProcessUnit>,
}

pub struct RuntimeTree {
    data: HashMap<Tid, ThreadRuntimePath>,
}
