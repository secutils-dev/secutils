use std::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

/// Represents possible errors that can happen during signup.
#[derive(Debug)]
pub enum UserSignupError {
    EmailAlreadyRegistered,
}

impl Display for UserSignupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Error for UserSignupError {}
