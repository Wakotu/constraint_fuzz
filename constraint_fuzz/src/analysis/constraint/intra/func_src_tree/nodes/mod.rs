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
            custom_class_query::VarType,
            func_invoc_query::{FuncInvoc, FuncInvocMap},
            scope_var_query::{FuncScopeMap, SrcVar, StmtScopeMap},
            switch_query::CaseMap,
        },
        nodes::cf_nodes::{CFNode, CasePtrMap, SwitchArm, SwitchNode},
        stmts::{
            BlockStmt, BlockType, ChildEntry, ForStmt, IfStmt, LabelDict, QLLoc, StmtType,
            SwitchStmt, WhileStmt,
        },
    },
    stmt_collect::ProcessUnit,
};

pub mod cf_nodes;

pub enum StmtNodeVariants {
    Block(BlockStmtNode),
    Plain(PlainStmtNode),
    CF(CFNode),
}

pub struct StmtNode {
    /// the field where data is stored
    pub variants: StmtNodeVariants,
    /// parent pointer for non-root nodes
    pub parent_ptr_op: Option<WeakStmtNodePtr>,
    /// index in parent's stmts vec, None for non-block parents
    pub parent_idx_op: Option<usize>,
    /// case label location if this node is under a switch-case
    pub parent_armidx_op: Option<usize>,
    /// valid variables in scope at this statement
    pub valid_var_vec: Vec<SrcVar>,
}

impl StmtNode {
    pub fn is_jump_stmt(&self) -> bool {
        match &self.variants {
            StmtNodeVariants::Plain(plain_node) => {
                matches!(plain_node.stmt_type, StmtType::Break)
                    || matches!(plain_node.stmt_type, StmtType::Continue)
                    || matches!(plain_node.stmt_type, StmtType::Goto)
            }
            _ => false,
        }
    }

