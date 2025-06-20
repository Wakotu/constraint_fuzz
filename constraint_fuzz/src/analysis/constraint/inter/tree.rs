use color_eyre::eyre::{bail, Result};
use std::{
    cell::RefCell,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
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

    pub fn is_call(line: &str) -> bool {
        line.starts_with(Self::ENT_PREFIX)
    }

    pub fn is_return(line: &str) -> bool {
        line.starts_with(Self::RET_PREFIX)
    }

    pub fn get_func_name(line: &str) -> Result<&str> {
        if !Self::is_call(line) && !Self::is_return(line) {
            bail!("Line does not match function action type: {}", line);
        }

        if Self::is_call(line) {
            get_func_namne_from_line(line, Self::ENT_PREFIX)
        } else {
            get_func_namne_from_line(line, Self::RET_PREFIX)
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
    line: u32,
    col: u32,
}

impl SrcLoc {
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

        let col = col_str.parse::<u32>()?;
        let line = line_str.parse::<u32>()?;
        let fpath = PathBuf::from(fpath_str);

        Ok(Self { fpath, line, col })
    }
}

#[derive(Clone)]
pub struct BrAction {
    cond_loc: SrcLoc,
    cond_val: bool,
    dest_loc: SrcLoc,
}

impl ActionTrait for BrAction {
    fn from_line(line: &str) -> Result<Self> {
        let prefix = "Branch Guard:";
        if !line.starts_with(prefix) {
            bail!("Line does not start with expected prefix: {}", line);
        }

        let line_cont = &line[prefix.len()..].trim();
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
            cond_loc,
            cond_val,
            dest_loc,
        })
    }
}

#[derive(Clone)]
pub enum ExecAction {
    Func(FuncAction),
    Br(BrAction),
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

// pub type FuncBrStack = Vec<FuncEntry>;
pub struct FuncBrTree {
    cur_node_ptr: SharedFuncNodePtr,
    root_ptr: SharedFuncNodePtr,
    // data: Vec<FuncNode>,
}

impl FuncBrTree {
    pub fn new() -> Self {
        let root_ptr = FuncNode::init_node().get_node_ptr();

        Self {
            cur_node_ptr: root_ptr.clone(),
            root_ptr,
        }
    }

    //// add record to current entry
    fn add_act(&mut self, act: &ExecAction) -> Result<()> {
        let mut cur_node = self.cur_node_ptr.borrow_mut();
        cur_node.push(act.to_owned());

        Ok(())
    }

    fn create_func_act(&self, line: &str) -> Result<FuncAction> {
        if !FuncActionType::is_call(line) && !FuncActionType::is_return(line) {
            bail!("Line does not match function action type: {}", line);
        }
        let func_name = FuncActionType::get_func_name(line)?;
        if FuncActionType::is_return(line) {
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

    fn create_act(&self, line: &str) -> Result<ExecAction> {
        if let Ok(br_act) = BrAction::from_line(line) {
            return Ok(ExecAction::Br(br_act));
        }

        let func_act = self.create_func_act(line)?;
        Ok(ExecAction::Func(func_act))
    }

    pub fn read_line(&mut self, line: &str) -> Result<()> {
        let act = self.create_act(line)?;
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
                let parent_ptr = self.cur_node_ptr.borrow().get_parent_ptr().ok_or_else(|| {
                    eyre::eyre!(
                        "Current node has no parent, cannot return: {}",
                        func_act.func_name
                    )
                })?;
                self.cur_node_ptr = parent_ptr;
            }
        }

        Ok(())
    }
}

fn get_func_namne_from_line<'a>(line: &'a str, prefix: &'a str) -> Result<&'a str> {
    if !line.starts_with(prefix) {
        bail!("Line does not start with expected prefix: {}", line);
    }

    // extract func_name: get rid of prefix and read until char '('
    let start = prefix.len();
    let end = line.find('(').unwrap_or_else(|| line.len());
    let func_name = &line[start..end];
    Ok(func_name)
}

pub fn get_inter_proc_path(target_func: &str, fs_path: &Path) -> Result<FuncBrTree> {
    let mut fb_tree: FuncBrTree = FuncBrTree::new();

    let file = File::open(fs_path)?;
    let reader = BufReader::new(file);
    for line_res in reader.lines() {
        let line = line_res?;
        // let exec_act = ExecAction::from_line(&line)?;
        fb_tree.read_line(&line)?;
    }

    todo!()
}
