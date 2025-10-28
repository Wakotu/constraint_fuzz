use std::num::ParseIntError;

use color_eyre::eyre::Result;
pub enum ActrecParseError {
    PrefixError { data: eyre::Report },
    ParseError { data: eyre::Report },
    SkipError { data: eyre::Report, skip_num: usize },
}

impl From<eyre::Report> for ActrecParseError {
    fn from(err: eyre::Report) -> Self {
        ActrecParseError::ParseError { data: err }
    }
}

impl From<ParseIntError> for ActrecParseError {
    fn from(value: ParseIntError) -> Self {
        ActrecParseError::ParseError {
            data: eyre::eyre!(value),
        }
    }
}

impl ActrecParseError {
    ///  msg here should include process stage and cause of error
    pub fn to_prefix_err<T: Into<String>>(msg: T) -> Self {
        ActrecParseError::PrefixError {
            data: eyre::eyre!(msg.into()),
        }
    }

    pub fn as_prefix_err(report: eyre::Report) -> Self {
        ActrecParseError::PrefixError { data: report }
    }
    pub fn as_parse_err(report: eyre::Report) -> Self {
        ActrecParseError::ParseError { data: report }
    }

    pub fn as_skip_err(report: eyre::Report, skip_num: usize) -> Self {
        ActrecParseError::SkipError {
            data: report,
            skip_num,
        }
    }

    pub fn is_skip_err(&self) -> bool {
        matches!(self, ActrecParseError::SkipError { .. })
    }

    pub fn get_data(&self) -> &eyre::Report {
        match self {
            ActrecParseError::PrefixError { data } => data,
            ActrecParseError::ParseError { data } => data,
            ActrecParseError::SkipError { data, skip_num: _ } => data,
        }
    }

    /// Used in an intermediate process: return Ok(None) to jump to next parsing process
    pub fn to_eyre<T>(res: std::result::Result<T, ActrecParseError>) -> Result<Option<T>> {
        match res {
            Ok(val) => Ok(Some(val)),
            Err(ActrecParseError::PrefixError { data }) => {
                log::trace!("GuardParse Prefix Error: {}", data);
                Ok(None)
            }
            Err(ActrecParseError::ParseError { data }) => Err(data),
            Err(ActrecParseError::SkipError { data, skip_num: _ }) => {
                panic!(
                    "GuardParse Skip Error should not be converted to eyre Result: {}",
                    data
                );
            }
        }
    }

    pub fn to_eyre_ultimate<T>(res: std::result::Result<T, ActrecParseError>) -> Result<T> {
        match res {
            Ok(val) => Ok(val),
            Err(ActrecParseError::PrefixError { data }) => {
                log::trace!("GuardParse Prefix Error: {}", data);
                Err(data)
            }
            Err(ActrecParseError::ParseError { data }) => Err(data),
            Err(ActrecParseError::SkipError { data, skip_num: _ }) => {
                panic!(
                    "GuardParse Skip Error should not be converted to eyre Result: {}",
                    data
                );
            }
        }
    }
}
