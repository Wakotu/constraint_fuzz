use crate::analysis::constraint::inter::exec_tree::action::{
    ExecAction, FuncAction, FuncCallAction, JumpActionType,
};
use crate::analysis::constraint::inter::loc::{SrcLocEnum, ValidSrcLoc};
use crate::analysis::constraint::intra::func_src_tree::nodes::SharedStmtNodePtr;
use crate::analysis::constraint::intra::func_src_tree::{
    code_query::scope_var_query::SrcVar, stmts::QLLoc,
};
use crate::analysis::constraint::stmt_collect::{
    InnerCondRec, ProcessUnit, ProcessUnitVariant, StmtCollector,
};
use std::slice::Iter;

use color_eyre::eyre::Result;
use eyre::bail;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::id;

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

    pub fn get_nonvoid_retstr(&self) -> Result<String> {
        match &self.ret_var_op {
            None => bail!("Invoc Substitution: void function invocation should not occur in value expressions"),
            Some(var) => Ok(var.var_name_str()),
        }
    }

    pub fn get_offset(&self) -> i32 {
        let orig_len = self.end_idx - self.start_idx + 1;
        let new_len = self.get_ret_str().len();
        new_len as i32 - orig_len as i32
    }
}

#[derive(PartialEq, Eq)]
pub struct InvocSubstOprState {
    opr: InvocSubstOpr,
    covered: bool,
}

impl InvocSubstOprState {
    pub fn is_valid(&self) -> bool {
        !self.covered
    }

    pub fn before_cond(&self, cond_state: &CondRecState) -> bool {
        self.opr.end_idx < cond_state.data.inner_idx
    }

    pub fn after_cond(&self, cond_state: &CondRecState) -> bool {
        self.opr.start_idx > cond_state.data.inner_idx
    }

    pub fn contains_cond(&self, cond_state: &CondRecState) -> bool {
        self.opr.start_idx <= cond_state.data.inner_idx
            && cond_state.data.inner_idx <= self.opr.end_idx
    }

    pub fn get_offset(&self) -> i32 {
        self.opr.get_offset()
    }
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

pub struct InvocSubRecs {
    data: Vec<InvocSubstOprState>,
}

impl InvocSubRecs {
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

    pub fn next_on_cond_with_idx(
        &self,
        cond_state: &CondRecState,
        start_idx_op: Option<usize>,
        off: &mut i32,
    ) -> Result<Option<usize>> {
        let start_idx = match start_idx_op {
            Some(idx) => idx,
            None => return Ok(None),
        };
        for (idx, sub_rec) in self.data[start_idx..].iter().enumerate() {
            if sub_rec.covered {
                continue;
            }

            // Contains
            if sub_rec.contains_cond(cond_state) {
                bail!("Sub Next idx sync: Valid Substitution record should not contain cond state")
            }

            // after
            if sub_rec.after_cond(cond_state) {
                return Ok(Some(idx));
            }

            // Before: update offset
            *off += sub_rec.get_offset();
        }
        Ok(None)
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
    pub expr_str: String,
    pub var_vec: Vec<SrcVar>,
    pub cond_vec: Vec<InnerCondRec>,
}

impl ArgExpr {
    pub fn from_arg_seg(
        arg_seg: &str,
        live_var_vec: &Vec<SrcVar>,
        ret_var_vec: &Vec<SrcVar>,
        cond_vec: Vec<InnerCondRec>,
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
            cond_vec,
        }
    }

    pub fn derive_cond_vec(&self, offset: usize) -> Vec<InnerCondRec> {
        self.cond_vec
            .iter()
            .map(|cond_rec| cond_rec.derive_plus(offset))
            .collect()
    }
}

#[derive(PartialEq, Eq)]
pub struct CondRecState {
    data: InnerCondRec,
    valid: bool,
}

impl CondRecState {
    pub fn before(&self, loc: usize) -> bool {
        self.data.before(loc)
    }

    pub fn before_or_eq(&self, loc: usize) -> bool {
        self.data.before_or_eq(loc)
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }
}

impl PartialOrd for CondRecState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl Ord for CondRecState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data.cmp(&other.data)
    }
}

