use std::{
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::feedback::clang_coverage::{get_cov_region_fileid, CodeCoverage, CovFunction};
use color_eyre::eyre::Result;
use colored::Colorize;
use serde::Serialize;
use std::fs::File;

use super::{branch_eval, get_branch_fileid, Branch};

pub type Loc = [usize; 2];

pub type Range = [usize; 4];

pub trait RangeTrait {
    fn from_slice(region: &[usize]) -> Result<Range>;
    fn get_range_text(&self, fpath: &Path) -> Result<String>;
    /// returns [start_loc, end_loc]
    fn extract_locs(&self) -> Result<[Loc; 2]>;
}

impl RangeTrait for Range {
    fn extract_locs(&self) -> Result<[Loc; 2]> {
        let sloc = self[0..2].try_into()?;
        let eloc = self[2..4].try_into()?;
        Ok([sloc, eloc])
    }
    fn from_slice(region: &[usize]) -> Result<Range> {
        let range = region[0..4].try_into()?;
        Ok(range)
    }

    fn get_range_text(&self, fpath: &Path) -> Result<String> {
        let file = File::open(fpath)?;
        let reader = BufReader::new(file);

        let [ls, cs, le, ce] = self;
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
            // log::debug!("LINE cur_line: {}", cur_line);

            // column judging
            for (cidx, ch) in line.chars().enumerate() {
                let cur_col = cidx + 1;
                if cur_line == *ls && cur_col == *cs {
                    sta = true;
                    // log::debug!(
                    //     "COL cur_line: {}, cur_col: {}, ch: {}",
                    //     cur_line,
                    //     cur_col,
                    //     ch
                    // );
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
}

#[derive(Debug, Serialize)]
pub struct Constraint {
    cond_expr: String,
    res: bool,
    fpath: PathBuf,
    range: Range,
    func_name: String,
    slice: String,
}

impl CovFunction {
    pub fn get_source_file_path_by_file_id(&self, file_id: usize) -> Result<PathBuf> {
        let fpath = self.get_file_path(file_id);
        let fpath = PathBuf::from_str(&fpath)?;
        Ok(fpath)
    }

    pub fn get_source_file_path(&self, br: &Branch) -> Result<PathBuf> {
        let file_id = get_branch_fileid(br);
        let fpath = self.get_source_file_path_by_file_id(file_id)?;
        Ok(fpath)
    }

    fn get_func_text(&self) -> Result<String> {
        let region = self.get_body_region();
        let range = Range::from_slice(&region)?;
        let file_id = get_cov_region_fileid(&region);
        let fpath = self.get_source_file_path_by_file_id(file_id)?;
        let text = range.get_range_text(&fpath)?;
        Ok(text)
    }

    fn parse_constraint_from_branch(&self, br: &Branch) -> Result<Constraint> {
        let res = branch_eval(br);
        let (cond_expr, fpath, range, func_name, slice) = self.extract_branch_in_covfunc(br)?;

        let cons = Constraint {
            cond_expr,
            res,
            fpath,
            range,
            func_name,
            slice,
        };
        Ok(cons)
    }

    /// extract (cond_expr, file_path, range, func_sig, slice) from branch
    fn extract_branch_in_covfunc(
        &self,
        br: &Branch,
    ) -> Result<(String, PathBuf, Range, String, String)> {
        let range = Range::from_slice(br)?;
        let fpath = self.get_source_file_path(br)?;

        // get text of specified range in source file
        let expr = range.get_range_text(&fpath)?;
        let slice = self.get_func_text()?;
        let func_name = self.get_func_sig(&fpath)?;
        Ok((expr, fpath, range, func_name, slice))
    }

    /// returns modifies offset of reader and returns bytes offset value
    fn get_bytes_offset_from_loc(reader: &mut BufReader<File>, loc: &Loc) -> Result<usize> {
        let [m, n] = loc;
        let mut ofs: usize = 0;
        let mut buf = String::new();
        for _ in 0..*m - 1 {
            let bytes = reader.read_line(&mut buf)?;
            ofs += bytes;
            buf.clear();
        }

        reader.read_line(&mut buf)?;
        let line_m = buf;
        for (cidx, (pos, ch)) in line_m.char_indices().enumerate() {
            let col = cidx + 1;
            if col == *n {
                ofs += pos;
                break;
            }
        }

        Ok(ofs)
    }

    /// start exlusive, end inclusive
    fn get_slice_rev_until(
        reader: &mut BufReader<File>,
        start_ofs: usize,
        delim: &str,
    ) -> Result<(String, usize)> {
        let mut prev_ofs = start_ofs;
        let mut cur_ofs = start_ofs;

        let mut slice: String = String::new();
        while cur_ofs > 0 {
            cur_ofs -= 1;
            let len = prev_ofs - cur_ofs;
            let mut char_buf: [u8; 4] = [0; 4];

            reader.seek(SeekFrom::Start(cur_ofs as u64))?;
            reader.read_exact(&mut char_buf[..len])?;

            if let Ok(s) = std::str::from_utf8(&char_buf[..len]) {
                slice.push_str(s);
                if s == delim {
                    break;
                }

                prev_ofs = cur_ofs;
            }
        }
        let slice = slice.chars().rev().collect::<String>();

        Ok((slice, cur_ofs))
    }

    fn get_func_sig(&self, fpath: &Path) -> Result<String> {
        let body_reg = self.get_body_region();

        let rng = Range::from_slice(&body_reg)?;
        let [sloc, _] = rng.extract_locs()?;
        // TODO: implement get function signature
        let file = File::open(fpath)?;
        let mut reader = BufReader::new(file);

        // get start offset
        let start_ofs = Self::get_bytes_offset_from_loc(&mut reader, &sloc)?;
        let (par1, start_ofs) = Self::get_slice_rev_until(&mut reader, start_ofs, "(")?;
        let (par2, start_ofs) = Self::get_slice_rev_until(&mut reader, start_ofs, " ")?;
        let (par3, _) = Self::get_slice_rev_until(&mut reader, start_ofs, "\n")?;
        let par3 = par3.trim();

        let sig_str = format!("{}{}{}", par3, par2, par1);
        Ok(sig_str)
    }
}

impl CodeCoverage {
    pub fn collect_constraints_from_cov(&self) -> Result<Vec<Constraint>> {
        let mut cons_list: Vec<Constraint> = vec![];

        for func in self.iter_function_covs() {
            // let br_list = func.get_covered_banch();
            let br_list = func.get_unselected_branch();
            for br in br_list.iter() {
                let cons = func.parse_constraint_from_branch(br)?;
                cons_list.push(cons);
            }
        }

        Ok(cons_list)
    }
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
        let text = range.get_range_text(&fpath)?;
        log::debug!("text: {}", text);

        Ok(())
    }
}
