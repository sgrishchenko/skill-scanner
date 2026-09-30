pub mod aggregation;
pub mod cli;
pub mod github;
pub mod metadata;
pub mod report;
pub mod repository;
pub mod scanner;
pub mod web;

use std::fmt;

#[derive(Debug)]
pub struct ScanError(pub(crate) String);

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScanError {}

#[cfg(test)]
mod test_support;
