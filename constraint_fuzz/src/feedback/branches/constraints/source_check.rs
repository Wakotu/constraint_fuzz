use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::{
    analysis::constraint::inter::loc::SrcLoc,
    feedback::branches::constraints::{Loc, LocTrait, Range, RangeTrait, UBConstraint},
};
use color_eyre::eyre::Result;

enum UBType {
    // selection or loop
    SelorLoop,
    Switch,
}

impl UBConstraint {
    fn is_switch_case_clause(fpath: &Path, range: Range) -> Result<bool> {
        const CASE_PREFIX: &str = "case";
        const DEFAULT_PREFIX: &str = "default";

        let file = File::open(fpath)?;
        let reader = BufReader::new(file);
        let [s_loc, e_loc] = range.extract_locs()?;

        // get first word started at `s_loc`
        let mut word = String::new();
        let mut flag = false;
        for (r_idx, line_res) in reader.lines().enumerate() {
            let row = r_idx + 1;
            let line = line_res?;

            for (c_idx, ch) in line.chars().enumerate() {
                let col = c_idx + 1;
                if s_loc.loc_equal(row, col) {
                    flag = true;
                }
                if e_loc.loc_equal(row, col) {
                    flag = false;
                    break;
                }
                if ch.is_whitespace() {
                    break;
                }

                if flag {
                    word.push(ch);
                }
            }
        }

        Ok(word == CASE_PREFIX || word == DEFAULT_PREFIX)
    }

    pub fn source_check(&self) -> Result<UBType> {
        let src_fpath = &self.fpath;
        let src_range = self.range;

        if Self::is_switch_case_clause(src_fpath, src_range)? {
            Ok(UBType::Switch)
        } else {
            Ok(UBType::SelorLoop)
        }
    }

    /// return start location switch structure
    /// Assume as switch UB
    pub fn get_switch_start(&self) -> Result<SrcLoc> {
        todo!()
    }
}
