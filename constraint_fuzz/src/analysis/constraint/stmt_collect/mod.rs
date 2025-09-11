use crate::{
    analysis::constraint::{
        inter::exec_tree::{action::ExecAction, thread_tree::SharedFuncNodePtr, ExecForest},
        intra::func_src_tree::{
            builder::FuncSrcForest,
            nodes::{FuncSrcTree, StmtNodeVariants},
        },
    },
    feedback::branches::constraints::UBConstraint,
};

use color_eyre::eyre::Result;
use eyre::bail;

pub type StmtStr = String;

pub struct StmtCollector<'a> {
    exec_forest: &'a ExecForest,
    func_src_forest: &'a FuncSrcForest,
    ub_cons: &'a UBConstraint,
}

impl<'a> StmtCollector<'a> {
    pub fn new(
        exec_forest: &'a ExecForest,
        func_src_forest: &'a FuncSrcForest,
        ub_cons: &'a UBConstraint,
    ) -> Self {
        Self {
            exec_forest,
            func_src_forest,
            ub_cons,
        }
    }

    fn collect_intra(
        &self,
        src_tree: &FuncSrcTree,
        exec_node_ptr: SharedFuncNodePtr,
    ) -> Result<Vec<StmtStr>> {
        let exec_func = exec_node_ptr.borrow();
        let mut exec_idx: usize = 0;
        let mut stmts = Vec::new();

        let mut iter = src_tree.iter();
        let mut stmt_ptr_op;
        loop {
            // iteration logic
            stmt_ptr_op = iter.next();
            let stmt_ptr = match stmt_ptr_op {
                Some(ptr) => ptr,
                None => break,
            };
            let stmt_node = stmt_ptr.borrow();

            match &stmt_node.variants {
                StmtNodeVariants::Block(_) => continue,
                StmtNodeVariants::CFStruct(cf_struct) => {
                    // TODO: core logic: need to invoke `iter.select()` here
                }
                StmtNodeVariants::Plain(plain_stmt) => {
                    // handle function imigrate and plain string collection.
                    let stmt_str = plain_stmt.get_expr_str()?;
                    stmts.push(stmt_str);
                }
            }
        }

        Ok(stmts)
    }

    fn collect_recur(&self, func_node_ptr: SharedFuncNodePtr) -> Result<Vec<StmtStr>> {
        let func_node = func_node_ptr.borrow();
        if func_node.is_init() {
            assert!(
                func_node.data.len() == 1,
                "Init node should have only one action"
            );
            let exec_act = &func_node.data[0];
            match exec_act {
                ExecAction::Func(func_act) => {
                    let child_ptr = func_act
                        .get_child_ptr()
                        .ok_or_else(|| eyre::eyre!("Init Func action should have a child node"))?;
                    return self.collect_recur(child_ptr);
                }
                _ => {
                    bail!("Init node should have only one Func action");
                }
            }
        }

        let func_name = func_node.get_func_name().ok_or_else(|| {
            eyre::eyre!("Function node should have a function name, but got None")
        })?;

        let src_tree = self.func_src_forest.get_value(func_name).ok_or_else(|| {
            eyre::eyre!(
                "Function source tree not found for function: {}. Available functions: {:?}",
                func_name,
                self.func_src_forest.get_all_func_names()
            )
        })?;

        self.collect_intra(src_tree, func_node_ptr)
    }

    pub fn collect(&self) -> Result<Vec<StmtStr>> {
        let root_ptr = self.exec_forest.get_main_root_ptr();
        self.collect_recur(root_ptr)
    }
}
