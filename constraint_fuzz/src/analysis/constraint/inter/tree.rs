use color_eyre::eyre::{bail, Result};
use std::{
    cell::RefCell,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf, Prefix},
    rc::{Rc, Weak},
};

use crate::{
    config::get_trunc_cnt,
    feedback::branches::constraints::{Constraint, LocTrait, Range, RangeTrait},
};

pub type SharedFuncNodePtr = Rc<RefCell<FuncNode>>;
pub type WeakFuncNodePtr = Weak<RefCell<FuncNode>>;

#[derive(Clone)]
pub enum FuncActionType {
    Call { child_ptr: SharedFuncNodePtr },
    Return,
}

impl FuncActionType {
    const ENT_PREFIX: &'static str = "enter ";
    const RET_PREFIX: &'static str = "return from ";

    pub fn is_call_guard(line: &str) -> bool {
        line.starts_with(Self::ENT_PREFIX)
    }

    pub fn is_return_guard(line: &str) -> bool {
        line.starts_with(Self::RET_PREFIX)
    }

    fn get_func_name_from_line<'a>(line: &'a str, prefix: &'a str) -> Result<&'a str> {
        if !line.starts_with(prefix) {
            bail!("Line does not start with expected prefix: {}", line);
        }

        // extract func_name: get rid of prefix and read until char '('
        let start = prefix.len();
        let end = line.find('(').unwrap_or_else(|| line.len());
        let func_name = &line[start..end];
        Ok(func_name)
    }

    pub fn get_func_name(line: &str) -> Result<&str> {
        if !Self::is_call_guard(line) && !Self::is_return_guard(line) {
            bail!("Line does not match function action type: {}", line);
        }

        if Self::is_call_guard(line) {
            Self::get_func_name_from_line(line, Self::ENT_PREFIX)
        } else {
            Self::get_func_name_from_line(line, Self::RET_PREFIX)
        }
    }
}

pub trait ActionTrait {
    fn from_line(line: &str) -> Result<Self>
    where
        Self: Sized;
}

#[derive(Clone)]
pub struct FuncAction {
    act_type: FuncActionType,
    func_name: String,
}

impl FuncAction {
    pub fn is_call(&self) -> bool {
        matches!(self.act_type, FuncActionType::Call { .. })
    }

    pub fn is_return(&self) -> bool {
        matches!(self.act_type, FuncActionType::Return)
    }

    pub fn get_child_ptr(&self) -> Option<SharedFuncNodePtr> {
        if let FuncActionType::Call { child_ptr } = &self.act_type {
            Some(child_ptr.clone())
        } else {
            None
        }
    }
}

// impl ActionTrait for FuncAction {
//     fn from_line(line: &str) -> Result<Self> {
//         let (act_type, pref_len) = FuncActionType::from_line(line)?;

//         let func_name = get_func_namne_from_line(line, &line[0..pref_len])?;
//         Ok(Self {
//             act_type,
//             func_name: func_name.to_owned(),
//         })
//     }
// }

#[derive(Clone)]
pub struct SrcLoc {
    fpath: PathBuf,
    line: usize,
    col: usize,
}

impl SrcLoc {
    pub fn inside_range(&self, rng: &Range, fpath: &Path) -> Result<bool> {
        if self.fpath != fpath {
            return Ok(false);
        }

        let [sloc, eloc] = rng.extract_locs()?;
        let loc = [self.line, self.col];
        Ok(sloc.is_less_equal(&loc) && loc.is_less_equal(&eloc))
    }

    pub fn parse_line_with_prefix(line: &str, prefix: &str) -> Result<Self> {
        if !line.starts_with(prefix) {
            bail!("Line does not start with expected prefix: {}", line);
        }

        let loc_str = &line[prefix.len()..].trim();
        Self::from_str(loc_str)
    }

    pub fn from_str(s: &str) -> Result<Self> {
        // example: /path/to/file.c:123:45
        let mut parts = s.rsplitn(3, ':');
        let col_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing column in source location"))?;
        let line_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing line in source location"))?;
        let fpath_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing file path in source location"))?;

        let col = col_str.parse::<usize>()?;
        let line = line_str.parse::<usize>()?;
        let fpath = PathBuf::from(fpath_str);

        Ok(Self { fpath, line, col })
    }
}

#[derive(Clone)]
enum IntraActionType {
    BrGuard,
    SwitchGuard,
    IndirectGuard,
}

impl IntraActionType {
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "Merge Br Guard:" => Some(IntraActionType::BrGuard),
            "Switch Guard:" => Some(IntraActionType::SwitchGuard),
            "IndirectBr Guard:" => Some(IntraActionType::IndirectGuard),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct IntraAction {
    intra_type: IntraActionType,
    cond_loc: SrcLoc,
    cond_val: bool,
    dest_loc: SrcLoc,
}

fn get_prefix(line: &str) -> Result<&str> {
    // get position of ':' in the line
    if let Some(pos) = line.find(':') {
        // return the substring from the start to the position of ':'
        Ok(&line[..pos + 1])
    } else {
        bail!("Line does not contain a colon: {}", line);
    }
}
impl IntraAction {
    pub fn parse_simple_guard(line: &str) -> Result<Self> {
        let prefix = get_prefix(line)?;
        let intra_type = IntraActionType::from_prefix(prefix)
            .ok_or_else(|| eyre::eyre!("Unknown intra action type prefix: {}", prefix))?;

        let line_cont = line[prefix.len()..].trim();
        let mut iter = line_cont.split_whitespace();
        let cond_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition location"))?;
        let cond_loc = SrcLoc::from_str(cond_loc_str)?;

        let cond_val_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing condition value"))?;
        let cond_val = match cond_val_str {
            "1" => true,
            "0" => false,
            _ => bail!("Unexpected condition value: {}", cond_val_str),
        };
        let dest_loc_str = iter
            .next()
            .ok_or_else(|| eyre::eyre!("Missing destination location"))?;
        let dest_loc = SrcLoc::from_str(dest_loc_str)?;

        Ok(Self {
            intra_type,
            cond_loc,
            cond_val,
            dest_loc,
        })
    }

