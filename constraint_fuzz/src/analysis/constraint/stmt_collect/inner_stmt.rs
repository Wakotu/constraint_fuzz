use crate::analysis::constraint::inter::exec_tree::action::{
    ExecAction, FuncAction, FuncCallAction, JumpActionType,
};
use crate::analysis::constraint::inter::loc::{SrcLocEnum, ValidSrcLoc};
use crate::analysis::constraint::intra::func_src_tree::nodes::SharedStmtNodePtr;
use crate::analysis::constraint::intra::func_src_tree::{
    code_query::scope_var_query::SrcVar, stmts::QLLoc,
};
use crate::analysis::constraint::stmt_collect::{ProcessUnit, StmtCollector};

use color_eyre::eyre::Result;
use eyre::bail;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(PartialEq, Eq)]
pub struct InvocSubstOpr {
    pub start_idx: usize,
    // inclusive
    pub end_idx: usize,
    pub ret_var_op: Option<SrcVar>,
}

impl PartialOrd for InvocSubstOpr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.start_idx.cmp(&other.start_idx) {
            Ordering::Equal => Some(self.end_idx.cmp(&other.end_idx)),
            ord => Some(ord),
        }
    }
}

impl Ord for InvocSubstOpr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl InvocSubstOpr {
    pub fn get_ret_str(&self) -> String {
        match &self.ret_var_op {
            None => "".to_string(),
            Some(var) => var.var_name_str(),
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct InvocSubstOprState {
    opr: InvocSubstOpr,
    covered: bool,
}

impl PartialOrd for InvocSubstOprState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.opr.partial_cmp(&other.opr)
    }
}

impl Ord for InvocSubstOprState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.opr.cmp(&other.opr)
    }
}

pub struct SubstRecord {
    data: Vec<InvocSubstOprState>,
}

impl SubstRecord {
    pub fn new() -> Self {
        Self { data: vec![] }
    }

    fn push(&mut self, opr: InvocSubstOpr) {
        self.data.push(InvocSubstOprState {
            opr,
            covered: false,
        });
        self.data.sort();
    }

    pub fn add(&mut self, start_idx: usize, end_idx: usize, ret_var: Option<SrcVar>) {
        let opr = InvocSubstOpr {
            start_idx,
            end_idx,
            ret_var_op: ret_var,
        };
        self.push(opr);
    }

    pub fn next_on_loc(&self, loc: usize) -> Result<usize> {
        for (idx, rec) in self.data.iter().enumerate() {
            if rec.covered {
                continue;
            }
            if rec.opr.start_idx >= loc {
                return Ok(idx);
            }
        }
        bail!("SubstRecord Next on loc: No next operator found");
    }

    pub fn is_start(&self, loc: usize, idx: usize) -> Result<bool> {
        let state = match self.data.get(idx) {
            Some(s) => s,
            None => return Ok(false),
        };
        if state.covered {
            bail!("SubstRecord Is start: Operator already covered");
        }
        Ok(state.opr.start_idx == loc)
    }

    fn is_covered(&self, idx: usize) -> bool {
        let state = match self.data.get(idx) {
            Some(s) => s,
            // false if
            None => return false,
        };
        state.covered
    }

    pub fn next_on_idx(&mut self, idx: usize, update: bool) -> Result<usize> {
        if update {
            let sub_state = match self.data.get_mut(idx) {
                Some(s) => s,
                None => bail!("SubstRecord Next on idx: Invalid index"),
            };
            sub_state.covered = true;
        }
        let mut next_idx = idx + 1;
        while self.is_covered(next_idx) {
            next_idx += 1;
        }
        Ok(next_idx)
    }
}

pub struct ArgExpr {
    expr_str: String,
    var_vec: Vec<SrcVar>,
}

impl ArgExpr {
    pub fn from_arg_seg(
        arg_seg: &str,
        live_var_vec: &Vec<SrcVar>,
        ret_var_vec: &Vec<SrcVar>,
    ) -> Self {
        let mut var_vec = ret_var_vec.clone();
        for var in live_var_vec.iter() {
            if arg_seg.contains(&var.name) {
                var_vec.push(var.clone());
            }
        }
        Self {
            expr_str: arg_seg.to_string(),
            var_vec,
        }
    }
}

// TODO: add methods
pub struct InnerStmtHandler<'a> {
    stmt_info: StmtStrInfo,
    subst_recs: SubstRecord,
    stmt_ptr: SharedStmtNodePtr,
    collector: &'a StmtCollector<'a>,
    live_var_vec: Vec<SrcVar>,
    // result field
    pu_vec: Vec<ProcessUnit>,
}

impl<'a> InnerStmtHandler<'a> {
    // construction method
    pub fn new(stmt_ptr: SharedStmtNodePtr, collector: &'a StmtCollector) -> Result<Self> {
        let stmt_node = stmt_ptr.borrow();
        let stmt_loc = stmt_node.get_loc();
        let stmt_info = StmtStrInfo::from_qlloc(stmt_loc)?;
        let live_var_vec = SrcVar::get_live_var(stmt_ptr.clone());

        Ok(Self {
            stmt_info,
            subst_recs: SubstRecord::new(),
            stmt_ptr: stmt_ptr.clone(),
            collector,
            live_var_vec,
            pu_vec: vec![],
        })
    }

