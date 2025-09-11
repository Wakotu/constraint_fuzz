use core::panic;
use std::{
    cell::RefCell,
    process::Child,
    rc::{Rc, Weak},
};

use color_eyre::eyre::Result;
use eyre::bail;
use my_macros::EquivByLoc;

use crate::analysis::constraint::{
    inter::exec_tree::thread_tree::ExecFuncNode,
    intra::func_src_tree::{
        code_query::{
            func_invoc_query::{FuncInvoc, FuncInvocMap},
            switch_query::CaseMap,
        },
        nodes::cf_mod::{CFStruct, CasePtrMap},
        stmts::{BlockStmt, BlockType, ChildEntry, ForStmt, IfStmt, QLLoc, SwitchStmt, WhileStmt},
    },
};

pub enum StmtNodeVariants {
    Block(BlockStmtNode),
    Plain(PlainStmtNode),
    CFStruct(CFStruct),
}

pub struct StmtNode {
    /// the field where data is stored
    pub variants: StmtNodeVariants,
    /// parent pointer for non-root nodes
    pub parent_ptr_op: Option<WeakStmtNodePtr>,
    /// index in parent's stmts vec, None for non-block parents
    pub parent_idx_op: Option<usize>,
    /// case label location if this node is under a switch-case
    pub parent_case_loc_op: Option<QLLoc>,
}

impl StmtNode {
    /**
     * Default Pointer Creation
     */

    pub fn create_plain_ptr(
        entry: &ChildEntry,
        func_invoc_map: &FuncInvocMap,
        // parent_ptr: WeakStmtNodePtr
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Plain(SrcExpr::from_loc_and_invocs(
                &entry.loc,
                func_invoc_map,
            )),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }

