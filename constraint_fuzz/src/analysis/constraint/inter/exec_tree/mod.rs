use chrono::format::parse;
use color_eyre::eyre::{bail, Result};
use dot_writer::{Attributes, DotWriter, Style};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::{
    cell::RefCell,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    rc::{Rc, Weak},
};

use crate::analysis::constraint::inter::error::GuardParseError;
use crate::analysis::constraint::inter::exec_tree::action::{
    get_prefix, ExecAction, FuncAction, FuncActionType, JumpAction, LoopAction, RecurAction,
    ThreadAction,
};
use crate::analysis::constraint::inter::exec_tree::analyze::FuncNodeLenEntry;
use crate::analysis::constraint::inter::loc::SrcLoc;
use crate::{
    config::{get_trunc_cnt, is_debug_mode},
    feedback::branches::constraints::Constraint,
};

pub mod action;
pub mod analyze;

pub trait FuncIter {
    fn iter_sub_funcs(&self) -> SubFuncIter;
}

pub type SharedFuncNodePtr = Rc<RefCell<FuncNode>>;
pub type WeakFuncNodePtr = Weak<RefCell<FuncNode>>;

const NODE_DELIM: &str = ", ";

pub static DOT_COUNTER: OnceLock<Mutex<usize>> = OnceLock::new();

impl FuncIter for SharedFuncNodePtr {
    fn iter_sub_funcs(&self) -> SubFuncIter {
        SubFuncIter::from_func_ptr(self.clone())
    }
}

pub fn incre_dot_counter() -> usize {
    let mut counter = DOT_COUNTER
        .get_or_init(|| Mutex::new(0))
        .lock()
        .expect("Failed to lock DOT_COUNTER mutex");
    *counter += 1;
    *counter
}

pub trait DotId {
    fn get_dot_id(&self) -> String;
}

pub enum FuncEntryType {
    Init,
    // regular function which has correponding function name
    Regular {
        parent_idx: usize,
        name: String,
        parent: WeakFuncNodePtr,
    },
}

pub struct SubFuncIter {
    parent_func_ptr: SharedFuncNodePtr,
    cur_func_ptr: Option<SharedFuncNodePtr>,
}

impl SubFuncIter {
    pub fn from_func_ptr(parent_func_ptr: SharedFuncNodePtr) -> Self {
        Self {
            parent_func_ptr,
            cur_func_ptr: None,
        }
    }
}

impl Iterator for SubFuncIter {
    type Item = SharedFuncNodePtr;

    fn next(&mut self) -> Option<Self::Item> {
        let prev_idx = if let Some(cur_ptr) = &self.cur_func_ptr {
            let cur_func = cur_ptr.borrow();
            cur_func
                .get_parent_idx()
                .expect("Current function pointer should have a parent index") as i64
        } else {
            -1
        };
        let parent_func = self.parent_func_ptr.borrow();
        for act in parent_func.iter_acts_at((prev_idx + 1) as usize) {
            if let ExecAction::Func(func_act) = act {
                // get child pointer
                if let Some(child_ptr) = func_act.get_child_ptr() {
                    self.cur_func_ptr = Some(child_ptr.clone());
                    return Some(child_ptr.clone());
                }
            }
        }
        None
    }
}

pub struct FuncNode {
    // node_type field which contains func name
    node_type: FuncEntryType,
    data: Vec<ExecAction>,
}

impl fmt::Debug for FuncNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.node_type {
            FuncEntryType::Init => write!(f, "Init Node("),
            FuncEntryType::Regular {
                name,
                parent: _,
                parent_idx: _,
            } => {
                write!(f, "{} Node(", name)
            }
        }?;

        write!(f, "\n")?;

        // action list output
        for (idx, act) in self.data.iter().enumerate() {
            if idx > 0 {
                write!(f, "{}", NODE_DELIM)?;
            }
            writeln!(f, "{:?}", act)?;
        }

        writeln!(f, ")")?;
        Ok(())
    }
}

impl DotId for FuncNode {
    fn get_dot_id(&self) -> String {
        let cnt = incre_dot_counter();
        match self.node_type {
            FuncEntryType::Init => format!("init_node_{}", cnt),
            FuncEntryType::Regular {
                ref name,
                parent: _,
                parent_idx: _,
            } => {
                // use function name as dot id
                format!("{}_{}", name, cnt)
            }
        }
    }
}

impl FuncNode {
    pub fn init_node() -> Self {
        Self {
            node_type: FuncEntryType::Init,
            data: vec![],
        }
    }

    pub fn get_len(&self) -> usize {
        self.data.len()
    }

    pub fn get_parent_idx(&self) -> Option<usize> {
        match &self.node_type {
            FuncEntryType::Regular { parent_idx, .. } => Some(*parent_idx),
            FuncEntryType::Init => None,
        }
    }

