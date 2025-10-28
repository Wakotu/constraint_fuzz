use color_eyre::eyre::Result;
use eyre::bail;
use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{
    analysis::constraint::inter::error::ActrecParseError,
    feedback::{
        branches::constraints::{Loc, LocTrait, Range, RangeTrait, UBConstraint},
        clang_coverage::{BranchCount, CovBranch, CovFunction},
    },
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ValidSrcLoc {
    pub file_path: PathBuf,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum SrcLocEnum {
    NullLoc,
    Valid(ValidSrcLoc),
}

impl fmt::Debug for SrcLocEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SrcLocEnum::NullLoc => return write!(f, "NullLoc"),
            SrcLocEnum::Valid(valid_loc) => {
                if valid_loc.file_path.as_os_str().is_empty() {
                    return write!(
                        f,
                        "ValidLoc: <empty file path>:{}:{}",
                        valid_loc.line, valid_loc.col
                    );
                }
                return write!(
                    f,
                    "ValidLoc: {}:{}:{}",
                    valid_loc.file_path.display(),
                    valid_loc.line,
                    valid_loc.col
                );
            }
        }
    }
}

impl SrcLocEnum {
    pub fn get_src_path(&self) -> Option<&Path> {
        match self {
            SrcLocEnum::NullLoc => None,
            SrcLocEnum::Valid(valid_loc) => Some(valid_loc.file_path.as_path()),
        }
    }

    pub fn get_line(&self) -> Option<usize> {
        match self {
            SrcLocEnum::NullLoc => None,
            SrcLocEnum::Valid(valid_loc) => Some(valid_loc.line),
        }
    }

    pub fn get_col(&self) -> Option<usize> {
        match self {
            SrcLocEnum::NullLoc => None,
            SrcLocEnum::Valid(valid_loc) => Some(valid_loc.col),
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            SrcLocEnum::NullLoc => false,
            SrcLocEnum::Valid(valid_loc) => {
                !valid_loc.file_path.as_os_str().is_empty()
                    && valid_loc.line > 0
                    && valid_loc.col > 0
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
    ) -> std::result::Result<Self, ActrecParseError> {
        if !line.starts_with(prefix) {
            return Err(ActrecParseError::as_prefix_err(eyre::eyre!(
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

        if line == 0 || col == 0 {
            return Ok(Self::NullLoc);
        }

        Ok(Self::Valid(ValidSrcLoc {
            file_path: fpath,
            line: line,
            col: col,
        }))
    }

    pub fn from_cov_loc<P: AsRef<Path>>(cov_loc: &Loc, fpath: P) -> Self {
        Self::Valid(ValidSrcLoc {
            file_path: fpath.as_ref().to_owned(),
            line: cov_loc[0],
            col: cov_loc[1],
        })
    }
}

pub struct SrcRegion {
    start: SrcLocEnum,
    end: SrcLocEnum,
    func_name: String,
}

impl SrcRegion {
    pub fn from_range(rng: &Range, fpath: &Path, func_name: &str) -> Result<Self> {
        let [start, end] = rng.extract_locs()?;
        let start_loc = SrcLocEnum::from_cov_loc(&start, fpath);
        let end_loc = SrcLocEnum::from_cov_loc(&end, fpath);

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

    pub fn is_related_to_cons(&self, cons: &UBConstraint) -> Result<bool> {
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