pub struct CondStateVec {
    data: Vec<CondRecState>,
}

impl CondStateVec {
    pub fn new() -> Self {
        Self { data: vec![] }
    }

    pub fn iter(&self) -> Iter<'_, CondRecState> {
        self.data.iter()
    }

    pub fn add(&mut self, cond_rec: InnerCondRec) {
        self.data.push(CondRecState {
            data: cond_rec,
            valid: true,
        });
        self.data.sort();
    }

    pub fn get_valid_rec(&self) -> Vec<InnerCondRec> {
        // filter and convert CondRec
        self.data
            .iter()
            .filter_map(|cond_state| match cond_state.valid {
                false => None,
                true => Some(cond_state.data.clone()),
            })
            .collect()
    }

    pub fn next_idx_on_loc_and_start(&self, loc: usize, start_idx: usize) -> usize {
        let mut idx = start_idx;
        while idx < self.data.len() && (self.data[idx].before(loc) || !self.data[idx].is_valid()) {
            idx += 1;
        }

        idx
    }

    pub fn next_idx_on_loc(&self, loc: usize) -> usize {
        self.next_idx_on_loc_and_start(loc, 0)
    }

    /**
     * Range here is designed to be left-inclusive and right-exclusive: [left_loc, right_loc)
     */
    pub fn filter_recs_inrange(
        &mut self,
        start_idx: usize,
        left_loc: usize,
        right_loc: usize,
    ) -> (Vec<InnerCondRec>, usize) {
        let mut idx = start_idx;

        // move to left loc
        while idx < self.data.len()
            && (self.data[idx].before(left_loc) || !self.data[idx].is_valid())
        {
            idx += 1;
        }

        // collect inrange recs
        let mut cond_recs = vec![];
        while idx < self.data.len() && self.data[idx].before(right_loc) {
            if !self.data[idx].is_valid() {
                idx += 1;
                continue;
            }

            // collect and update valid status
            cond_recs.push(self.data[idx].data.derive_minus(left_loc));
            self.data[idx].valid = false;
            idx += 1;
        }

        (cond_recs, idx)
    }
}

pub struct InnerStmtHandler<'a> {
    // derive from ql_loc of specified expr/stmt
    stmt_info: StmtStrInfo,
    // stmt_ptr: SharedStmtNodePtr,
    collector: &'a StmtCollector<'a>,
    // derive from stmt_ptr
    live_var_vec: Vec<SrcVar>,
    // middle state
    invoc_sub_recs: InvocSubRecs,
    cond_state_vec: CondStateVec,
    // result field
    pu_vec: Vec<ProcessUnit>,
}

impl<'a> InnerStmtHandler<'a> {
    pub fn new(
        // used for action loc match
        expr_loc: &QLLoc,
        // used for valid var query
        stmt_ptr: SharedStmtNodePtr,
        collector: &'a StmtCollector,
    ) -> Result<Self> {
        let stmt_info = StmtStrInfo::from_qlloc(expr_loc)?;
        let live_var_vec = SrcVar::get_live_var(stmt_ptr.clone());

        Ok(Self {
            stmt_info,
            invoc_sub_recs: InvocSubRecs::new(),
            // stmt_ptr: stmt_ptr.clone(),
            collector,
            live_var_vec,
            cond_state_vec: CondStateVec::new(),
            pu_vec: vec![],
        })
    }

    // construction method
    pub fn from_stmt_ptr(
        stmt_ptr: SharedStmtNodePtr,
        collector: &'a StmtCollector,
    ) -> Result<Self> {
        let stmt_node = stmt_ptr.borrow();
        let stmt_loc = stmt_node.get_loc();
        Self::new(stmt_loc, stmt_ptr.clone(), collector)
    }

