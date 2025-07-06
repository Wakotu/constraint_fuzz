use color_eyre::eyre::Result;
pub enum GuardParseError {
    PrefixError { data: eyre::Report },
    ParseError { data: eyre::Report },
}

impl From<eyre::Report> for GuardParseError {
    fn from(err: eyre::Report) -> Self {
        GuardParseError::ParseError { data: err }
    }
}

impl GuardParseError {
    pub fn to_prefix_err(report: eyre::Report) -> Self {
        GuardParseError::PrefixError { data: report }
    }
    pub fn to_parse_err(report: eyre::Report) -> Self {
        GuardParseError::ParseError { data: report }
    }
}

pub fn handle_guard_err_result<T>(
    res: std::result::Result<T, GuardParseError>,
) -> Result<Option<T>> {
    match res {
        Ok(val) => Ok(Some(val)),
        Err(GuardParseError::PrefixError { data }) => {
            log::trace!("GuardParse Prefix Error: {}", data);
            Ok(None)
        }
        Err(GuardParseError::ParseError { data }) => Err(data),
    }
}
