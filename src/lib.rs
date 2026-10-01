pub mod aggregation;
pub mod cache;
pub mod cli;
pub mod github;
pub mod metadata;
pub mod organization;
pub mod recent;
pub mod report;
pub mod repository;
pub mod scanner;
pub mod starred;
mod storage;
pub mod web;

use std::fmt;

#[derive(Debug)]
pub struct ScanError {
    message: String,
    /// Credential, rate-limit, and connection failures also affect every later
    /// request, so organization scans stop instead of trying more repositories.
    service: bool,
}

impl ScanError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            service: false,
        }
    }

    pub(crate) fn service(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            service: true,
        }
    }

    pub(crate) fn is_service(&self) -> bool {
        self.service
    }

    pub(crate) fn context(self, context: &str) -> Self {
        Self {
            message: format!("{context}: {}", self.message),
            ..self
        }
    }
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ScanError {}

#[cfg(test)]
mod test_support;
