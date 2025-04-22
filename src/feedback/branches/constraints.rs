use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use crate::{
    execution::max_cpu_count,
    feedback::clang_coverage::{get_cov_region_fileid, CodeCoverage, CovFunction, CovRegion},
};
use color_eyre::eyre::Result;
use eyre::bail;
use regex::Regex;
use serde::Serialize;
use std::fs::File;
use threadpool::ThreadPool;

use super::{Branch, BranchTrait};

pub type Loc = [usize; 2];

pub type Range = [usize; 4];

pub type MacMapping = HashMap<String, String>;

pub trait LocTrait {
    fn is_less_equal(&self, loc: &Loc) -> bool;
}
impl LocTrait for Loc {
    fn is_less_equal(&self, loc: &Loc) -> bool {
        if self[0] != loc[0] {
            return self[0] < loc[0];
        }
        self[1] <= loc[1]
    }
}

pub trait RangeTrait {
    fn from_slice(region: &[usize]) -> Result<Range>;
    fn get_range_text_from_file(&self, fpath: &Path) -> Result<String>;
    /// returns [start_loc, end_loc]
    fn extract_locs(&self) -> Result<[Loc; 2]>;
    fn is_inside(&self, rng: &Range) -> Result<bool>;
}

impl RangeTrait for Range {
    fn is_inside(&self, rng: &Range) -> Result<bool> {
        let [a_sloc, a_eloc] = self.extract_locs()?;
        let [b_sloc, b_eloc] = rng.extract_locs()?;
        let flag = b_sloc.is_less_equal(&a_sloc) && a_eloc.is_less_equal(&b_eloc);
        Ok(flag)
    }
    fn extract_locs(&self) -> Result<[Loc; 2]> {
        let sloc = self[0..2].try_into()?;
        let eloc = self[2..4].try_into()?;
        Ok([sloc, eloc])
    }
    fn from_slice(region: &[usize]) -> Result<Range> {
        let range = region[0..4].try_into()?;
        Ok(range)
    }

    fn get_range_text_from_file(&self, fpath: &Path) -> Result<String> {
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

pub trait CovRegionTrait {
    fn get_file_id(&self) -> usize;
    fn get_expansion_file_id(&self) -> usize;
    fn get_kind(&self) -> usize;
    fn is_expansion_region(&self) -> bool;
    fn is_code_region(&self) -> bool;
    fn inside_branch(&self, rgn: &CovRegion) -> Result<bool>;
}

impl CovRegionTrait for CovRegion {
    fn is_code_region(&self) -> bool {
        let kind = self.get_kind();
        kind == 0
    }
    fn get_expansion_file_id(&self) -> usize {
        self[6]
    }

    fn get_file_id(&self) -> usize {
        self[5]
    }

    fn get_kind(&self) -> usize {
        self[7]
    }
    fn is_expansion_region(&self) -> bool {
        let kind = self.get_kind();
        kind == 1
    }

    fn inside_branch(&self, br: &Branch) -> Result<bool> {
        let a_file_id = self.get_file_id();
        let b_file_id = br.get_branch_fileid();
        if a_file_id != b_file_id {
            return Ok(false);
        }

        let a_rng = Range::from_slice(self)?;
        let b_rng = Range::from_slice(br)?;
        let flag = a_rng.is_inside(&b_rng)?;
        Ok(flag)
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Constraint {
    cond_expr: String,
    res: bool,
    fpath: PathBuf,
    range: Range,
    func_sig: String,
    /// Function Body as slice
    slice: String,
    macro_mapping: MacMapping,
}

impl Constraint {
    fn get_cond_expr_in_fname(&self) -> String {
        self.cond_expr
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c
                } else {
                    '%'
                }
            })
            .collect()
    }

    fn get_macro_map_str(&self) -> String {
        let mut res = String::new();
        for (key, val) in self.macro_mapping.iter() {
            let line = format!("{}: {}\n", key, val);
            res.push_str(&line);
        }
        res
    }

    pub fn get_show_filename(&self) -> Result<String> {
        let cons_fname = self.fpath.file_stem().unwrap().to_str().unwrap();
        let fname = format!("{}_{}.md", cons_fname, self.get_cond_expr_in_fname());
        // let fname = format!("{}_{}.md", cons_fname, &self.cond_expr);
        Ok(fname)
    }

    fn show_section(sec_name: &str, sec_content: &str) -> String {
        format!("\n### {}\n\n```c\n{}\n```\n", sec_name, sec_content)
    }

    pub fn get_show_content(&self) -> Result<String> {
        let expr = Self::show_section("Condition Expression", &self.cond_expr);
        let res = Self::show_section("Result Value", &self.res.to_string());
        let func_sig = Self::show_section("Function Signature", &self.func_sig);
        let func_body = Self::show_section("Function Body", &self.slice);
        let fpath = Self::show_section("File Location", self.fpath.to_str().unwrap());

        let macro_map = Self::show_section("Macro Exansion", &self.get_macro_map_str());
        let content = format!("{expr}{res}{macro_map}{func_sig}{func_body}{fpath}");
        Ok(content)
    }
}

impl CovFunction {
    pub fn get_source_file_path_by_file_id(&self, file_id: usize) -> Result<PathBuf> {
        let fpath = self.get_file_path(file_id);
        let fpath = PathBuf::from_str(&fpath)?;
        Ok(fpath)
    }