    pub fn regular_node(name: String, parent: WeakFuncNodePtr, parent_idx: usize) -> Self {
        Self {
            node_type: FuncEntryType::Regular {
                name,
                parent,
                parent_idx,
            },
            data: vec![],
        }
    }

    pub fn iter_acts(&self) -> impl Iterator<Item = &ExecAction> {
        self.data.iter()
    }

    pub fn iter_acts_at(&self, start: usize) -> impl Iterator<Item = &ExecAction> {
        self.data.iter().skip(start)
    }

    pub fn get_act_at(&self, idx: usize) -> Option<&ExecAction> {
        self.data.get(idx)
    }

    /// Should only be used during construction of ExecTree
    pub fn get_node_ptr(self) -> SharedFuncNodePtr {
        Rc::new(RefCell::new(self))
    }

    pub fn get_parent_ptr(&self) -> Option<SharedFuncNodePtr> {
        let weak_ptr = match &self.node_type {
            FuncEntryType::Regular { parent, .. } => Some(parent.clone()),
            FuncEntryType::Init => None,
        };
        weak_ptr.and_then(|w| w.upgrade())
    }

    pub fn to_len_entry(&self) -> FuncNodeLenEntry {
        let func_name = self.get_func_name_or_init().to_owned();
        let len = self.get_len();
        FuncNodeLenEntry::new(func_name, len)
    }

    pub fn is_init(&self) -> bool {
        matches!(self.node_type, FuncEntryType::Init)
    }

    pub fn is_regular(&self) -> bool {
        matches!(self.node_type, FuncEntryType::Regular { .. })
    }

    pub fn get_func_name(&self) -> Option<&str> {
        if let FuncEntryType::Regular {
            ref name,
            parent: _,
            parent_idx: _,
        } = self.node_type
        {
            Some(name)
        } else {
            None
        }
    }

    pub fn get_func_name_or_init(&self) -> &str {
        if let Some(name) = self.get_func_name() {
            name
        } else {
            "_init"
        }
    }

    pub fn push(&mut self, act: ExecAction) {
        self.data.push(act);
    }
}

pub struct ValueHit {
    loc: SrcLoc,
}

impl fmt::Debug for ValueHit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ValueHit({:?})", self.loc)
    }
}

impl ValueHit {
    pub fn get_src_path(&self) -> Option<&Path> {
        self.loc.get_src_path()
    }
    pub fn get_loc(&self) -> &SrcLoc {
        &self.loc
    }

    pub fn get_line(&self) -> Option<usize> {
        self.loc.get_line()
    }
    pub fn parse_value_guard(line: &str) -> std::result::Result<ValueHit, GuardParseError> {
        const VAL_PREFIX: &str = "Unconditional Branch Value:";
        let loc = SrcLoc::parse_line_with_prefix(line, VAL_PREFIX)?;

        Ok(ValueHit { loc })
    }

    pub fn from_str(slice: &str) -> Result<Self> {
        // example: /path/to/file.c:123:45
        let loc = SrcLoc::from_str(slice)?;
        Ok(ValueHit { loc })
    }
}
// pub type FuncBrStack = Vec<FuncEntry>;
pub struct ThreadTree {
    cur_node_ptr: SharedFuncNodePtr,
    root_ptr: SharedFuncNodePtr,
    cur_depth: usize,
    max_depth: usize,
    // data: Vec<FuncNode>,
}

impl ThreadTree {
    pub fn new() -> Self {
        let root_ptr = FuncNode::init_node().get_node_ptr();

        Self {
            cur_node_ptr: root_ptr.clone(),
            root_ptr,
            cur_depth: 0,
            max_depth: 0,
        }
    }

    pub fn get_root_ptr(&self) -> SharedFuncNodePtr {
        self.root_ptr.clone()
    }

    pub fn get_depth(&self) -> usize {
        self.max_depth
    }

    fn parse_regular_br_guard(
        line: &str,
    ) -> std::result::Result<(ValueHit, JumpAction), GuardParseError> {
        const REG_BR_PREFIX: &str = "Br Guard:";
        let prefix = get_prefix(line)?;
        if prefix != REG_BR_PREFIX {
            return Err(GuardParseError::as_prefix_err(eyre::eyre!(
                "Line does not match regular branch guard prefix: {}",
                line
            )));
        }

        let line_cont = line[prefix.len()..].trim();
        if let Some(idx) = line_cont.find(char::is_whitespace) {
            let value_hit_str = &line_cont[..idx];
            let intra_act_str = line_cont[idx..].trim();

            // parse value hit
            let value_hit = ValueHit::from_str(value_hit_str)?;

            // parse intra action
            let intra_act = JumpAction::from_slice(intra_act_str)?;

            return Ok((value_hit, intra_act));
        }

        // if line does not match any known action, return error
        Err(GuardParseError::from(eyre::eyre!(
            "Line does not match any known action format: {}",
            line
        )))
    }

