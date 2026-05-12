// This file is part of Astarte.
//
// Copyright 2026 SECO Mind Srl
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// SPDX-License-Identifier: Apache-2.0

//! Generic error type to store a context and message information.
//!
//! It can be used to retun
//!
//! # Example
//!
//! This shows how to create and error that wraps [`ErrorKind`](std::io::ErrorKind).
//!
//! ```
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum ErrorKind {
//!     Io(io::ErrorKind),
//!     _Other,
//!     _Errors,
//! }
//!
//! impl Display for ErrorKind {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         match self {
//!             ErrorKind::_Some => write!(f, "some error"),
//!             ErrorKind::Io(error_kind) => write!(f, "io error {error_kind}"),
//!         }
//!     }
//! }
//!
//! let err = Error::with(ErrorKind::Io(io::ErrorKind::NotFound), "while reading")
//!     .message(Path::new("/foo/bar").display())
//!     .source(io::Error::from(io::ErrorKind::NotFound));
//!
//! let display = err.to_string();
//!
//! let exp = "io error entity not found while reading /foo/bar";
//!
//! assert_eq!(display, exp);
//! ```

#![warn(
    clippy::dbg_macro,
    clippy::todo,
    missing_docs,
    rustdoc::missing_crate_level_docs
)]

use std::fmt::{Debug, Display};

trait DebugDisplay: Display + Debug {}

impl<T> DebugDisplay for T where T: Display + Debug {}

/// Generic error struct to store the information.
///
/// It provides a way to store:
///
/// - An error kind
/// - A static context message
/// - A dynamic display message
/// - A generic error source
#[derive(Debug)]
#[must_use]
pub struct Error<K> {
    kind: K,
    ctx: Option<&'static str>,
    message: Option<Box<dyn DebugDisplay>>,
    source: Option<Box<dyn std::error::Error>>,
}

impl<K> Error<K> {
    /// Create a new error for the kind
    pub fn new(kind: K) -> Self
    where
        K: Display + Debug + PartialEq,
    {
        Self {
            kind,
            ctx: None,
            message: None,
            source: None,
        }
    }

    /// Create a new error for the kind and the given context
    pub fn with(kind: K, ctx: &'static str) -> Self
    where
        K: Display + Debug + PartialEq,
    {
        Self {
            kind,
            ctx: Some(ctx),
            message: None,
            source: None,
        }
    }

    /// Sets the message for the error
    pub fn set_message<T>(mut self, message: T) -> Self
    where
        T: Display + Debug + 'static,
    {
        self.message = Some(Box::new(message));
        self
    }

    /// Sets the error source
    pub fn source<T>(mut self, source: T) -> Self
    where
        T: std::error::Error + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns the error kind.
    pub fn kind(&self) -> &K {
        &self.kind
    }
}

impl<K> Display for Error<K>
where
    K: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            kind,
            ctx,
            message,
            source,
        } = self;

        write!(f, "{kind}")?;

        if let Some(ctx) = ctx {
            write!(f, " {ctx}")?;
        }

        if let Some(message) = message {
            write!(f, " {message}")?;
        }

        if let Some(source) = source
            && f.alternate()
        {
            write!(f, ": {source}")?;
        }

        Ok(())
    }
}

impl<K> std::error::Error for Error<K>
where
    K: Debug + Display,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ErrorKind {
        _Some,
        Io(io::ErrorKind),
    }

    impl Display for ErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ErrorKind::_Some => write!(f, "some error"),
                ErrorKind::Io(error_kind) => write!(f, "io error {error_kind}"),
            }
        }
    }

    #[test]
    fn should_display() {
        let err = Error::with(ErrorKind::Io(io::ErrorKind::NotFound), "while reading")
            .set_message(Path::new("/foo/bar").display())
            .source(io::Error::from(io::ErrorKind::NotFound));

        let display = err.to_string();

        let exp = "io error entity not found while reading /foo/bar";

        assert_eq!(display, exp);
    }

    #[test]
    fn should_display_alternate() {
        let err = Error::with(ErrorKind::Io(io::ErrorKind::NotFound), "while reading")
            .set_message(Path::new("/foo/bar").display())
            .source(io::Error::from(io::ErrorKind::NotFound));

        let display = format!("{err:#}");

        let exp = "io error entity not found while reading /foo/bar: entity not found";

        assert_eq!(display, exp);
    }

    #[test]
    fn should_check_size() {
        let kind = ErrorKind::Io(io::ErrorKind::NotFound);
        let err = Error::with(kind, "while reading")
            .set_message(Path::new("/foo/bar").display())
            .source(io::Error::from(io::ErrorKind::NotFound));

        let full_size = size_of_val(&err);

        assert_eq!(full_size, 56);

        let kind_size = size_of_val(&kind);

        assert_eq!(kind_size, 1);

        let err_size = full_size - kind_size;

        assert_eq!(err_size, 55);
    }
}
