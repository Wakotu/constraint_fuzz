use core::panic;
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashSet,
    process::Child,
    rc::{Rc, Weak},
};

use color_eyre::eyre::Result;
use eyre::bail;
use my_macros::EquivByLoc;

use crate::analysis::constraint::{
    inter::{
        exec_tree::{
            action::{ExecAction, FuncAction, JumpAction, JumpActionType, LoopAction},
            thread_tree::ExecFuncNode,
        },
        loc::SrcLocEnum,
    },
    intra::func_src_tree::{
        code_query::{
            func_invoc_query::{FuncInvoc, FuncInvocMap},
            scope_var_query::{FuncScopeMap, SrcVar, StmtScopeMap},
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
    /// valid variables in scope at this statement
    pub valid_var_vec: Vec<SrcVar>,
}

impl StmtNode {
    pub fn get_loc(&self) -> &QLLoc {
        match &self.variants {
            StmtNodeVariants::Block(block_node) => &block_node.loc,
            StmtNodeVariants::Plain(plain_node) => &plain_node.loc,
            StmtNodeVariants::CFStruct(cf_struct) => match cf_struct {
                CFStruct::If(if_node) => &if_node.loc,
                CFStruct::Switch(switch_node) => &switch_node.loc,
                CFStruct::While(while_node) => &while_node.loc,
                CFStruct::For(for_node) => &for_node.loc,
            },
        }
    }

    pub fn get_parent_ptr(&self) -> Option<SharedStmtNodePtr> {
        match &self.parent_ptr_op {
            None => None,
            Some(wp) => Some(
                wp.upgrade()
                    .expect("Stmt Node Pointer: Parent pointer has been dropped"),
            ),
        }
    }

    pub fn is_loop_node(&self) -> bool {
        match &self.variants {
            StmtNodeVariants::CFStruct(cf_struct) => {
                matches!(cf_struct, CFStruct::While(_)) || matches!(cf_struct, CFStruct::For(_))
            }
            _ => false,
        }
    }

    /**
     * Default Pointer Creation
     */

    pub fn create_plain_ptr(
        entry: &ChildEntry,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
        // parent_ptr: WeakStmtNodePtr
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&entry.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Plain(PlainStmtNode::from_loc_and_invocs(
                &entry.loc,
                func_invoc_map,
            )),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
            valid_var_vec,
        }))
    }

    pub fn create_block_ptr(
        block_stmt: &BlockStmt,
        stmts: Vec<SharedStmtNodePtr>,
        stmt_scope_map: &StmtScopeMap,
        // parent_ptr: Option<WeakStmtNodePtr>,
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&block_stmt.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Block(BlockStmtNode {
                loc: block_stmt.loc.clone(),
                block_type: block_stmt.block_type.clone(),
                stmts,
            }),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
            valid_var_vec,
        }))
    }

    pub fn create_if_ptr(
        if_stmt: &IfStmt,
        then_ptr: SharedStmtNodePtr,
        else_ptr: Option<SharedStmtNodePtr>,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&if_stmt.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
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
            valid_var_vec,
        }))
    }

    pub fn create_switch_ptr(
        switch_stmt: &SwitchStmt,
        case_ptr_map: CasePtrMap,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&switch_stmt.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::Switch(cf_mod::SwitchNode {
                loc: switch_stmt.loc.clone(),
                expr_loc: SrcExpr::from_loc_and_invocs(&switch_stmt.expr_loc, func_invoc_map),
                case_ptr_map,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_case_loc_op: None,
            valid_var_vec,
        }))
    }

    pub fn create_while_ptr(
        while_stmt: &WhileStmt,
        body_ptr: SharedStmtNodePtr,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&while_stmt.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
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
            valid_var_vec,
        }))
    }

    pub fn create_for_ptr(
        for_stmt: &ForStmt,
        body_ptr: SharedStmtNodePtr,
        func_invoc_map: &FuncInvocMap,
        stmt_scope_map: &StmtScopeMap,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&for_stmt.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
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
            valid_var_vec,
        }))
    }
}