    /// May return 3 kinds of GuardParseError variants as error.
    /// Of which SkipError should be taken care of as a repeat signal
    fn parse_guard_impl(
        &self,
        line: &str,
    ) -> std::result::Result<
        (Option<ValueHit>, Option<ExecAction>, Option<THCPEntry>),
        GuardParseError,
    > {
        // value hit
        if let Some(value_hit) = GuardParseError::to_eyre(ValueHit::parse_value_guard(line))? {
            return Ok((Some(value_hit), None, None));
        }
        // simple guards
        if let Some(intra_act) = GuardParseError::to_eyre(JumpAction::parse_simple_guard(line))? {
            return Ok((None, Some(ExecAction::Intra(intra_act)), None));
        }
        if let Some(loop_act) = GuardParseError::to_eyre(LoopAction::parse_loop_guard(line))? {
            return Ok((None, Some(ExecAction::Loop(loop_act)), None));
        }
        if let Some(recur_act) = GuardParseError::to_eyre(RecurAction::parse_recur_guard(line))? {
            return Ok((None, Some(ExecAction::Recur(recur_act)), None));
        }
        if let Some(thread_act) = GuardParseError::to_eyre(ThreadAction::parse_thread_guard(line))?
        {
            // construct a thcp entry
            let func_node_ptr = self.cur_node_ptr.clone();
            let act_idx = func_node_ptr.borrow().get_len();
            let tid = thread_act.get_thread_id();
            let thcp_entry = (tid, ActionPoint::new(func_node_ptr, act_idx));

            return Ok((None, Some(ExecAction::Thread(thread_act)), Some(thcp_entry)));
        }

        // regular br parse
        if let Some((value_hit, intra_act)) =
            GuardParseError::to_eyre(Self::parse_regular_br_guard(line))?
        {
            return Ok((Some(value_hit), Some(ExecAction::Intra(intra_act)), None));
        }

        // function action parse
        let func_act = self.create_func_act(line)?;

        Ok((None, Some(ExecAction::Func(func_act)), None))
    }

    fn parse_guard(
        &self,
        line: &str,
    ) -> Result<(Option<ValueHit>, Option<ExecAction>, Option<THCPEntry>)> {
        let mut parse_res;
        let mut parse_content = line;

        loop {
            parse_res = self.parse_guard_impl(parse_content);
            if let Err(GuardParseError::SkipError { data: _, skip_num }) = parse_res {
                // if SkipError, skip the number of characters and try again
                parse_content = &parse_content[skip_num..];
            } else {
                break;
            }
        }
        GuardParseError::to_eyre_ultimate(parse_res)
    }

    //// add record to current entry
    fn add_act(&mut self, act: &ExecAction) -> Result<()> {
        let mut cur_node = self.cur_node_ptr.borrow_mut();
        cur_node.push(act.to_owned());

        Ok(())
    }

    fn create_func_act(&self, line: &str) -> std::result::Result<FuncAction, GuardParseError> {
        if let Ok(return_act) = FuncAction::parse_return_guard(line) {
            return Ok(return_act);
        }

        if let Ok(unwind_act) = FuncAction::parse_unwind_guard(line) {
            return Ok(unwind_act);
        }

        // possible to return skip error
        let (invoc_loc_op, func_name) = FuncAction::parse_call_guard(line)?;

        /* get context information for newly created function node */
        // get index of Function Action which corresponds to new function node.
        let cur_act_len = {
            let cur_func = self.cur_node_ptr.borrow();
            cur_func.get_len()
        };
        // create node and act_type
        let child_ptr = FuncNode::regular_node(
            func_name.to_owned(),
            Rc::downgrade(&self.cur_node_ptr),
            cur_act_len,
        )
        .get_node_ptr();

        let act_type = FuncActionType::Call {
            child_ptr,
            invoc_loc: invoc_loc_op,
        };

        let func_act = FuncAction::new(act_type, func_name.to_owned());
        return Ok(func_act);
    }

    // fn create_act(&self, line: &str) -> Result<ExecAction> {
    //     if let Ok(br_act) = IntraAction::from_line(line) {
    //         return Ok(ExecAction::Br(br_act));
    //     }

    //     let func_act = self.create_func_act(line)?;
    //     Ok(ExecAction::Func(func_act))
    // }

