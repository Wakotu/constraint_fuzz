use std::{cell::RefCell, rc::Rc, rc::Weak};

use my_macros::EquivByLoc;

use crate::analysis::constraint::intra::func_src_tree::{nodes::cf_mod::CFStruct, stmts::QLLoc};

pub enum StmtNode {
    Block(BlockNode),
    Plain(PlainNode),
    CFStruct(CFStruct),
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
    stmts: Vec<SharedStmtNodePtr>,
}

pub mod cf_mod {

    use my_macros::EquivByLoc;

    use crate::analysis::constraint::intra::func_src_tree::{
        code_query::switch_query::CaseMap,
        nodes::SharedStmtNodePtr,
        stmts::{QLLoc, WhileType},
    };
    #[derive(EquivByLoc)]
    pub struct IfNode {
        loc: QLLoc,
        cond: QLLoc,
        then_blk: SharedStmtNodePtr,
        else_blk: Option<SharedStmtNodePtr>,
    }

    #[derive(EquivByLoc)]
    pub struct SwitchNode {
        loc: QLLoc,
        cond: QLLoc,
        case_map: CaseMap,
    }

    #[derive(EquivByLoc)]
    pub struct WhileNode {
        loc: QLLoc,
        while_type: WhileType,
        cond: QLLoc,
        body: SharedStmtNodePtr,
    }

    #[derive(EquivByLoc)]
    pub struct ForNode {
        loc: QLLoc,
        init: Option<QLLoc>,
        cond: QLLoc,
        step: QLLoc,
        body: SharedStmtNodePtr,
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
