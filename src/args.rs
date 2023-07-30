use anyhow::Result;
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Args {
    program: String,
    sub_files: Vec<PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            program: env!("CARGO_PKG_NAME").to_string(),
            sub_files: Default::default(),
        }
    }
}

impl Args {
    pub fn parse_from_env() -> Result<Self> {
        use lexopt::prelude::*;

        let mut args = Args::default();
        let mut parser = lexopt::Parser::from_env();

        if let Some(program) = parser.bin_name() {
            args.program = program.to_string();
        }

        while let Some(arg) = parser.next()? {
            match arg {
                Value(sub_file) => args.sub_files.push(sub_file.into()),
                _ => todo!(),
            }
        }

        Ok(args)
    }

    /*
    pub fn program(&self) -> &str {
        &self.program
    }*/

    pub fn sub_files(&self) -> impl Iterator<Item = &PathBuf> {
        self.sub_files.iter()
    }
}