    pub fn read_line(
        &mut self,
        line: &str,
        cons_op: Option<&Constraint>,
        hit_cnt: &mut usize,
    ) -> Result<Option<THCPEntry>> {
        let (value_hit_op, act_op, thcp_entry_op) = self.parse_guard(line)?;

        if let Some(act) = act_op {
            // add action to current node
            self.add_act(&act)?;

            // update context information in case of function actions: current pointer and depth
            if let ExecAction::Func(func_act) = act {
                if func_act.is_call() {
                    // update current node pointer to the new function node
                    let child_ptr = func_act.get_child_ptr().ok_or_else(|| {
                        eyre::eyre!(
                            "Function action is a call but has no child pointer: {}",
                            func_act.get_name()
                        )
                    })?;
                    self.cur_node_ptr = child_ptr;
                    self.cur_depth += 1;
                    if self.cur_depth > self.max_depth {
                        self.max_depth = self.cur_depth;
                    }
                } else if func_act.is_return() {
                    // move up in the tree
                    let parent_ptr =
                        self.cur_node_ptr.borrow().get_parent_ptr().ok_or_else(|| {
                            eyre::eyre!(
                                "Current node has no parent, cannot return: {}",
                                func_act.get_name()
                            )
                        })?;
                    self.cur_node_ptr = parent_ptr;
                    self.cur_depth -= 1;
                }
            }

            // let act = self.create_act(line)?;
        }

        if let Some(val_hit) = value_hit_op {
            // if is_debug_mode() && cons.same_src_file(&val_hit) {
            //     log::debug!("Value Hit: {:?}", val_hit);
            //     log::debug!("Constraint: {}", cons);
            // }
            if let Some(cons) = cons_op {
                if is_debug_mode() && cons.near_hit(&val_hit) {
                    log::debug!("Value Hit: {:?}", val_hit);
                    log::debug!("Constraint: {}", cons);
                }
                if cons.is_hit(&val_hit)? {
                    *hit_cnt += 1;
                }
            }
        }

        Ok(thcp_entry_op)
    }

    pub fn from_guard_file<P: AsRef<Path>>(
        fs_path: P,
        cons: &Constraint,
    ) -> Result<(Self, THCPMAPPING)> {
        Self::from_guard_file_impl(fs_path.as_ref(), Some(cons))
    }

    pub fn from_guard_file_wo_constraint<P: AsRef<Path>>(
        fs_path: P,
    ) -> Result<(Self, THCPMAPPING)> {
        Self::from_guard_file_impl(fs_path.as_ref(), None)
    }

    // single tree version
    pub fn from_guard_file_wo_constraint_st<P: AsRef<Path>>(fs_path: P) -> Result<Self> {
        let (exec_tree, _) = Self::from_guard_file_impl(fs_path.as_ref(), None)?;
        Ok(exec_tree)
    }

    pub fn from_guard_file_impl(
        fs_path: &Path,
        cons_op: Option<&Constraint>,
    ) -> Result<(Self, THCPMAPPING)> {
        let mut exec_tree: ThreadTree = ThreadTree::new();
        let mut thcp_mapping = HashMap::new();

        let file = File::open(fs_path)?;
        let reader = BufReader::new(file);
        let mut hit_cnt = 0;
        for (idx, line_res) in reader.lines().enumerate() {
            log::debug!("Processing line {}: {}", idx + 1, fs_path.display());
            let line = line_res?;
            // let exec_act = ExecAction::from_line(&line)?;
            let thcp_entry_op = exec_tree.read_line(&line, cons_op, &mut hit_cnt)?;
            if let Some(thcp_entry) = thcp_entry_op {
                thcp_mapping.insert(thcp_entry.0, thcp_entry.1);
            }

            // truncate if hit count exceeds truncation count
            if hit_cnt >= get_trunc_cnt() {
                break;
            }
        }

        Ok((exec_tree, thcp_mapping))
    }
}

impl fmt::Debug for ThreadTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root = self.root_ptr.borrow();
        writeln!(f, "ExecTree:")?;
        write!(f, "{:?}", root)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ActionPoint {
    func_node_ptr: SharedFuncNodePtr,
    act_idx: usize,
}

impl ActionPoint {
    pub fn new(func_node_ptr: SharedFuncNodePtr, act_idx: usize) -> Self {
        Self {
            func_node_ptr,
            act_idx,
        }
    }

    pub fn get_func_node_ptr(&self) -> SharedFuncNodePtr {
        self.func_node_ptr.clone()
    }

    pub fn get_act_idx(&self) -> usize {
        self.act_idx
    }
}

pub type Tid = usize;
pub type THCPEntry = (Tid, ActionPoint);
pub type THCPMAPPING = HashMap<Tid, ActionPoint>;
pub struct ExecForest {
    main_tree: ThreadTree,
    sub_trees: Vec<ThreadTree>,
    /// means mapping for thread id to thread creation action point
    thcp_mapping: THCPMAPPING,
}

impl ExecForest {
    pub fn from_guard_dir<P: AsRef<Path>>(guard_dir: P) {}
}