    fn arg_expr_collect(&mut self, left_idx: usize) -> Result<(Vec<ArgExpr>, usize)> {
        let mut left_loc = left_idx + 1;
        let mut arg_expr_vec = vec![];
        let mut next_sub_idx = self.subst_recs.next_on_loc(left_loc)?;

        let right_loc = loop {
            let mut loc = left_loc;
            let mut arg_seg = String::new();
            let mut ret_var_vec: Vec<SrcVar> = vec![];
            while self.stmt_info.byte_at(loc)? != b',' && self.stmt_info.byte_at(loc)? != b')' {
                if self.subst_recs.is_start(loc, next_sub_idx)? {
                    let sub_state = &self.subst_recs.data[next_sub_idx];
                    // ret_var
                    if let Some(var) = &sub_state.opr.ret_var_op {
                        ret_var_vec.push(var.clone());
                    }
                    // update loc
                    loc = sub_state.opr.end_idx + 1;
                    // arg seg update
                    arg_seg.push_str(&sub_state.opr.get_ret_str());
                    // update idx
                    next_sub_idx = self.subst_recs.next_on_idx(next_sub_idx, true)?;
                } else {
                    arg_seg.push(self.stmt_info.byte_at(loc)? as char);
                    loc += 1;
                }
            }

            let arg_expr = ArgExpr::from_arg_seg(&arg_seg, &self.live_var_vec, &ret_var_vec);
            arg_expr_vec.push(arg_expr);

            if self.stmt_info.byte_at(loc)? == b')' {
                break loc;
            } else {
                left_loc = loc + 1;
            }
        };

        Ok((arg_expr_vec, right_loc))
    }

    fn pre_assign_stmts_construct(
        &mut self,
        left_idx: usize,
        func_name: &str,
    ) -> Result<(Vec<ProcessUnit>, usize)> {
        let (arg_expr_vec, right_idx) = self.arg_expr_collect(left_idx)?;

        let param_var_vec = {
            let called_func_tree = self.collector.get_src_func_tree(func_name)?;
            called_func_tree.get_formal_param_vec()
        };

        assert!(
            arg_expr_vec.len() == param_var_vec.len(),
            "Pre Assign Stmt Construction: arg expression length not match with param var length"
        );

        let mut pu_vec: Vec<ProcessUnit> = vec![];
        for (arg_expr, param_var) in arg_expr_vec.iter().zip(param_var_vec.iter()) {
            let assign_str = format!("{} = {};", param_var.var_name_str(), arg_expr.expr_str);
            let mut var_vec = arg_expr.var_vec.clone();
            var_vec.push(param_var.clone());
            let pu = ProcessUnit::create_plain_pu(assign_str, var_vec);
            pu_vec.push(pu);
        }

        Ok((pu_vec, right_idx))
    }

    fn func_invoc_handle(&mut self, call_act: &FuncCallAction) -> Result<()> {
        let (start_idx, left_idx) = self.stmt_info.get_start_idxs(call_act)?;
        let (pre_pu_vec, right_idx) =
            self.pre_assign_stmts_construct(left_idx, &call_act.func_name)?;
        self.pu_vec.extend(pre_pu_vec);

        let called_func_tree = self.collector.get_src_func_tree(&call_act.func_name)?;
        let (func_pu_vec, ret_var_op) = self
            .collector
            .collect_intra(called_func_tree, call_act.child_ptr.clone())?;
        self.pu_vec.extend(func_pu_vec);

        // update subst recs
        self.subst_recs.add(start_idx, right_idx, ret_var_op);
        Ok(())
    }

    // one act handle interface
    pub fn act_handle(&mut self, act: &ExecAction) -> Result<()> {
        match act {
            // handle func invocation
            ExecAction::Func(func_act) => match func_act {
                FuncAction::Call(call_act) => self.func_invoc_handle(call_act),
                FuncAction::Unwind { loc } => {
                    todo!()
                }
                _ => {
                    bail!("Unexpected Func action: {:?}", func_act);
                }
            },

            ExecAction::Intra(jump_act) => match &jump_act.jump_variants {
                JumpActionType::BrGuard { val_loc } => {
                    todo!()
                }
                JumpActionType::MergeBrGuard => {
                    todo!()
                }
                jump_act_type => bail!(
                    "Stmt Action handle: Unexpected jump action type: {:?}",
                    jump_act_type
                ),
            },

            _ => {
                bail!("Stmt Action handle: Unexpected actions")
            }
        }
    }