    pub fn get_source_file_path_by_region(&self, rgn: &CovRegion) -> Result<PathBuf> {
        let file_id = rgn.get_file_id();
        let fpath = self.get_source_file_path_by_file_id(file_id)?;
        Ok(fpath)
    }

    pub fn get_source_file_path_by_branch(&self, br: &Branch) -> Result<PathBuf> {
        let file_id = br.get_branch_fileid();
        let fpath = self.get_source_file_path_by_file_id(file_id)?;
        Ok(fpath)
    }

    fn get_func_text(&self) -> Result<String> {
        let region = self.get_body_region();
        let range = Range::from_slice(&region)?;
        let file_id = get_cov_region_fileid(&region);
        let fpath = self.get_source_file_path_by_file_id(file_id)?;
        let text = range.get_range_text_from_file(&fpath)?;
        Ok(text)
    }

    fn parse_constraint_from_branch(&self, br: &Branch) -> Result<Constraint> {
        let res = br.branch_eval();
        let (cond_expr, fpath, range, func_sig, slice, macro_mapping) =
            self.extract_branch_in_covfunc(br)?;

        let cons = Constraint {
            cond_expr,
            res,
            fpath,
            range,
            func_sig,
            slice,
            macro_mapping,
        };
        Ok(cons)
    }

    fn get_macro_regions_from_branch(&self, br: &Branch) -> Result<Vec<CovRegion>> {
        let mut mac_rgns: Vec<CovRegion> = vec![];
        for rgn in self.regions.iter() {
            if !rgn.is_expansion_region() || !rgn.inside_branch(br)? {
                continue;
            }
            mac_rgns.push(rgn.to_owned());
        }
        Ok(mac_rgns)
    }

    fn get_expanded_region_for_mac_region(&self, mac_rgn: &CovRegion) -> Result<CovRegion> {
        let mac_file_id = mac_rgn.get_expansion_file_id();
        for rgn in self.regions.iter() {
            let file_id = rgn.get_file_id();
            if file_id == mac_file_id {
                assert!(
                    rgn.is_code_region(),
                    "expanded region is not a code region: {:?}",
                    rgn
                );
                return Ok(rgn.to_owned());
            }
        }
        bail!("Failed to get expanded region for mac region {:?}", mac_rgn);
    }

    fn get_region_text(&self, rgn: &CovRegion) -> Result<String> {
        let fpath = self.get_source_file_path_by_region(rgn)?;
        let rng = Range::from_slice(rgn)?;
        let text = rng.get_range_text_from_file(&fpath)?;
        Ok(text)
    }

    fn show_region_debug_info(
        &self,
        rgn: &CovRegion,
        rgn_name: &str,
        rgn_text: &str,
    ) -> Result<()> {
        let fpath = self.get_source_file_path_by_region(rgn)?;
        log::debug!("{} text: {:?}", rgn_name, rgn_text);
        log::debug!("{} region: {:?}", rgn_name, rgn);
        log::debug!("{} fpath: {:?}", rgn_name, fpath);
        Ok(())
    }

    fn get_macro_mapping(&self, br: &Branch) -> Result<MacMapping> {
        let mut mac_mapping: MacMapping = HashMap::new();
        let mac_rgns = self.get_macro_regions_from_branch(br)?;
        for mac_rgn in mac_rgns.iter() {
            let expd_rgn = self.get_expanded_region_for_mac_region(mac_rgn)?;
            let mac_text = self.get_region_text(mac_rgn)?;
            let expd_text = self.get_region_text(&expd_rgn)?;

            #[cfg(debug_assertions)]
            {
                self.show_region_debug_info(mac_rgn, "macro", &mac_text)?;
                self.show_region_debug_info(&expd_rgn, "expanded", &expd_text)?;
            }

            mac_mapping.insert(mac_text, expd_text);
        }
        Ok(mac_mapping)
    }

