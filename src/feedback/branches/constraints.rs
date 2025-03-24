use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::feedback::clang_coverage::{CodeCoverage, CovBranch, CovFunction};
use color_eyre::eyre::Result;
use eyre::bail;
use std::fs::File;

use super::{branch_eval, Branch};

type Range = [usize; 4];

#[derive(Debug)]
pub struct Constraint {
    cond_expr: String,
    res: bool,
}

fn get_source_file_path(br: &Branch, func: &CovFunction) -> Result<PathBuf> {
    let file_id = br[4];
    let fpath = func.get_file_path(file_id);
    let fpath = PathBuf::from_str(&fpath)?;
    Ok(fpath)
}

fn get_range_text(range: &Range, fpath: &Path) -> Result<String> {
    let file = File::open(fpath)?;
    let reader = BufReader::new(file);

    let [ls, cs, le, ce] = range;
    let mut expr = String::new();
    let mut sta = false;

    for (lidx, line_res) in reader.lines().enumerate() {
        let line = line_res?;
        let cur_line = lidx + 1;
        if cur_line < *ls {
            continue;
        }
        if cur_line > *le {
            break;
        }
        log::debug!("LINE cur_line: {}", cur_line);

        // column judging
        for (cidx, ch) in line.chars().enumerate() {
            let cur_col = cidx + 1;
            if cur_line == *ls && cur_col == *cs {
                sta = true;
                log::debug!(
                    "COL cur_line: {}, cur_col: {}, ch: {}",
                    cur_line,
                    cur_col,
                    ch
                );
            }
            if cur_line == *le && cur_col == *ce {
                return Ok(expr);
            }

            if sta {
                expr.push(ch);
            }
        }
        expr.push('\n');
    }

    Ok(expr)
}

fn extract_cond_expr_from_branch(br: &Branch, func: &CovFunction) -> Result<String> {
    let range: Range = br[0..4].try_into()?;
    let fpath = get_source_file_path(br, func)?;

    // get text of specified range in source file
    let expr = get_range_text(&range, &fpath)?;
    Ok(expr)
}

fn parse_constraint_from_branch(br: &Branch, func: &CovFunction) -> Result<Constraint> {
    let res = branch_eval(br);
    let cond_expr = extract_cond_expr_from_branch(br, func)?;

    let cons = Constraint { cond_expr, res };
    Ok(cons)
}

pub fn collect_constraints_from_cov(cov: &CodeCoverage) -> Result<Vec<Constraint>> {
    let mut cons_list: Vec<Constraint> = vec![];

    for func in cov.iter_function_covs() {
        let br_list = func.get_covered_banch();
        for br in br_list.iter() {
            let cons = parse_constraint_from_branch(br, func)?;
            cons_list.push(cons);
        }
    }

    Ok(cons_list)
}

#[cfg(test)]
mod tests {
    use crate::{deopt::Deopt, init_debug_logger};
    use color_eyre::eyre::Result;

    use super::*;

    #[test]
    fn test_get_range_text() -> Result<()> {
        init_debug_logger()?;
        let range = [16, 7, 16, 11];
        let dir = Deopt::get_test_data_dir()?;
        let fpath = dir.join("add.c");
        let text = get_range_text(&range, &fpath)?;
        log::debug!("text: {}", text);

        Ok(())
    }
}