    pub fn from_slice(slice: &str) -> Result<Self> {
        // example: /path/to/file.c:123:45 1 /path/to/dest.c:67:89
        let parts: Vec<&str> = slice.split_whitespace().collect();
        if parts.len() != 3 {
            bail!("Expected 3 parts in intra action, found {}", parts.len());
        }

        let cond_loc = SrcLoc::from_str(parts[0])?;
        let cond_val = match parts[1] {
            "1" => true,
            "0" => false,
            _ => bail!("Unexpected condition value: {}", parts[1]),
        };
        let dest_loc = SrcLoc::from_str(parts[2])?;

        Ok(Self {
            intra_type: IntraActionType::BrGuard, // default type, can be changed later
            cond_loc,
            cond_val,
            dest_loc,
        })
    }
}

#[derive(Clone)]
pub enum ExecAction {
    Func(FuncAction),

    Br(IntraAction),
}

// impl ActionTrait for ExecAction {
//     fn from_line(line: &str) -> Result<Self> {
//         if let Ok(func_act) = FuncAction::from_line(line) {
//             return Ok(ExecAction::Func(func_act));
//         }
//         if let Ok(br_act) = BrAction::from_line(line) {
//             return Ok(ExecAction::Br(br_act));
//         }
//         bail!("Line does not match any known action format: {}", line);
//     }
// }

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

impl ValueHit {
    pub fn get_loc(&self) -> &SrcLoc {
        &self.loc
    }
    pub fn parse_value_guard(line: &str) -> Result<ValueHit> {
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

    fn parse_regular_br_guard(line: &str) -> Result<(ValueHit, IntraAction)> {
        const REG_BR_PREFIX: &str = "Br Guard:";
        let prefix = get_prefix(line)?;
        if prefix != REG_BR_PREFIX {
            bail!("Line does not match regular branch guard prefix: {}", line);
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
        bail!("Line does not match any known action format: {}", line);
    }

    fn parse_guard(&self, line: &str) -> Result<(Option<ValueHit>, Option<ExecAction>)> {
        // handle simple guards
        if let Ok(value_hit) = ValueHit::parse_value_guard(line) {
            return Ok((Some(value_hit), None));
        }
        if let Ok(intra_act) = IntraAction::parse_simple_guard(line) {
            return Ok((None, Some(ExecAction::Br(intra_act))));
        }

        // regular br parse
        if let Ok((value_hit, intra_act)) = Self::parse_regular_br_guard(line) {
            return Ok((Some(value_hit), Some(ExecAction::Br(intra_act))));
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
            let func_act = FuncAction {
                act_type: FuncActionType::Return,
                func_name: func_name.to_owned(),
            };
            return Ok(func_act);
        }

        // create node and act_type
        let child_ptr =
            FuncNode::regular_node(func_name.to_owned(), Rc::downgrade(&self.cur_node_ptr))
                .get_node_ptr();

        let act_type = FuncActionType::Call { child_ptr };

        let func_act = FuncAction {
            act_type,
            func_name: func_name.to_owned(),
        };
        return Ok(func_act);
    }

    // fn create_act(&self, line: &str) -> Result<ExecAction> {
    //     if let Ok(br_act) = IntraAction::from_line(line) {
    //         return Ok(ExecAction::Br(br_act));
    //     }

    //     let func_act = self.create_func_act(line)?;
    //     Ok(ExecAction::Func(func_act))
    // }

    pub fn read_line(&mut self, line: &str, cons: &Constraint, hit_cnt: &mut usize) -> Result<()> {
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
                            func_act.func_name
                        )
                    })?;
                    self.cur_node_ptr = child_ptr;
                } else if func_act.is_return() {
                    // move up in the tree
                    let parent_ptr =
                        self.cur_node_ptr.borrow().get_parent_ptr().ok_or_else(|| {
                            eyre::eyre!(
                                "Current node has no parent, cannot return: {}",
                                func_act.func_name
                            )
                        })?;
                    self.cur_node_ptr = parent_ptr;
                }
            }

            // let act = self.create_act(line)?;
        }

        if let Some(val_hit) = value_hit_op {
            // TODO: truncation logic
            if cons.is_hit(&val_hit)? {
                *hit_cnt += 1;
            }
        }

        Ok(())
    }

    pub fn get_inter_proc_path(fs_path: &Path, cons: &Constraint) -> Result<Self> {
        let mut exec_tree: ExecTree = ExecTree::new();

        let file = File::open(fs_path)?;
        let reader = BufReader::new(file);
        let mut hit_cnt = 0;
        for line_res in reader.lines() {
            let line = line_res?;
            // let exec_act = ExecAction::from_line(&line)?;
            exec_tree.read_line(&line, cons, &mut hit_cnt)?;

            if hit_cnt >= get_trunc_cnt() {
                break;
            }
        }

        Ok(exec_tree)
    }
}
