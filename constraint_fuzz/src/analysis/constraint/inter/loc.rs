use color_eyre::eyre::Result;
use eyre::bail;
use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    analysis::constraint::inter::error::GuardParseError,
    feedback::{
        branches::constraints::{Constraint, Loc, LocTrait, Range, RangeTrait},
        clang_coverage::{BranchCount, CovBranch, CovFunction},
    },
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SrcLoc {
    NullLoc,
    Valid {
        fpath: PathBuf,
        line: usize,
        col: usize,
    },
}

impl fmt::Debug for SrcLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrcLoc::NullLoc => return write!(f, "NullLoc"),
            SrcLoc::Valid { fpath, line, col } => {
                if fpath.as_os_str().is_empty() {
                    return write!(f, "ValidLoc: <empty file path>:{}:{}", line, col);
                }
                return write!(f, "ValidLoc: {}:{}:{}", fpath.display(), line, col);
            }
        }
    }
}

impl SrcLoc {
    pub fn get_src_path(&self) -> Option<&Path> {
        match self {
            SrcLoc::NullLoc => None,
            SrcLoc::Valid { fpath, .. } => Some(fpath.as_path()),
        }
    }

    pub fn get_line(&self) -> Option<usize> {
        match self {
            SrcLoc::NullLoc => None,
            SrcLoc::Valid { line, .. } => Some(*line),
        }
    }

    pub fn get_col(&self) -> Option<usize> {
        match self {
            SrcLoc::NullLoc => None,
            SrcLoc::Valid { col, .. } => Some(*col),
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            SrcLoc::NullLoc => false,
            SrcLoc::Valid { fpath, line, col } => {
                !fpath.as_os_str().is_empty() && *line > 0 && *col > 0
            }
        }
    }

    pub fn inside_range(&self, rng: &Range, fpath: &Path) -> Result<bool> {
        if !self.is_valid() {
            return Ok(false);
        }
        let src_path = self
            .get_src_path()
            .ok_or_else(|| eyre::eyre!("Source location is null"))?;
        if src_path != fpath {
            return Ok(false);
        }

        let [sloc, eloc] = rng.extract_locs()?;
        let loc = [
            self.get_line()
                .ok_or_else(|| eyre::eyre!("Src Loc is Null"))?,
            self.get_col()
                .ok_or_else(|| eyre::eyre!("Src Loc is Null"))?,
        ];
        Ok(sloc.is_less_equal(&loc) && loc.is_less_equal(&eloc))
    }

    pub fn parse_line_with_prefix(
        line: &str,
        prefix: &str,
    ) -> std::result::Result<Self, GuardParseError> {
        if !line.starts_with(prefix) {
            return Err(GuardParseError::as_prefix_err(eyre::eyre!(
                "Line does not start with expected prefix: {}",
                prefix
            )));
        }

        let loc_str = &line[prefix.len()..].trim();
        let res = Self::from_str(loc_str)?;
        Ok(res)
    }

    pub fn from_str(s: &str) -> Result<Self> {
        // parse nullloc
        let ss = s.to_lowercase();
        if ss == "nullloc" || ss == "null" {
            return Ok(Self::NullLoc);
        }

        // example: /path/to/file.c:123:45
        let mut parts = s.rsplitn(3, ':');
        let col_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing column in source location"))?;
        let line_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing line in source location"))?;
        let fpath_str = parts
            .next()
            .ok_or_else(|| eyre::eyre!("Missing file path in source location"))?;

        let col = col_str.parse::<usize>()?;
        let line = line_str.parse::<usize>()?;
        let fpath = PathBuf::from(fpath_str);

        Ok(Self::Valid {
            fpath: fpath,
            line: line,
            col: col,
        })
    }

    pub fn from_cov_loc<P: AsRef<Path>>(cov_loc: &Loc, fpath: P) -> Self {
        Self::Valid {
            fpath: fpath.as_ref().to_owned(),
            line: cov_loc[0],
            col: cov_loc[1],
        }
    }
}

pub struct SrcRegion {
    start: SrcLoc,
    end: SrcLoc,
    func_name: String,
}

impl SrcRegion {
    pub fn from_range(rng: &Range, fpath: &Path, func_name: &str) -> Result<Self> {
        let [start, end] = rng.extract_locs()?;
        let start_loc = SrcLoc::from_cov_loc(&start, fpath);
        let end_loc = SrcLoc::from_cov_loc(&end, fpath);

        if start_loc.get_src_path() != end_loc.get_src_path() {
            bail!("Start and end locations must be in the same file");
        }

        if start_loc.get_line() > end_loc.get_line()
            || (start_loc.get_line() == end_loc.get_line()
                && start_loc.get_col() > end_loc.get_col())
        {
            bail!("Start location must be before or equal to end location");
        }

        Ok(Self {
            start: start_loc,
            end: end_loc,
            func_name: func_name.to_owned(),
        })
    }

    pub fn from_cov_br(cov_br: &CovBranch, cov_func: &CovFunction) -> Result<Self> {
        let fpath = cov_func.get_source_file_path_by_cov_branch(cov_br)?;
        let rng = cov_br.get_range()?;
        Self::from_range(&rng, &fpath, &cov_func.name)
    }

    pub fn get_src_fpath(&self) -> Option<PathBuf> {
        self.start.get_src_path().map(|p| p.to_owned())
    }

    pub fn is_related_to_cons(&self, cons: &Constraint) -> Result<bool> {
        let src_path = self
            .get_src_fpath()
            .ok_or_else(|| eyre::eyre!("Source file path is null"))?;
        let flag = src_path == cons.fpath && self.func_name == cons.get_func_name()?;
        Ok(flag)
    }
}

impl fmt::Debug for SrcRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}-{:?} in function {}",
            self.start, self.end, self.func_name
        )
    }
}