    pub fn create_block_ptr(
        block_stmt: &BlockStmt,
        stmts: Vec<SharedStmtNodePtr>,
        // parent_ptr: Option<WeakStmtNodePtr>,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Block(BlockStmtNode {
                loc: block_stmt.loc.clone(),
                block_type: block_stmt.block_type.clone(),
                stmts,
            }),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }

    pub fn create_if_ptr(
        if_stmt: &IfStmt,
        then_ptr: SharedStmtNodePtr,
        else_ptr: Option<SharedStmtNodePtr>,
        func_invoc_map: &FuncInvocMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::If(cf_mod::IfNode {
                loc: if_stmt.loc.clone(),
                cond_expr: SrcExpr::from_loc_and_invocs(&if_stmt.cond_loc, func_invoc_map),
                then_blk: then_ptr,
                else_blk: else_ptr,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }

    pub fn create_switch_ptr(
        switch_stmt: &SwitchStmt,
        case_ptr_map: CasePtrMap,
        func_invoc_map: &FuncInvocMap,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::Switch(cf_mod::SwitchNode {
                loc: switch_stmt.loc.clone(),
                expr_loc: SrcExpr::from_loc_and_invocs(&switch_stmt.expr_loc, func_invoc_map),
                case_ptr_map,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }

    pub fn create_while_ptr(
        while_stmt: &WhileStmt,
        body_ptr: SharedStmtNodePtr,
        func_invoc_map: &FuncInvocMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::While(cf_mod::WhileNode {
                loc: while_stmt.loc.clone(),
                while_type: while_stmt.while_type.clone(),
                cond_expr: SrcExpr::from_loc_and_invocs(&while_stmt.cond_loc, func_invoc_map),
                body: body_ptr,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }

    pub fn create_for_ptr(
        for_stmt: &ForStmt,
        body_ptr: SharedStmtNodePtr,
        func_invoc_map: &FuncInvocMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::For(cf_mod::ForNode {
                loc: for_stmt.loc.clone(),
                init: match &for_stmt.init_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                cond: match &for_stmt.cond_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                update: match &for_stmt.update_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                body: body_ptr,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
        }))
    }
}

pub type SharedStmtNodePtr = Rc<RefCell<StmtNode>>;
pub type WeakStmtNodePtr = Weak<RefCell<StmtNode>>;

pub type PlainStmtNode = SrcExpr;

#[derive(EquivByLoc)]
pub struct BlockStmtNode {
    loc: QLLoc,
    block_type: BlockType,
    stmts: Vec<SharedStmtNodePtr>,
}

impl BlockStmtNode {
    pub fn stmts_len(&self) -> usize {
        self.stmts.len()
    }

    pub fn get_first_stmt(&self) -> Option<SharedStmtNodePtr> {
        if self.stmts.is_empty() {
            None
        } else {
            Some(Rc::clone(&self.stmts[0]))
        }
    }
}

pub mod cf_mod {

    use std::collections::HashMap;

    use my_macros::EquivByLoc;

    use crate::analysis::constraint::intra::func_src_tree::{
        nodes::{SharedStmtNodePtr, SrcExpr},
        stmts::{QLLoc, WhileType},
    };

    #[derive(EquivByLoc)]
    pub struct IfNode {
        pub loc: QLLoc,
        pub cond_expr: SrcExpr,
        pub then_blk: SharedStmtNodePtr,
        pub else_blk: Option<SharedStmtNodePtr>,
    }

    pub type CasePtrMap = HashMap<QLLoc, Vec<SharedStmtNodePtr>>;

    #[derive(EquivByLoc)]
    pub struct SwitchNode {
        pub loc: QLLoc,
        pub expr_loc: SrcExpr,
        pub case_ptr_map: CasePtrMap,
    }

    #[derive(EquivByLoc)]
    pub struct WhileNode {
        pub loc: QLLoc,
        pub while_type: WhileType,
        pub cond_expr: SrcExpr,
        pub body: SharedStmtNodePtr,
    }

    #[derive(EquivByLoc)]
    pub struct ForNode {
        pub loc: QLLoc,
        pub init: Option<SrcExpr>,
        pub cond: Option<SrcExpr>,
        pub update: Option<SrcExpr>,
        pub body: SharedStmtNodePtr,
    }

    pub enum CFStruct {
        If(IfNode),
        Switch(SwitchNode),
        While(WhileNode),
        For(ForNode),
    }
}

pub struct FuncSrcTree {
    root: SharedStmtNodePtr,
}

impl FuncSrcTree {
    pub fn new(root: SharedStmtNodePtr) -> Self {
        Self { root }
    }

    pub fn get_root(&self) -> SharedStmtNodePtr {
        Rc::clone(&self.root)
    }

    pub fn iter(&self) -> FuncSrcTreeIter {
        FuncSrcTreeIter {
            cur_ptr_op: Some(Rc::clone(&self.root)),
        }
    }
}

pub struct FuncSrcTreeIter {
    cur_ptr_op: Option<SharedStmtNodePtr>,
}

impl FuncSrcTreeIter {
    pub fn select(&mut self, cf_struct: &CFStruct, exec_node: &ExecFuncNode, exec_idx: &mut usize) {
        unimplemented!()
    }

    fn get_next_child_ptr(
        par_ptr: SharedStmtNodePtr,
        cur_ptr: SharedStmtNodePtr,
    ) -> Option<SharedStmtNodePtr> {
        let par_node = par_ptr.borrow();
        match &par_node.variants {
            StmtNodeVariants::Block(block_node) => {
                let idx = cur_ptr
                    .borrow()
                    .parent_idx_op
                    .expect("Block child must have idx");
                if idx + 1 >= block_node.stmts.len() {
                    None
                } else {
                    Some(Rc::clone(&block_node.stmts[idx + 1]))
                }
            }
            StmtNodeVariants::CFStruct(cf_struct) => match cf_struct {
                CFStruct::Switch(switch_node) => {
                    let cur_node = cur_ptr.borrow();
                    let case_loc = cur_node
                        .parent_case_loc_op
                        .as_ref()
                        .expect("Switch child must have case loc");
                    let case_ptr_vec = switch_node
                        .case_ptr_map
                        .get(case_loc)
                        .expect("Could not find case loc in case_ptr_map");
                    let idx = cur_ptr
                        .borrow()
                        .parent_idx_op
                        .expect("Switch case child must have idx");
                    if idx + 1 >= case_ptr_vec.len() {
                        None
                    } else {
                        Some(Rc::clone(&case_ptr_vec[idx + 1]))
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_next_ptr(&self) -> Result<Option<SharedStmtNodePtr>> {
        let cur_ptr = match &self.cur_ptr_op {
            None => return Ok(None),
            Some(p) => Rc::clone(p),
        };
        let cur_node = cur_ptr.borrow();
        match &cur_node.variants {
            StmtNodeVariants::Block(block_node) => Ok(block_node.get_first_stmt()),
            StmtNodeVariants::CFStruct(_) => {
                bail!("Should not call get_next_ptr on CFStruct node directly")
            }
            StmtNodeVariants::Plain(_) => {
                let mut cur_ptr = cur_ptr.clone();
                let mut par_ptr;
                loop {
                    par_ptr = match &cur_ptr.borrow().parent_ptr_op {
                        None => return Ok(None),
                        Some(wp) => match wp.upgrade() {
                            None => return Ok(None),
                            Some(p) => p,
                        },
                    };
                    if let Some(ptr) = Self::get_next_child_ptr(par_ptr.clone(), cur_ptr.clone()) {
                        return Ok(Some(ptr));
                    }

                    cur_ptr = par_ptr;
                }
            }
        }
    }
}

impl Iterator for FuncSrcTreeIter {
    type Item = SharedStmtNodePtr;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.cur_ptr_op {
            None => None,
            Some(ptr) => {
                let next_ptr = self
                    .get_next_ptr()
                    .unwrap_or_else(|e| panic!("Error getting next ptr: {:?}", e));
                let ret_ptr = Some(Rc::clone(ptr));
                self.cur_ptr_op = next_ptr;
                ret_ptr
            }
        }
    }
}

#[derive(EquivByLoc, Clone)]
pub struct SrcExpr {
    loc: QLLoc,
    func_invoc_vec: Vec<FuncInvoc>,
}

impl SrcExpr {
    pub fn get_expr_str(&self) -> Result<String> {
        self.loc.get_content()
    }

    pub fn get_invoc_by_loc(loc: &QLLoc, func_invoc_map: &FuncInvocMap) -> Vec<FuncInvoc> {
        let file_path = &loc.file_path;
        let invoc_vec = match func_invoc_map.get(file_path) {
            Some(vec) => vec,
            None => return vec![],
        };
        // binary search
        let mut left = 0;
        let mut right = invoc_vec.len() - 1;
        let mut idx: Option<usize> = None;
        while left <= right {
            let mid = (left + right) / 2;
            let invoc_loc = &invoc_vec[mid].loc;

            if loc.contains(invoc_loc) {
                idx = Some(mid);
                break;
            }

            if loc.end_before(&invoc_loc) {
                right = mid - 1;
            } else if loc.start_after(&invoc_loc) {
                left = mid + 1;
            }
        }
        match idx {
            Some(i) => {
                let mut res = vec![];
                // go left
                let mut j = i;
                while j > 0 && loc.contains(&invoc_vec[j - 1].loc) {
                    j -= 1;
                }
                while j < invoc_vec.len() && loc.contains(&invoc_vec[j].loc) {
                    res.push(invoc_vec[j].clone());
                    j += 1;
                }
                res
            }
            None => vec![],
        }
    }

    pub fn from_loc_and_invocs(loc: &QLLoc, func_invoc_map: &FuncInvocMap) -> Self {
        let invoc_vec = Self::get_invoc_by_loc(loc, func_invoc_map);

        Self {
            loc: loc.clone(),
            func_invoc_vec: invoc_vec,
        }
    }
}
