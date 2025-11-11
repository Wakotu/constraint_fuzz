use crate::analysis::constraint::{
    inter::exec_tree::thread_tree::Tid,
    intra::func_src_tree::{code_query::scope_var_query::SrcVar, nodes::SharedStmtNodePtr},
    stmt_collect::path_collect::inner_stmt::ArgExpr,
};

pub mod path_collect;

pub mod runtime_path;

pub type StmtStr = String;

pub enum ExprPuVariant {
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

pub enum ProcessUnit {
    Thread(ThreadPu),
    Expr(ExprPu),
}

impl std::fmt::Display for ProcessUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessUnit::Thread(thread_pu) => write!(f, "ThreadPu: TID {}", thread_pu.thread_id),
            ProcessUnit::Expr(expr_pu) => write!(f, "ExprPu: {}", expr_pu),
        }
    }
}

impl ProcessUnit {
    pub fn new_thread_pu(thread_id: Tid) -> Self {
        ProcessUnit::Thread(ThreadPu { thread_id })
    }

    pub fn get_exprpu_ref(&self) -> Option<&ExprPu> {
        match self {
            ProcessUnit::Expr(expr_pu) => Some(expr_pu),
            _ => None,
        }
    }

    pub fn get_exprpu(self) -> Option<ExprPu> {
        match self {
            ProcessUnit::Expr(expr_pu) => Some(expr_pu),
            _ => None,
        }
    }

    pub fn from_exprpu(expr_pu: ExprPu) -> Self {
        ProcessUnit::Expr(expr_pu)
    }

    pub fn create_plain_pu(content: String, valid_var_vec: Vec<SrcVar>) -> Self {
        let expr_pu = ExprPu::create_plain_pu(content, valid_var_vec);
        ProcessUnit::Expr(expr_pu)
    }

    pub fn create_pre_func_assign_pu(arg_expr: &ArgExpr, param_var: &SrcVar) -> Self {
        let expr_pu = ExprPu::create_pre_func_assign_pu(arg_expr, param_var);
        ProcessUnit::Expr(expr_pu)
    }

    pub fn create_plain_pu_with_cond_recs(
        content: String,
        valid_var_vec: Vec<SrcVar>,
        cond_recs: &Vec<InnerCondRec>,
    ) -> Self {
        let expr_pu = ExprPu::create_plain_pu_with_cond_recs(content, valid_var_vec, cond_recs);
        ProcessUnit::Expr(expr_pu)
    }

    pub fn create_ret_assign_pu(
        ret_expr: &ExprPu,
        ret_var: &SrcVar,
        ret_stmt_ptr: SharedStmtNodePtr,
    ) -> Self {
        let expr_pu = ExprPu::create_ret_assign_pu(ret_expr, ret_var, ret_stmt_ptr);
        ProcessUnit::Expr(expr_pu)
    }
}

pub struct ThreadPu {
    pub thread_id: Tid,
}

pub struct ExprPu {
    pub content: String,
    pub valid_var_vec: Vec<SrcVar>,
    pub cond_rec_vec: Vec<InnerCondRec>,
    pub variants: ExprPuVariant,
}

impl std::fmt::Display for ExprPu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

impl ExprPu {
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
            variants: ExprPuVariant::Plain {},
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
            variants: ExprPuVariant::Plain {},
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
            variants: ExprPuVariant::Plain {},
        }
    }

    pub fn create_ret_assign_pu(
        ret_expr: &ExprPu,
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
            variants: ExprPuVariant::Plain {},
        }
    }

    pub fn concat_cond_pu(expr_pu: ExprPu, val_lit: String) -> Self {
        let mut pu = expr_pu;
        let cond_str = format!(" == {}", val_lit);
        pu.content.push_str(&cond_str);
        pu.variants = ExprPuVariant::CondExpr { val: true };
        pu
    }
}
