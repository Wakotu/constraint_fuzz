use std::{
    cell::RefCell,
    process::Child,
    rc::{Rc, Weak},
};

use my_macros::EquivByLoc;

use crate::analysis::constraint::intra::func_src_tree::{
    code_query::switch_query::CaseMap,
    nodes::cf_mod::{CFStruct, CasePtrMap},
    stmts::{BlockStmt, BlockType, ChildEntry, ForStmt, IfStmt, QLLoc, SwitchStmt, WhileStmt},
};

pub enum StmtNodeVariants {
    Block(BlockNode),
    Plain(PlainNode),
    CFStruct(CFStruct),
}

pub struct StmtNode {
    /// the field where data is stored
    pub variants: StmtNodeVariants,
    /// parent pointer for non-root nodes
    pub parent_ptr: Option<WeakStmtNodePtr>,
    /// index in parent's stmts vec, None for non-block parents
    pub parent_idx: Option<usize>,
    /// case label location if this node is under a switch-case
    pub parent_case_loc: Option<QLLoc>,
}

impl StmtNode {
    /**
     * Default Pointer Creation
     */

    pub fn create_plain_ptr(
        entry: &ChildEntry,
        // parent_ptr: WeakStmtNodePtr
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Plain(PlainNode {
                loc: entry.loc.clone(),
            }),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }

    pub fn create_block_ptr(
        block_stmt: &BlockStmt,
        stmts: Vec<SharedStmtNodePtr>,
        // parent_ptr: Option<WeakStmtNodePtr>,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::Block(BlockNode {
                loc: block_stmt.loc.clone(),
                block_type: block_stmt.block_type.clone(),
                stmts,
            }),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }

    pub fn create_if_ptr(
        if_stmt: &IfStmt,
        then_ptr: SharedStmtNodePtr,
        else_ptr: Option<SharedStmtNodePtr>,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::If(cf_mod::IfNode {
                loc: if_stmt.loc.clone(),
                cond: if_stmt.cond_loc.clone(),
                then_blk: then_ptr,
                else_blk: else_ptr,
            })),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }

    pub fn create_switch_ptr(
        switch_stmt: &SwitchStmt,
        case_ptr_map: CasePtrMap,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::Switch(cf_mod::SwitchNode {
                loc: switch_stmt.loc.clone(),
                expr_loc: switch_stmt.expr_loc.clone(),
                case_ptr_map,
            })),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }

    pub fn create_while_ptr(
        while_stmt: &WhileStmt,
        body_ptr: SharedStmtNodePtr,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::While(cf_mod::WhileNode {
                loc: while_stmt.loc.clone(),
                while_type: while_stmt.while_type.clone(),
                cond_loc: while_stmt.cond_loc.clone(),
                body: body_ptr,
            })),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }

    pub fn create_for_ptr(
        for_stmt: &ForStmt,
        body_ptr: SharedStmtNodePtr,
        // parent_ptr: WeakStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode {
            variants: StmtNodeVariants::CFStruct(CFStruct::For(cf_mod::ForNode {
                loc: for_stmt.loc.clone(),
                init: for_stmt.init_loc.clone(),
                cond: for_stmt.cond_loc.clone(),
                step: for_stmt.update_loc.clone(),
                body: body_ptr,
            })),
            parent_ptr: None,
            parent_idx: None,
            parent_case_loc: None,
        }))
    }
}

pub type SharedStmtNodePtr = Rc<RefCell<StmtNode>>;
pub type WeakStmtNodePtr = Weak<RefCell<StmtNode>>;

#[derive(EquivByLoc)]
pub struct PlainNode {
    loc: QLLoc,
}

#[derive(EquivByLoc)]
pub struct BlockNode {
    loc: QLLoc,
    block_type: BlockType,
    stmts: Vec<SharedStmtNodePtr>,
}

pub mod cf_mod {

    use std::collections::HashMap;

    use my_macros::EquivByLoc;

    use crate::analysis::constraint::intra::func_src_tree::{
        nodes::SharedStmtNodePtr,
        stmts::{QLLoc, WhileType},
    };
    #[derive(EquivByLoc)]
    pub struct IfNode {
        pub loc: QLLoc,
        pub cond: QLLoc,
        pub then_blk: SharedStmtNodePtr,
        pub else_blk: Option<SharedStmtNodePtr>,
    }

    pub type CasePtrMap = HashMap<QLLoc, Vec<SharedStmtNodePtr>>;

    #[derive(EquivByLoc)]
    pub struct SwitchNode {
        pub loc: QLLoc,
        pub expr_loc: QLLoc,
        pub case_ptr_map: CasePtrMap,
    }

    #[derive(EquivByLoc)]
    pub struct WhileNode {
        pub loc: QLLoc,
        pub while_type: WhileType,
        pub cond_loc: QLLoc,
        pub body: SharedStmtNodePtr,
    }

    #[derive(EquivByLoc)]
    pub struct ForNode {
        pub loc: QLLoc,
        pub init: Option<QLLoc>,
        pub cond: Option<QLLoc>,
        pub step: Option<QLLoc>,
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
}