    fn arg_expr_collect(&mut self, left_idx: usize) -> Result<(Vec<ArgExpr>, usize)> {
        let mut left_loc = left_idx + 1;
        let mut arg_expr_vec = vec![];
        let mut next_sub_idx = self.invoc_sub_recs.next_on_loc(left_loc)?;
        let mut next_cond_idx = 0;

        let right_loc = loop {
            let mut loc = left_loc;
            let mut arg_seg = String::new();
            let mut ret_var_vec: Vec<SrcVar> = vec![];

            // arg segment recognize and inner substitution execution
            while self.stmt_info.byte_at(loc)? != b',' && self.stmt_info.byte_at(loc)? != b')' {
                if self.invoc_sub_recs.is_start(loc, next_sub_idx)? {
                    let sub_state = &self.invoc_sub_recs.data[next_sub_idx];
                    // ret_var
                    if let Some(var) = &sub_state.opr.ret_var_op {
                        ret_var_vec.push(var.clone());
                    }
                    // update loc
                    loc = sub_state.opr.end_idx + 1;
                    // arg seg update: do not allow void function invocation
                    arg_seg.push_str(&sub_state.opr.get_nonvoid_retstr()?);
                    // update idx
                    next_sub_idx = self.invoc_sub_recs.next_on_idx(next_sub_idx, true)?;
                } else {
                    arg_seg.push(self.stmt_info.byte_at(loc)? as char);
                    loc += 1;
                }
            }

            // left_loc -> left_loc and loc -> right_loc here
            let right_loc = loc;
            next_cond_idx = self
                .cond_state_vec
                .next_idx_on_loc_and_start(left_loc, next_cond_idx);
            let (cond_vec, next_idx) =
                self.cond_state_vec
                    .filter_recs_inrange(next_cond_idx, left_loc, right_loc);
            next_cond_idx = next_idx;

            let arg_expr =
                ArgExpr::from_arg_seg(&arg_seg, &self.live_var_vec, &ret_var_vec, cond_vec);
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
            let pu = ProcessUnit::create_pre_func_assign_pu(arg_expr, param_var);
            pu_vec.push(pu);
        }

        Ok((pu_vec, right_idx))
    }

    /**
     * Function Invocation action handle
     */
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