pub type SharedStmtNodePtr = Rc<RefCell<StmtNode>>;
pub type WeakStmtNodePtr = Weak<RefCell<StmtNode>>;

// pub type PlainStmtNode = SrcExpr;

#[derive(EquivByLoc, Clone)]
pub struct PlainStmtNode {
    loc: QLLoc,
    func_invoc_vec: Vec<FuncInvoc>,
}

impl PlainStmtNode {
    pub fn from_loc_and_invocs(loc: &QLLoc, func_invoc_map: &FuncInvocMap) -> Self {
        let invoc_vec = SrcExpr::get_invoc_by_loc(loc, func_invoc_map);

        Self {
            loc: loc.clone(),
            func_invoc_vec: invoc_vec,
        }
    }

    pub fn compare_src_loc(&self, src_loc: &SrcLocEnum) -> Option<Ordering> {
        match self.loc.compare_src_loc(src_loc) {
            Some(ord) => Some(ord),
            None => {
                log::warn!(
                    "Function Unwind Action location is not comparable with statement location"
                );
                None
            }
        }
    }

    fn match_act_loc_impl(&self, act: &ExecAction) -> bool {
        let act_loc = match act.get_match_loc() {
            None => return false,
            Some(loc) => loc,
        };
        let loc_ord = match self.compare_src_loc(act_loc) {
            None => return false,
            Some(o) => o,
        };
        loc_ord == Ordering::Equal
    }

    pub fn match_act_loc(&self, act: &ExecAction) -> Result<bool> {
        let act_match = self.match_act_loc_impl(act);
        if act_match && !act.plain_stmt_suitable() {
            bail!("Statement-Action location match, but action type is not suitable for plain statement");
        }
        Ok(act_match)
    }
}

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
    valid_var_vec: Vec<SrcVar>,
}

impl FuncSrcTree {
    pub fn get_formal_param_vec(&self) -> &Vec<SrcVar> {
        &self.valid_var_vec
    }
    pub fn new(root: SharedStmtNodePtr, name: &str, func_scope_map: &FuncScopeMap) -> Self {
        let valid_var_vec = match func_scope_map.get(name) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Self {
            root,
            valid_var_vec,
        }
    }

    pub fn get_root(&self) -> SharedStmtNodePtr {
        Rc::clone(&self.root)
    }

    pub fn iter(&self) -> FuncSrcTreeIter {
        FuncSrcTreeIter {
            cur_ptr_op: Some(Rc::clone(&self.root)),
        }
    }

    pub fn get_valid_var_vec_for_stmt(&self, stmt_ptr: SharedStmtNodePtr) -> Vec<SrcVar> {
        let mut name_set: HashSet<String> = HashSet::new();
        let mut ptr = Some(stmt_ptr);
        let mut var_vec = vec![];
        loop {
            match ptr {
                None => break,
                Some(p) => {
                    let node = p.borrow();
                    for var in &node.valid_var_vec {
                        if !name_set.contains(&var.name) {
                            var_vec.push(var.clone());
                            name_set.insert(var.name.clone());
                        }
                    }
                    ptr = node.get_parent_ptr();
                }
            }
        }
        var_vec
    }
}

pub struct FuncSrcTreeIter {
    cur_ptr_op: Option<SharedStmtNodePtr>,
}

impl FuncSrcTreeIter {
    pub fn select(&mut self, cf_struct: &CFStruct, exec_node: &ExecFuncNode, exec_idx: &mut usize) {
        // TODO: implement the selection logic
        unimplemented!()
    }

    fn get_next_sibling_ptr(
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
                    // get next sibling ptr
                    par_ptr = match &cur_ptr.borrow().parent_ptr_op {
                        None => return Ok(None),
                        Some(wp) => match wp.upgrade() {
                            None => return Ok(None),
                            Some(p) => p,
                        },
                    };
                    if let Some(ptr) = Self::get_next_sibling_ptr(par_ptr.clone(), cur_ptr.clone())
                    {
                        return Ok(Some(ptr));
                    }

                    // No next sibling: stop at loop node or go up
                    if par_ptr.borrow().is_loop_node() {
                        return Ok(Some(par_ptr));
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
