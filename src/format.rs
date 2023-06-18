use anyhow::{Error, Result};
use std::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Format<'a> {
    pub sub_index: usize,
    sub_width: usize,
    pub file_index: usize,
    file_width: usize,
    format: &'a str,
}

impl<'a> Format<'a> {
    pub fn new(sub_count: usize, file_count: usize, format: &'a str) -> Result<Self> {
        let sub_width: usize = sub_count
            .checked_ilog10()
            .ok_or(Error::msg("no subtitles"))?
            .try_into()?;
        let file_width: usize = file_count
            .checked_ilog10()
            .ok_or(Error::msg("no files"))?
            .try_into()?;
        Ok(Self {
            sub_index: 0,
            sub_width: sub_width + 1,
            file_index: 0,
            file_width: file_width + 1,
            format,
        })
    }
}

impl<'a> fmt::Display for Format<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self
            .format
            .replace(
                "%s",
                format!("{:0width$}", self.sub_index, width = self.sub_width).as_str(),
            )
            .replace(
                "%f",
                format!("{:0width$}", self.file_index, width = self.file_width).as_str(),
            );
        write!(f, "{}", text)
    }
}