        // return value handle: update subst recs
        self.invoc_sub_recs.add(start_idx, right_idx, ret_var_op);
        Ok(())
    }

    /**
     * Inner Cond Value Handle: for multiple kinds of actions
     */

    fn inner_cond_val_handle(&mut self, loc: &SrcLocEnum, val: bool) -> Result<()> {
        let stmt_idx = self.stmt_info.get_relaidx_by_srcloc(loc)?;
        // cv_rec -> cond val rec
        let cond_rec = InnerCondRec {
            inner_idx: stmt_idx,
            cond_val: val,
        };

        self.cond_state_vec.add(cond_rec);
        Ok(())
    }

    // one act handle interface
    pub fn act_handle(&mut self, act: &ExecAction) -> Result<()> {
        match act {
            // handle func invocation
            ExecAction::Func(func_act) => match func_act {
                FuncAction::Call(call_act) => self.func_invoc_handle(call_act),
                // Unwind should not be handled in inner stmt handler
                FuncAction::Unwind { loc: _ } => {
                    bail!("Unwind action should not be handled in inner stmt handler")
                }
                _ => {
                    bail!("Unexpected Func action: {:?}", func_act);
                }
            },

            ExecAction::Intra(jump_act) => match &jump_act.jump_variants {
                JumpActionType::Br { val_loc } => {
                    self.inner_cond_val_handle(val_loc, jump_act.cond_val)
                }
                JumpActionType::MergeBr => Ok(()),
                jump_act_type => bail!(
                    "Stmt Action handle: Unexpected jump action type: {:?}",
                    jump_act_type
                ),
            },

            ExecAction::Select(sel_act) => self.inner_cond_val_handle(&sel_act.loc, sel_act.val),

            ExecAction::UBV(ubv_hit) => self.inner_cond_val_handle(&ubv_hit.loc, ubv_hit.val),

            _ => {
                bail!("Stmt Action handle: Unexpected actions")
            }
        }
    }

    fn valid_var_filter(&self, stmt_str: &str) -> Vec<SrcVar> {
        self.live_var_vec
            .iter()
            .filter(|var| stmt_str.contains(&var.var_name_str()))
            .cloned()
            .collect()
    }

    fn derive_final_pu_cond_recs(&self) -> Result<Vec<InnerCondRec>> {
        let mut next_sub_idx_op = Some(0);
        let mut off: i32 = 0;

        let mut cond_recs = vec![];
        for cond_state in self.cond_state_vec.iter() {
            if !cond_state.is_valid() {
                continue;
            }
            next_sub_idx_op =
                self.invoc_sub_recs
                    .next_on_cond_with_idx(cond_state, next_sub_idx_op, &mut off)?;

            let new_idx = cond_state.data.inner_idx as i32 + off;
            if new_idx < 0 {
                bail!(
                    "Final PU Derive Error: Cond rec index should not be minus after substitution"
                );
            }
            cond_recs.push(InnerCondRec {
                inner_idx: new_idx as usize,
                cond_val: cond_state.data.cond_val,
            });
        }
        Ok(cond_recs)
    }

    fn derive_final_pu(&mut self) -> Result<ProcessUnit> {
        let mut loc = 0;
        let mut next_sub_idx = self.invoc_sub_recs.next_on_loc(loc)?;

        // Execute substitution: construct final pu str and ret var collection
        let mut pu_str = String::new();
        let mut ret_var_vec: Vec<SrcVar> = vec![];
        while self.stmt_info.valid_idx(loc) {
            if self.invoc_sub_recs.is_start(loc, next_sub_idx)? {
                let sub_state = &self.invoc_sub_recs.data[next_sub_idx];
                if let Some(var) = &sub_state.opr.ret_var_op {
                    ret_var_vec.push(var.clone());
                }

                // allows void
                pu_str.push_str(&sub_state.opr.get_ret_str());
                loc = sub_state.opr.end_idx + 1;
                next_sub_idx = self.invoc_sub_recs.next_on_idx(next_sub_idx, false)?;
            } else {
                pu_str.push(self.stmt_info.byte_at(loc)? as char);
                loc += 1;
            }
        }

        // Valid var filter
        let mut valid_var_vec = self.valid_var_filter(&pu_str);
        valid_var_vec.extend(ret_var_vec);

        // Cond Rec Derivation
        let cond_recs = self.derive_final_pu_cond_recs()?;

        let pu = ProcessUnit {
            content: pu_str,
            valid_var_vec,
            cond_rec_vec: cond_recs,
            variants: ProcessUnitVariant::Plain {},
        };
        Ok(pu)
    }

    fn append_stmt_final_pu(&mut self) -> Result<()> {
        let fpu = self.derive_final_pu()?;
        self.pu_vec.push(fpu);
        Ok(())
    }

    // Should be called at the end of inner statement handle
    pub fn update_pu(mut self, pu_vec: &mut Vec<ProcessUnit>) -> Result<()> {
        self.append_stmt_final_pu()?;
        pu_vec.extend(self.pu_vec);
        Ok(())
    }

    pub fn get_finalpu_while_update(
        mut self,
        pu_vec: &mut Vec<ProcessUnit>,
    ) -> Result<ProcessUnit> {
        let fpu = self.derive_final_pu()?;
        pu_vec.extend(self.pu_vec);
        Ok(fpu)
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

    pub fn get_relaidx_by_srcloc(&self, src_loc: &SrcLocEnum) -> Result<usize> {
        match src_loc {
            SrcLocEnum::NullLoc => bail!("QLStrInfo Relative: Null Loc"),
            SrcLocEnum::Valid(valid_loc) => {
                assert!(
                    valid_loc.file_path == self.stmt_loc.file_path,
                    "QLStrInfo Relative: File path mismatch"
                );
                Ok(self.get_relative_idx_by_valid_loc(valid_loc))
            }
        }
    }

    fn get_rela_idx_for_func_invoc(
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
            self.get_rela_idx_for_func_invoc(&call_act.func_name, call_act.invoc_loc_op.as_ref())?;

        let mut idx = invoc_idx + call_act.func_name.len();
        while idx < self.content.len() && self.content.as_bytes()[idx].is_ascii_whitespace() {
            idx += 1;
        }
        assert!(self.content.as_bytes()[idx] == b'(');
        let left_idx = idx;

        Ok((invoc_idx, left_idx))
    }
}
