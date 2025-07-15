use color_eyre::eyre::Result;
pub enum GuardParseError {
    PrefixError { data: eyre::Report },
    ParseError { data: eyre::Report },
    SkipError { data: eyre::Report, skip_num: usize },
}

impl From<eyre::Report> for GuardParseError {
    fn from(err: eyre::Report) -> Self {
        GuardParseError::ParseError { data: err }
    }
}

impl GuardParseError {
    pub fn as_prefix_err(report: eyre::Report) -> Self {
        GuardParseError::PrefixError { data: report }
    }
    pub fn as_parse_err(report: eyre::Report) -> Self {
        GuardParseError::ParseError { data: report }
    }

    pub fn as_skip_err(report: eyre::Report, skip_num: usize) -> Self {
        GuardParseError::SkipError {
            data: report,
            skip_num,
        }
    }

    pub fn is_skip_err(&self) -> bool {
        matches!(self, GuardParseError::SkipError { .. })
    }

    pub fn get_data(&self) -> &eyre::Report {
        match self {
            GuardParseError::PrefixError { data } => data,
            GuardParseError::ParseError { data } => data,
            GuardParseError::SkipError { data, skip_num: _ } => data,
        }
    }

    /// Used in an intermediate process: return Ok(None) to jump to next parsing process
    pub fn to_eyre<T>(res: std::result::Result<T, GuardParseError>) -> Result<Option<T>> {
        match res {
            Ok(val) => Ok(Some(val)),
            Err(GuardParseError::PrefixError { data }) => {
                log::trace!("GuardParse Prefix Error: {}", data);
                Ok(None)
            }
            Err(GuardParseError::ParseError { data }) => Err(data),
            Err(GuardParseError::SkipError { data, skip_num: _ }) => {
                panic!(
                    "GuardParse Skip Error should not be converted to eyre Result: {}",
                    data
                );
            }
        }
    }

    pub fn to_eyre_ultimate<T>(res: std::result::Result<T, GuardParseError>) -> Result<T> {
        match res {
            Ok(val) => Ok(val),
            Err(GuardParseError::PrefixError { data }) => {
                log::trace!("GuardParse Prefix Error: {}", data);
                Err(data)
            }
            Err(GuardParseError::ParseError { data }) => Err(data),
            Err(GuardParseError::SkipError { data, skip_num: _ }) => {
                panic!(
                    "GuardParse Skip Error should not be converted to eyre Result: {}",
                    data
                );
            }
        }
    }
}
