use anyhow::{Context, Error, Result};
use std::fmt;
use std::num::NonZeroUsize;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Format<'a> {
    pub sub_index: usize,
    sub_width: NonZeroUsize,
    pub file_index: usize,
    file_width: NonZeroUsize,
    pub rect_index: usize,
    rect_width: Option<NonZeroUsize>,
    format: &'a str,
}

impl<'a> Format<'a> {
    pub fn new(sub_count: usize, file_count: usize, format: &'a str) -> Result<Self> {
        Ok(Self {
            sub_index: 0,
            sub_width: Self::count_to_width(sub_count)?,
            file_index: 0,
            file_width: Self::count_to_width(file_count)?,
            rect_index: 0,
            rect_width: None,
            format,
        })
    }

    pub fn set_file_index(&mut self, index: usize) -> &Self {
        self.file_index = index;
        self
    }

    pub fn set_sub_index(&mut self, index: usize) -> &Self {
        self.sub_index = index;
        self
    }

    pub fn set_rect_index(&mut self, index: usize) -> &Self {
        self.rect_index = index;
        self
    }

    pub fn set_rect_count(&mut self, count: usize) -> Result<&Self> {
        self.rect_width = Some(Self::count_to_width(count)?);
        Ok(self)
    }

    fn count_to_width(count: usize) -> Result<NonZeroUsize> {
        let width: usize = count
            .checked_ilog10()
            .context("zero width")?
            .try_into()
            .with_context(|| format!("Failed to convert {} from a u32 to usize", count.ilog10()))?;
        Ok(width
            .checked_add(1)
            .ok_or(Error::msg("overflow"))?
            .try_into()
            .unwrap())
    }
}

impl<'a> fmt::Display for Format<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self
            .format
            .replace(
                "%s",
                format!("{:0width$}", self.sub_index, width = self.sub_width.get()).as_str(),
            )
            .replace(
                "%f",
                format!("{:0width$}", self.file_index, width = self.file_width.get()).as_str(),
            )
            .replace(
                "%r",
                format!(
                    "{:0width$}",
                    self.rect_index,
                    width = self.rect_width.unwrap_or(NonZeroUsize::MIN).get()
                )
                .as_str(),
            );
        write!(f, "{}", text)
    }
}
