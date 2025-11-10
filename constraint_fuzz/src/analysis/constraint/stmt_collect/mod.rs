use crate::analysis::constraint::{
    intra::func_src_tree::{code_query::scope_var_query::SrcVar, nodes::SharedStmtNodePtr},
    stmt_collect::path_collect::inner_stmt::ArgExpr,
};

pub mod path_collect;

pub mod runtime_path;

pub type StmtStr = String;

pub enum ProcessUnitVariant {
    Plain {},
    CondExpr { val: bool },
}

#[derive(Clone, PartialEq, Eq)]
pub struct InnerCondRec {
    inner_idx: usize,
    cond_val: bool,
}

impl InnerCondRec {
    pub fn before(&self, loc: usize) -> bool {
        self.inner_idx < loc
    }

    pub fn before_or_eq(&self, loc: usize) -> bool {
        self.inner_idx <= loc
    }

    pub fn derive_minus(&self, loc: usize) -> Self {
        Self {
            inner_idx: self.inner_idx - loc,
            cond_val: self.cond_val,
        }
    }

    pub fn derive_plus(&self, loc: usize) -> Self {
        Self {
            inner_idx: self.inner_idx + loc,
            cond_val: self.cond_val,
        }
    }
}

impl PartialOrd for InnerCondRec {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.inner_idx.partial_cmp(&other.inner_idx)
    }
}

impl Ord for InnerCondRec {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner_idx.cmp(&other.inner_idx)
    }
}

pub struct ProcessUnit {
    pub content: String,
    pub valid_var_vec: Vec<SrcVar>,
    pub cond_rec_vec: Vec<InnerCondRec>,
    pub variants: ProcessUnitVariant,
}

impl std::fmt::Display for ProcessUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

impl ProcessUnit {
    /**
     * Construction Related
     */

    /**
     * Plain means no extra information
     */
    pub fn create_plain_pu(content: String, valid_var_vec: Vec<SrcVar>) -> Self {
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: vec![],
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_pre_func_assign_pu(arg_expr: &ArgExpr, param_var: &SrcVar) -> Self {
        let param_var_str = param_var.var_name_str();
        let pre_off = param_var_str.len() + 3;
        let assign_str = format!("{} = {};", param_var.var_name_str(), arg_expr.expr_str);

        let mut var_vec = arg_expr.var_vec.clone();
        var_vec.push(param_var.clone());

        let cond_vec = arg_expr.derive_cond_vec(pre_off);

        Self {
            content: assign_str,
            valid_var_vec: var_vec,
            cond_rec_vec: cond_vec,
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_plain_pu_with_cond_recs(
        content: String,
        valid_var_vec: Vec<SrcVar>,
        cond_recs: &Vec<InnerCondRec>,
    ) -> Self {
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: cond_recs.to_vec(),
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn create_ret_assign_pu(
        ret_expr: &ProcessUnit,
        ret_var: &SrcVar,
        ret_stmt_ptr: SharedStmtNodePtr,
    ) -> Self {
        let content = format!("{} = {};", ret_var.name, ret_expr);
        let mut valid_var_vec = SrcVar::get_live_var(ret_stmt_ptr);
        valid_var_vec.push(ret_var.clone());
        Self {
            content,
            valid_var_vec,
            cond_rec_vec: vec![],
            variants: ProcessUnitVariant::Plain {},
        }
    }

    pub fn concat_cond_pu(expr_pu: ProcessUnit, val_lit: String) -> Self {
        let mut pu = expr_pu;
        let cond_str = format!(" == {}", val_lit);
        pu.content.push_str(&cond_str);
        pu.variants = ProcessUnitVariant::CondExpr { val: true };
        pu
    }
}
