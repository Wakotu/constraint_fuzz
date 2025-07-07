use color_eyre::eyre::{bail, Result};
use dot_writer::{Attributes, DotWriter, Style};
use std::fmt;
use std::{
    cell::RefCell,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    rc::{Rc, Weak},
};

use crate::analysis::constraint::inter::action::{
    get_prefix, ExecAction, FuncAction, FuncActionType, IntraAction, LoopAction,
};
use crate::analysis::constraint::inter::error::{handle_guard_err_result, GuardParseError};
use crate::analysis::constraint::inter::loc::SrcLoc;
use crate::deopt::utils::write_bytes_to_file;
use crate::{
    config::{get_trunc_cnt, is_debug_mode},
    feedback::branches::constraints::Constraint,
};

pub type SharedFuncNodePtr = Rc<RefCell<FuncNode>>;
pub type WeakFuncNodePtr = Weak<RefCell<FuncNode>>;

const NODE_DELIM: &str = ", ";

pub trait DotId {
    fn get_dot_id(&self) -> &str;
}

pub enum FuncEntryType {
    Init,
    // regular function which has correponding function name
    Regular {
        name: String,
        parent: WeakFuncNodePtr,
    },
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
            FuncEntryType::Regular { name, parent: _ } => {
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
    fn get_dot_id(&self) -> &str {
        match self.node_type {
            FuncEntryType::Init => "init_node",
            FuncEntryType::Regular {
                ref name,
                parent: _,
            } => {
                // use function name as dot id
                name
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

    pub fn regular_node(name: String, parent: WeakFuncNodePtr) -> Self {
        Self {
            node_type: FuncEntryType::Regular { name, parent },
            data: vec![],
        }
    }

    pub fn iter_acts(&self) -> impl Iterator<Item = &ExecAction> {
        self.data.iter()
    }

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
        } = self.node_type
        {
            Some(name)
        } else {
            None
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
pub struct ExecTree {
    cur_node_ptr: SharedFuncNodePtr,
    root_ptr: SharedFuncNodePtr,
    // data: Vec<FuncNode>,
}

impl ExecTree {
    pub fn new() -> Self {
        let root_ptr = FuncNode::init_node().get_node_ptr();

        Self {
            cur_node_ptr: root_ptr.clone(),
            root_ptr,
        }
    }

    fn parse_regular_br_guard(
        line: &str,
    ) -> std::result::Result<(ValueHit, IntraAction), GuardParseError> {
        const REG_BR_PREFIX: &str = "Br Guard:";
        let prefix = get_prefix(line)?;
        if prefix != REG_BR_PREFIX {
            return Err(GuardParseError::to_prefix_err(eyre::eyre!(
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
            let intra_act = IntraAction::from_slice(intra_act_str)?;

            return Ok((value_hit, intra_act));
        }

        // if line does not match any known action, return error
        Err(GuardParseError::from(eyre::eyre!(
            "Line does not match any known action format: {}",
            line
        )))
    }

    fn parse_guard(&self, line: &str) -> Result<(Option<ValueHit>, Option<ExecAction>)> {
        // handle simple guards
        if let Some(value_hit) = handle_guard_err_result(ValueHit::parse_value_guard(line))? {
            return Ok((Some(value_hit), None));
        }
        if let Some(intra_act) = handle_guard_err_result(IntraAction::parse_simple_guard(line))? {
            return Ok((None, Some(ExecAction::Intra(intra_act))));
        }
        if let Some(loop_act) = handle_guard_err_result(LoopAction::parse_loop_guard(line))? {
            return Ok((None, Some(ExecAction::Loop(loop_act))));
        }

        // regular br parse
        if let Some((value_hit, intra_act)) =
            handle_guard_err_result(Self::parse_regular_br_guard(line))?
        {
            return Ok((Some(value_hit), Some(ExecAction::Intra(intra_act))));
        }

        let func_act = self.create_func_act(line)?;
        Ok((None, Some(ExecAction::Func(func_act))))
    }

    //// add record to current entry
    fn add_act(&mut self, act: &ExecAction) -> Result<()> {
        let mut cur_node = self.cur_node_ptr.borrow_mut();
        cur_node.push(act.to_owned());

        Ok(())
    }

    fn create_func_act(&self, line: &str) -> Result<FuncAction> {
        if !FuncActionType::is_call_guard(line) && !FuncActionType::is_return_guard(line) {
            bail!("Line does not match function action type: {}", line);
        }
        let func_name = FuncActionType::get_func_name(line)?;
        if FuncActionType::is_return_guard(line) {
            let func_act = FuncAction::new(FuncActionType::Return, func_name.to_owned());
            return Ok(func_act);
        }

        // create node and act_type
        let child_ptr =
            FuncNode::regular_node(func_name.to_owned(), Rc::downgrade(&self.cur_node_ptr))
                .get_node_ptr();

        let act_type = FuncActionType::Call { child_ptr };

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
    ) -> Result<()> {
        let (value_hit_op, act_op) = self.parse_guard(line)?;

        if let Some(act) = act_op {
            // add action to current node
            self.add_act(&act)?;

            // update current node pointer if action is a function call
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

        Ok(())
    }

    pub fn from_guard_file<P: AsRef<Path>>(fs_path: P, cons: &Constraint) -> Result<Self> {
        Self::from_guard_file_impl(fs_path.as_ref(), Some(cons))
    }

    pub fn from_guard_file_wo_constraint<P: AsRef<Path>>(fs_path: P) -> Result<Self> {
        Self::from_guard_file_impl(fs_path.as_ref(), None)
    }

    pub fn from_guard_file_impl(fs_path: &Path, cons_op: Option<&Constraint>) -> Result<Self> {
        let mut exec_tree: ExecTree = ExecTree::new();

        let file = File::open(fs_path)?;
        let reader = BufReader::new(file);
        let mut hit_cnt = 0;
        for (idx, line_res) in reader.lines().enumerate() {
            log::debug!("Processing line {}: {}", idx + 1, fs_path.display());
            let line = line_res?;
            // let exec_act = ExecAction::from_line(&line)?;
            exec_tree.read_line(&line, cons_op, &mut hit_cnt)?;

            if hit_cnt >= get_trunc_cnt() {
                break;
            }
        }

        Ok(exec_tree)
    }

    fn draw_func_cluster<'d, 'w>(
        cur_func_ptr: SharedFuncNodePtr,
        digraph: &mut dot_writer::Scope<'d, 'w>,
    ) -> Result<String> {
        let cur_func = cur_func_ptr.borrow();
        let cur_func_id = cur_func.get_dot_id();
        {
            let mut cluster = digraph.cluster();
            cluster.node_attributes().set_style(Style::Filled);

            // draw function -> action edges
            for act in cur_func.iter_acts() {
                let act_id = act.get_dot_id();
                cluster.edge(cur_func_id, act_id);
            }
        }
        // cluster.set_label(func_id);

        for act in cur_func.iter_acts() {
            if let Some(func_act) = act.get_func_call_act() {
                let sub_func_ptr = func_act.get_child_ptr().ok_or_else(|| {
                    eyre::eyre!(
                        "Function action is a call but has no child pointer: {}",
                        func_act.get_name()
                    )
                })?;
                let sub_func_id = Self::draw_func_cluster(sub_func_ptr, digraph)?;
                let act_id = act.get_dot_id();
                digraph.edge(act_id, sub_func_id);
            }
        }
        Ok(cur_func_id.to_owned())
    }

    fn draw_graph<'d, 'w>(&self, digraph: &mut dot_writer::Scope<'d, 'w>) -> Result<()> {
        Self::draw_func_cluster(self.root_ptr.clone(), digraph)?;
        Ok(())
    }

    pub fn to_dot_png<P: AsRef<Path>>(&self, png_path: P) -> Result<()> {
        let mut dot_bytes = vec![];

        // brackets to ensure that `dot_writer` is dropped before we write to the file
        {
            let mut dot_writer = DotWriter::from(&mut dot_bytes);
            dot_writer.set_pretty_print(false);
            let mut digraph = dot_writer.digraph();
            self.draw_graph(&mut digraph)?;
        }
        // write  dot_bytes to png_path
        write_bytes_to_file(png_path, &dot_bytes)?;

        Ok(())
    }
}

impl fmt::Debug for ExecTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let root = self.root_ptr.borrow();
        writeln!(f, "ExecTree:")?;
        write!(f, "{:?}", root)?;
        Ok(())
    }
}
