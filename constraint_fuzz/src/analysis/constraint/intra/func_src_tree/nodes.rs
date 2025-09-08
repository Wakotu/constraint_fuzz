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

pub enum StmtNode {
    Block(BlockNode),
    Plain(PlainNode),
    CFStruct(CFStruct),
}

impl StmtNode {
    pub fn create_plain_ptr(entry: &ChildEntry) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::Plain(PlainNode {
            loc: entry.loc.clone(),
        })))
    }

    pub fn create_block_ptr(
        block_stmt: &BlockStmt,
        stmts: Vec<SharedStmtNodePtr>,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::Block(BlockNode {
            loc: block_stmt.loc.clone(),
            block_type: block_stmt.block_type.clone(),
            stmts,
        })))
    }

    pub fn create_if_ptr(
        if_stmt: &IfStmt,
        then_ptr: SharedStmtNodePtr,
        else_ptr: Option<SharedStmtNodePtr>,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::CFStruct(CFStruct::If(
            cf_mod::IfNode {
                loc: if_stmt.loc.clone(),
                cond: if_stmt.cond_loc.clone(),
                then_blk: then_ptr,
                else_blk: else_ptr,
            },
        ))))
    }

    pub fn create_switch_ptr(
        switch_stmt: &SwitchStmt,
        case_ptr_map: CasePtrMap,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::CFStruct(CFStruct::Switch(
            cf_mod::SwitchNode {
                loc: switch_stmt.loc.clone(),
                expr_loc: switch_stmt.expr_loc.clone(),
                case_ptr_map,
            },
        ))))
    }

    pub fn create_while_ptr(
        while_stmt: &WhileStmt,
        body_ptr: SharedStmtNodePtr,
    ) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::CFStruct(CFStruct::While(
            cf_mod::WhileNode {
                loc: while_stmt.loc.clone(),
                while_type: while_stmt.while_type.clone(),
                cond_loc: while_stmt.cond_loc.clone(),
                body: body_ptr,
            },
        ))))
    }

    pub fn create_for_ptr(for_stmt: &ForStmt, body_ptr: SharedStmtNodePtr) -> SharedStmtNodePtr {
        Rc::new(RefCell::new(StmtNode::CFStruct(CFStruct::For(
            cf_mod::ForNode {
                loc: for_stmt.loc.clone(),
                init: for_stmt.init_loc.clone(),
                cond: for_stmt.cond_loc.clone(),
                step: for_stmt.update_loc.clone(),
                body: body_ptr,
            },
        ))))
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
        code_query::switch_query::CaseMap,
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