    pub fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match &self.variants {
            StmtNodeVariants::Block(block_node) => block_node.src_loc_inner(src_loc),
            StmtNodeVariants::CF(cf_node) => cf_node.src_loc_inner(src_loc),
            StmtNodeVariants::Plain(plain_node) => plain_node.src_loc_inner(src_loc),
        }
    }

    pub fn get_swtich_node(&self) -> Option<&SwitchNode> {
        match &self.variants {
            StmtNodeVariants::CF(cf_node) => match cf_node {
                CFNode::Switch(switch_node) => Some(switch_node),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn update_loopnode_count(&mut self) {
        match &mut self.variants {
            StmtNodeVariants::CF(cf_node) => match cf_node {
                CFNode::While(while_node) => {
                    while_node.count += 1;
                }
                CFNode::For(for_node) => {
                    for_node.count += 1;
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn get_loc(&self) -> &QLLoc {
        match &self.variants {
            StmtNodeVariants::Block(block_node) => &block_node.loc,
            StmtNodeVariants::Plain(plain_node) => &plain_node.loc,
            StmtNodeVariants::CF(cf_struct) => match cf_struct {
                CFNode::If(if_node) => &if_node.loc,
                CFNode::Switch(switch_node) => &switch_node.loc,
                CFNode::While(while_node) => &while_node.loc,
                CFNode::For(for_node) => &for_node.loc,
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

    pub fn is_cf_node(&self) -> bool {
        match self.variants {
            StmtNodeVariants::CF(_) => true,
            _ => false,
        }
    }

    pub fn is_loop_node(&self) -> bool {
        match &self.variants {
            StmtNodeVariants::CF(cf_struct) => {
                matches!(cf_struct, CFNode::While(_)) || matches!(cf_struct, CFNode::For(_))
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
        cur_entry: &ChildEntry,
        // parent_ptr: WeakStmtNodePtr
    ) -> SharedStmtNodePtr {
        let valid_var_vec = match stmt_scope_map.get(&entry.loc) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Plain(PlainStmtNode::new(
                &entry.loc,
                func_invoc_map,
                &cur_entry.stmt_type,
            )),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_armidx_op: None,
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
            parent_armidx_op: None,
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
            variants: StmtNodeVariants::CF(CFNode::If(cf_nodes::IfNode {
                loc: if_stmt.loc.clone(),
                cond_expr: SrcExpr::from_loc_and_invocs(&if_stmt.cond_loc, func_invoc_map),
                then_body: then_ptr,
                else_body_op: else_ptr,
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_armidx_op: None,
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
            variants: StmtNodeVariants::CF(CFNode::Switch(cf_nodes::SwitchNode {
                loc: switch_stmt.loc.clone(),
                expr: SrcExpr::from_loc_and_invocs(&switch_stmt.expr_loc, func_invoc_map),
                arm_vec: SwitchArm::get_vec_from_caseptr_map(case_ptr_map),
            })),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_armidx_op: None,
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
            variants: StmtNodeVariants::CF(CFNode::While(cf_nodes::WhileNode::new(
                &while_stmt.loc,
                &while_stmt.while_type,
                &SrcExpr::from_loc_and_invocs(&while_stmt.cond_loc, func_invoc_map),
                body_ptr,
            ))),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_armidx_op: None,
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
            variants: StmtNodeVariants::CF(CFNode::For(cf_nodes::ForNode::new(
                &for_stmt.loc,
                match &for_stmt.init_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                match &for_stmt.cond_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                match &for_stmt.update_loc {
                    None => None,
                    Some(loc) => Some(SrcExpr::from_loc_and_invocs(loc, func_invoc_map)),
                },
                body_ptr,
            ))),
            parent_ptr_op: None,
            parent_idx_op: None,
            parent_armidx_op: None,
            valid_var_vec,
        }))
    }
}

pub type SharedStmtNodePtr = Rc<RefCell<StmtNode>>;
pub type WeakStmtNodePtr = Weak<RefCell<StmtNode>>;

// pub type PlainStmtNode = SrcExpr;

#[derive(EquivByLoc, Clone)]
pub struct PlainStmtNode {
    pub loc: QLLoc,
    func_invoc_vec: Vec<FuncInvoc>,
    pub stmt_type: StmtType,
}

impl PlainStmtNode {
    pub fn get_goto_label(&self) -> Result<String> {
        const GOTO_PREFIX: &str = "goto ";
        let content = self.loc.get_content()?;
        let lab_name = content[GOTO_PREFIX.len()..content.len() - 1]
            .trim()
            .to_string();
        Ok(lab_name)
    }

    pub fn new(loc: &QLLoc, func_invoc_map: &FuncInvocMap, stmt_type: &StmtType) -> Self {
        let invoc_vec = SrcExpr::get_invoc_by_loc(loc, func_invoc_map);

        Self {
            loc: loc.clone(),
            func_invoc_vec: invoc_vec,
            stmt_type: stmt_type.clone(),
        }
    }

    pub fn get_return_expr(&self) -> Result<Option<String>> {
        match self.stmt_type {
            StmtType::Return => {
                let content = self.loc.get_content()?;
                let start_idx = "return ".len();
                // get rid of starting "return " and ending ";"
                Ok(Some(
                    content[start_idx..content.len() - 1].trim().to_string(),
                ))
            }
            _ => Ok(None),
        }
    }

    fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn act_inner(&self, act: &ExecAction) -> Result<bool> {
        match act {
            ExecAction::Intra(jump_act) => {
                if !self.src_loc_inner(&jump_act.from_loc) && self.src_loc_inner(&jump_act.dest_loc)
                {
                    return Ok(false);
                }

                match &jump_act.jump_variants {
                    JumpActionType::Br { val_loc } => {
                        let flag = self.src_loc_inner(val_loc);
                        if !flag {
                            bail!("Stmt action inner check: br guard value should match but actually not");
                        }
                        Ok(true)
                    }
                    JumpActionType::MergeBr => Ok(true),
                    var => {
                        bail!(
                            "Stmt action inner check: Unsupported jump action type: {:?}",
                            var
                        )
                    }
                }
            }
            ExecAction::UBV(ubv_hit) => Ok(self.src_loc_inner(ubv_hit.get_loc())),
            ExecAction::Select(sel_act) => Ok(self.src_loc_inner(sel_act.get_loc())),
            ExecAction::Func(FuncAction::Call(call_act)) => {
                match &call_act.invoc_loc_op {
                    None => {
                        log::warn!("Stmt action inner check: Function call action without invocation location");
                        Ok(false)
                    }
                    Some(loc) => Ok(self.src_loc_inner(loc)),
                }
            }
            _ => Ok(false),
        }
    }

    // fn match_act_loc_impl(&self, act: &ExecAction) -> bool {
    //     let act_loc = match act.get_match_loc() {
    //         None => return false,
    //         Some(loc) => loc,
    //     };
    //     let loc_ord = match self.loc.compare_src_loc(act_loc) {
    //         None => return false,
    //         Some(o) => o,
    //     };
    //     loc_ord == Ordering::Equal
    // }

    // pub fn match_act_loc(&self, act: &ExecAction) -> Result<bool> {
    //     let act_match = self.match_act_loc_impl(act);
    //     if act_match && !act.plain_stmt_suitable() {
    //         bail!("Statement-Action location match, but action type is not suitable for plain statement");
    //     }
    //     Ok(act_match)
    // }
}

#[derive(EquivByLoc)]
pub struct BlockStmtNode {
    loc: QLLoc,
    block_type: BlockType,
    stmts: Vec<SharedStmtNodePtr>,
}

impl BlockStmtNode {
    fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

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

pub struct FuncSrcTree {
    root: SharedStmtNodePtr,
    valid_var_vec: Vec<SrcVar>,
    pub ret_type: VarType,
    pub func_name: String,
    label_dict: LabelDict,
}

impl FuncSrcTree {
    pub fn get_formal_param_vec(&self) -> &Vec<SrcVar> {
        &self.valid_var_vec
    }
    pub fn new(
        root: SharedStmtNodePtr,
        name: &str,
        func_scope_map: &FuncScopeMap,
        ret_type: VarType,
        label_dict: LabelDict,
    ) -> Self {
        let valid_var_vec = match func_scope_map.get(name) {
            None => vec![],
            Some(var_vec) => var_vec.clone(),
        };
        Self {
            root,
            valid_var_vec,
            ret_type,
            func_name: name.to_string(),
            label_dict,
        }
    }

    pub fn get_root(&self) -> SharedStmtNodePtr {
        Rc::clone(&self.root)
    }

    pub fn iter(&self) -> FuncSrcTreeIter<'_> {
        FuncSrcTreeIter {
            cur_ptr_op: Some(Rc::clone(&self.root)),
            tree: self,
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

pub struct FuncSrcTreeIter<'a> {
    cur_ptr_op: Option<SharedStmtNodePtr>,
    tree: &'a FuncSrcTree,
}

impl<'a> FuncSrcTreeIter<'a> {
    pub fn select(&mut self, cf_struct: &CFNode, exec_node: &ExecFuncNode, exec_idx: &mut usize) {
        // TODO: implement the selection logic
        unimplemented!()
    }

    fn get_next_sibling_ptr_impl(
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
            StmtNodeVariants::CF(cf_struct) => match cf_struct {
                CFNode::Switch(switch_node) => {
                    let cur_node = cur_ptr.borrow();
                    let arm_idx = cur_node
                        .parent_armidx_op
                        .as_ref()
                        .expect("Switch child must have case loc");
                    let arm_body = switch_node
                        .get_arm_body(*arm_idx)
                        .expect("Could not find specified case with given index");
                    let idx = cur_ptr
                        .borrow()
                        .parent_idx_op
                        .expect("Switch case child must have idx");
                    if idx + 1 >= arm_body.len() {
                        None
                    } else {
                        Some(Rc::clone(&arm_body[idx + 1]))
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_after_next_ptr(cur_ptr: SharedStmtNodePtr) -> Result<Option<SharedStmtNodePtr>> {
        let mut cur_ptr = cur_ptr;
        // let mut par_ptr;
        loop {
            // get parent ptr
            let par_ptr = match &cur_ptr.borrow().parent_ptr_op {
                None => return Ok(None),
                Some(wp) => match wp.upgrade() {
                    None => bail!("FuncSrcTree Iter next: Failed to upgrade father ptr"),
                    Some(p) => p,
                },
            };
            // get next sibling ptr
            if let Some(ptr) = Self::get_next_sibling_ptr_impl(par_ptr.clone(), cur_ptr.clone()) {
                return Ok(Some(ptr));
            }

            // No next sibling: stop at loop node or go up
            if par_ptr.borrow().is_loop_node() {
                return Ok(Some(par_ptr));
            }
            // go up
            cur_ptr = par_ptr;
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
            StmtNodeVariants::CF(_) => {
                bail!("Should not call get_next_ptr on CFStruct node directly")
            }
            StmtNodeVariants::Plain(_) => Self::get_after_next_ptr(cur_ptr.clone()),
        }
    }

    pub fn update_in_cf(&mut self, next_ptr_op: Option<SharedStmtNodePtr>) {
        self.cur_ptr_op = next_ptr_op;
    }

    fn update_loop_node_count(stmt_ptr: SharedStmtNodePtr) {
        let mut stmt_node = stmt_ptr.borrow_mut();
        stmt_node.update_loopnode_count();
    }

    fn cur_jump(&self) -> bool {
        match &self.cur_ptr_op {
            None => false,
            Some(ptr) => {
                let stmt_node = ptr.borrow();
                stmt_node.is_jump_stmt()
            }
        }
    }

    fn get_parent_loopptr(&self) -> Result<SharedStmtNodePtr> {
        let mut cur_ptr = match &self.cur_ptr_op {
            None => bail!("FuncSrcTree Iter goup_until_loopnode: Current pointer is None"),
            Some(ptr) => ptr.clone(),
        };
        let par_loop_ptr = loop {
            let par_ptr = match &cur_ptr.borrow().parent_ptr_op {
                None => bail!(
                    "FuncSrcTree Iter goup_until_loopnode: Reached root without finding loop node"
                ),
                Some(wp) => match wp.upgrade() {
                    None => {
                        bail!("FuncSrcTree Iter goup_until_loopnode: Failed to upgrade father ptr")
                    }
                    Some(p) => p,
                },
            };
            if par_ptr.borrow().is_loop_node() {
                break par_ptr;
            }
            cur_ptr = par_ptr;
        };
        Ok(par_loop_ptr)
    }

    /**
     * Named label do not allow emtpry next pointer
     */
    fn get_goto_next_ptr(&self, plain_node: &PlainStmtNode) -> Result<SharedStmtNodePtr> {
        let lab_name = plain_node.get_goto_label()?;
        let lab_ptr = self.tree.label_dict.get(&lab_name).ok_or_else(||{
            eyre::eyre!("Func Src Iter, goto handle: Could not find corresponding label stmt pointer based on label name {}", lab_name)
        })?;
        let next_ptr = match Self::get_after_next_ptr(lab_ptr.clone())? {
            Some(p) => p,
            None => bail!(
                "Func Src Iter, goto handle: Could not find next pointer after label statement"
            ),
        };
        Ok(next_ptr)
    }

    fn jump_next(&mut self) -> Result<()> {
        let next_ptr_op = {
            let stmt_node = match &self.cur_ptr_op {
                None => bail!("FuncSrcTree Iter jump_next: Current pointer is None"),
                Some(ptr) => ptr.borrow(),
            };
            match &stmt_node.variants {
                StmtNodeVariants::Plain(plain_node) => match &plain_node.stmt_type {
                    StmtType::Continue => Some(self.get_parent_loopptr()?),
                    StmtType::Break => {
                        let loop_ptr = self.get_parent_loopptr()?;
                        Self::get_after_next_ptr(loop_ptr)?
                    }
                    StmtType::Goto => Some(self.get_goto_next_ptr(plain_node)?),
                    stmt_var => bail!(
                        "FuncSrcTree Iter jump_next: Current pointer is not a jump statement: {:?}",
                        stmt_var
                    ),
                },
                _ => bail!("FuncSrcTree Iter jump_next: Current pointer is not PlainStmtNode"),
            }
        };
        self.cur_ptr_op = next_ptr_op;
        Ok(())
    }

    pub fn skip_jump_statements(&mut self) -> Result<()> {
        while self.cur_jump() {
            self.jump_next()?;
        }
        Ok(())
    }
}

impl<'a> Iterator for FuncSrcTreeIter<'a> {
    type Item = SharedStmtNodePtr;

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = match &self.cur_ptr_op {
            None => return None,
            Some(p) => p.clone(),
        };

        let is_cf = {
            let stmt_node = ptr.borrow();
            stmt_node.is_cf_node()
        };

        if is_cf {
            // update Loop Node count field
            Self::update_loop_node_count(ptr.clone());

            // just return current ptr and do not modify iterator state
            // Since update logic would be implemented by later action handle
            return Some(ptr.clone());
        }

        // skip all jump statements
        self.skip_jump_statements().unwrap_or_else(|e| {
            panic!("Func Src Iter next: Failed to skip jump statements, {}", e)
        });

        // return current and update current
        let next_ptr = self
            .get_next_ptr()
            .unwrap_or_else(|e| panic!("Error getting next ptr: {:?}", e));
        let ret_ptr = Some(ptr.clone());
        self.cur_ptr_op = next_ptr;
        ret_ptr
    }
}

#[derive(EquivByLoc, Clone)]
pub struct SrcExpr {
    loc: QLLoc,
    func_invoc_vec: Vec<FuncInvoc>,
}

impl SrcExpr {
    fn src_loc_inner(&self, src_loc: &SrcLocEnum) -> bool {
        match self.loc.compare_src_loc(src_loc) {
            None => false,
            Some(ord) => ord == Ordering::Equal,
        }
    }

    pub fn act_inner(&self, act: &ExecAction) -> Result<bool> {
        match act {
            ExecAction::Intra(jump_act) => {
                if !self.src_loc_inner(&jump_act.from_loc) && self.src_loc_inner(&jump_act.dest_loc)
                {
                    return Ok(false);
                }

                match &jump_act.jump_variants {
                    JumpActionType::Br { val_loc } => {
                        let flag = self.src_loc_inner(val_loc);
                        if !flag {
                            bail!("Stmt action inner check: br guard value should match but actually not");
                        }
                        Ok(true)
                    }
                    JumpActionType::MergeBr => Ok(true),
                    var => {
                        bail!(
                            "Stmt action inner check: Unsupported jump action type: {:?}",
                            var
                        )
                    }
                }
            }
            ExecAction::UBV(ubv_hit) => Ok(self.src_loc_inner(ubv_hit.get_loc())),
            ExecAction::Select(sel_act) => Ok(self.src_loc_inner(sel_act.get_loc())),
            ExecAction::Func(FuncAction::Call(call_act)) => {
                match &call_act.invoc_loc_op {
                    None => {
                        log::warn!("Stmt action inner check: Function call action without invocation location");
                        Ok(false)
                    }
                    Some(loc) => Ok(self.src_loc_inner(loc)),
                }
            }
            _ => Ok(false),
        }
    }

    /**
     * Returns (is_inner, is_outer)
     * is_outer only applies to Br or MergeBr actions
     */
    pub fn cond_expr_act_match(&self, act: &ExecAction) -> Result<(bool, bool)> {
        match act {
            ExecAction::Intra(jump_act) => {
                let val_loc_op = match &jump_act.jump_variants {
                    JumpActionType::Br { val_loc } => Some(val_loc),
                    JumpActionType::MergeBr => None,
                    jump_var => bail!(
                        "Cond Expression should not match with jump action type: {:?}",
                        jump_var
                    ),
                };
                let dest_loc = &jump_act.dest_loc;
                let is_inner = match val_loc_op {
                    None => false,
                    Some(val_loc) => self.src_loc_inner(val_loc),
                };
                let is_outer = !self.src_loc_inner(dest_loc);
                Ok((is_inner, is_outer))
            }
            ExecAction::UBV(ubv_hit) => {
                let val_loc = ubv_hit.get_loc();
                let val_inner = self.src_loc_inner(val_loc);
                Ok((val_inner, false))
            }
            ExecAction::Select(sel_act) => {
                let val_loc = sel_act.get_loc();
                let val_inner = self.src_loc_inner(val_loc);
                Ok((val_inner, false))
            }
            ExecAction::Func(FuncAction::Call(call_act)) => {
                match &call_act.invoc_loc_op {
                    None => {
                        log::warn!("Stmt action inner check: Function call action without invocation location");
                        Ok((false, false))
                    }
                    Some(loc) => Ok((self.src_loc_inner(loc), false)),
                }
            }
            act => bail!(
                "Cond Expression should not match with action type: {:?}",
                act
            ),
        }
    }

    pub fn get_loc(&self) -> &QLLoc {
        &self.loc
    }

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