    fn create_stmt_pu(&mut self) -> Result<()> {
        let mut loc = 0;
        let mut next_sub_idx = self.subst_recs.next_on_loc(loc)?;

        let mut pu_str = String::new();
        while self.stmt_info.valid_idx(loc) {
            if self.subst_recs.is_start(loc, next_sub_idx)? {
                let sub_state = &self.subst_recs.data[next_sub_idx];
                pu_str.push_str(&sub_state.opr.get_ret_str());
                loc = sub_state.opr.end_idx + 1;
                next_sub_idx = self.subst_recs.next_on_idx(next_sub_idx, false)?;
            } else {
                pu_str.push(self.stmt_info.byte_at(loc)? as char);
                loc += 1;
            }
        }

        todo!()
    }

    pub fn update_pu(self, pu_vec: &mut Vec<ProcessUnit>) {
        pu_vec.extend(self.pu_vec);
    }
}

pub struct StmtStrInfo {
    pub stmt_loc: QLLoc,
    pub content: String,
    pub line_len_vec: Vec<usize>,
}

impl StmtStrInfo {
    pub fn get_seg(&self, start_idx: usize, end_idx: usize) -> Result<&str> {
        if start_idx >= self.content.len() || end_idx >= self.content.len() || start_idx > end_idx {
            bail!("StmtStrInfo Get seg: Index out of bound");
        }
        Ok(&self.content[start_idx..end_idx])
    }

    pub fn valid_idx(&self, idx: usize) -> bool {
        idx < self.content.len()
    }

    pub fn byte_at(&self, idx: usize) -> Result<u8> {
        if idx >= self.content.len() {
            bail!("StmtStrInfo Byte at: Index out of bound");
        }
        Ok(self.content.as_bytes()[idx])
    }

    pub fn from_qlloc(ql_loc: &QLLoc) -> Result<Self> {
        let file = File::open(&ql_loc.file_path)?;
        let reader = BufReader::new(file);

        // Content Construction
        let mut content = String::new();
        let mut line_len_vec = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line_num = idx + 1;
            let line = line?;
            if line_num < ql_loc.start_line {
                continue;
            }
            if line_num > ql_loc.end_line {
                break;
            }

            if line_num == ql_loc.start_line && line_num == ql_loc.end_line {
                // start line and end line are the same
                // NOTE: QLLoc is both-end inclusive
                let snippet = &line[ql_loc.start_column - 1..ql_loc.end_column];
                line_len_vec.push(snippet.len());
                content.push_str(snippet);
            } else if line_num == ql_loc.start_line {
                // start line only
                let snippet = &line[ql_loc.start_column - 1..];
                content.push_str(snippet);
                content.push('\n');
                line_len_vec.push(snippet.len() + 1);
            } else if line_num == ql_loc.end_line {
                // end line only
                // NOTE: QLLoc is both-end inclusive
                let snippet = &line[..ql_loc.end_column];
                content.push_str(snippet);
                line_len_vec.push(snippet.len());
            } else {
                // inner line
                content.push_str(&line);
                content.push('\n');
                line_len_vec.push(line.len() + 1);
            }
        }
        Ok(Self {
            stmt_loc: ql_loc.clone(),
            content,
            line_len_vec,
        })
    }

    fn get_relative_idx_with_func_name(&self, func_name: &str) -> Result<usize> {
        self.content
            .find(func_name)
            .ok_or_else(|| eyre::eyre!("Function name {} not found in QL string", func_name))
    }

    fn get_relative_idx_by_valid_loc(&self, valid_loc: &ValidSrcLoc) -> usize {
        let mut idx = 0;
        for (line_off, line_len) in self.line_len_vec.iter().enumerate() {
            let cur_line = self.stmt_loc.start_line + line_off;
            if cur_line < valid_loc.line {
                idx += line_len;
                continue;
            }

            if cur_line == valid_loc.line {
                let start_col = if cur_line == self.stmt_loc.start_line {
                    self.stmt_loc.start_column
                } else {
                    1
                };
                idx += valid_loc.col - start_col;
                break;
            }
        }
        idx
    }

    fn get_relative_idx(
        &self,
        func_name: &str,
        invoc_loc_op: Option<&SrcLocEnum>,
    ) -> Result<usize> {
        match invoc_loc_op {
            None => self.get_relative_idx_with_func_name(func_name),
            Some(invoc_loc) => match invoc_loc {
                SrcLocEnum::NullLoc => self.get_relative_idx_with_func_name(func_name),
                SrcLocEnum::Valid(valid_loc) => {
                    assert!(
                        valid_loc.file_path == self.stmt_loc.file_path,
                        "QLStrInfo Relative: File path mismatch"
                    );
                    Ok(self.get_relative_idx_by_valid_loc(valid_loc))
                }
            },
        }
    }

    pub fn get_start_idxs(&self, call_act: &FuncCallAction) -> Result<(usize, usize)> {
        let invoc_idx =
            self.get_relative_idx(&call_act.func_name, call_act.invoc_loc_op.as_ref())?;

        let mut idx = invoc_idx + call_act.func_name.len();
        while idx < self.content.len() && self.content.as_bytes()[idx].is_ascii_whitespace() {
            idx += 1;
        }
        assert!(self.content.as_bytes()[idx] == b'(');
        let left_idx = idx;

        Ok((invoc_idx, left_idx))
    }
}