    /// extract (cond_expr, file_path, range, func_sig, slice) from branch
    fn extract_branch_in_covfunc(
        &self,
        br: &Branch,
    ) -> Result<(String, PathBuf, Range, String, String, MacMapping)> {
        let range = Range::from_slice(br)?;
        let fpath = self.get_source_file_path_by_branch(br)?;

        // get text of specified range in source file
        let expr = range.get_range_text_from_file(&fpath)?;
        let slice = self.get_func_text()?;
        let func_sig = self.get_func_sig(&fpath)?;
        let mac_mapping = self.get_macro_mapping(br)?;

        Ok((expr, fpath, range, func_sig, slice, mac_mapping))
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
        for (cidx, (pos, _)) in line_m.char_indices().enumerate() {
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

    pub fn func_sig_formalize(func_sig: &str) -> Result<String> {
        let func_sig = func_sig.trim();
        let re = Regex::new(r"\n\s*")?;
        let func_sig = re.replace_all(func_sig, " ");
        let func_sig = func_sig.to_string();
        Ok(func_sig)
    }

    fn get_func_sig(&self, fpath: &Path) -> Result<String> {
        let body_reg = self.get_body_region();

        let rng = Range::from_slice(&body_reg)?;
        let [sloc, _] = rng.extract_locs()?;
        let file = File::open(fpath)?;
        let mut reader = BufReader::new(file);

        // get start offset
        let start_ofs = Self::get_bytes_offset_from_loc(&mut reader, &sloc)?;
        let (par1, start_ofs) = Self::get_slice_rev_until(&mut reader, start_ofs, "(")?;
        let (par2, start_ofs) = Self::get_slice_rev_until(&mut reader, start_ofs, " ")?;
        let (par3, _) = Self::get_slice_rev_until(&mut reader, start_ofs, "\n")?;
        let par1 = par1.trim();
        let par3 = par3.trim();

        let sig_str = format!("{}{}{}", par3, par2, par1);
        let sig_str = Self::func_sig_formalize(&sig_str)?;

        Ok(sig_str)
    }
}

impl CodeCoverage {
    pub fn collect_rev_constraints_from_cov_pool(&self) -> Result<Vec<Constraint>> {
        let cpu_count = max_cpu_count();
        let pool = ThreadPool::new(cpu_count);

        let shared_cons_list: Arc<Mutex<Vec<Constraint>>> = Arc::new(Mutex::new(vec![]));
        let err_occur = Arc::new(AtomicBool::new(false));

        for func in self.iter_function_covs() {
            // let br_list = func.get_covered_banch();
            let br_list = func.get_unselected_branch();
            for br in br_list.iter() {
                let func = func.clone();
                let br = *br;
                let cons_list_ptr = shared_cons_list.clone();
                let err_occur = err_occur.clone();

                pool.execute(move || {
                    if err_occur.load(std::sync::atomic::Ordering::SeqCst) {
                        return;
                    }
                    match func.parse_constraint_from_branch(&br) {
                        Ok(cons) => {
                            // add
                            let mut cons_list = cons_list_ptr.lock().unwrap();
                            cons_list.push(cons);
                        }
                        Err(e) => {
                            err_occur.store(true, std::sync::atomic::Ordering::SeqCst);
                            let fpath = func.get_source_file_path_by_branch(&br).unwrap();
                            log::error!(
                                "Failed to parse contraint for branch: {:?}, {:?}",
                                br,
                                fpath
                            );
                            log::error!("Error: {}", e);
                        }
                    };
                });
            }
        }

        pool.join();
        if err_occur.load(std::sync::atomic::Ordering::SeqCst) {
            bail!("Failed to collect constraints from code coverage");
        }

        let cons_list = shared_cons_list.lock().unwrap().to_vec();
        Ok(cons_list)
    }
}

#[cfg(test)]
mod tests {
    use crate::{deopt::Deopt, init_report_utils_for_tests};
    use color_eyre::eyre::Result;

    use super::*;

    #[test]
    fn test_get_range_text() -> Result<()> {
        init_report_utils_for_tests()?;
        let range = [16, 7, 16, 11];
        let dir = Deopt::get_test_data_dir()?;
        let fpath = dir.join("add.c");
        let text = range.get_range_text_from_file(&fpath)?;
        log::debug!("text: {}", text);

        Ok(())
    }

    #[test]
    fn test_func_sig_formalize() -> Result<()> {
        init_report_utils_for_tests()?;
        let func_sig = "
aom_codec_err_t aom_codec_enc_init_ver(aom_codec_ctx_t *ctx,
                                       aom_codec_iface_t *iface,
                                       const aom_codec_enc_cfg_t *cfg,
                                       aom_codec_flags_t flags, int ver)
";
        let form_func_sig = CovFunction::func_sig_formalize(func_sig)?;
        log::debug!("func_sig: {}", form_func_sig);
        Ok(())
    }
}
