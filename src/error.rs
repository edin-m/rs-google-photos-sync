use std::error::Error;
use std::fmt;
use std::io;
use std::net;
use std::convert::From;

use reqwest;

use serde::Deserialize;
use serde_json;

pub type CustomResult<T> = Result<T, CustomError>;

#[derive(Debug, Deserialize)]
pub enum CustomError {
    Err(String)
}

impl Error for CustomError {
    fn description(&self) -> &str {
        match *self {
            CustomError::Err(ref err) => err
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            CustomError::Err(_) => None
        }
    }
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CustomError::Err(ref s) => fmt::Display::fmt(s, f)
        }
    }
}

impl From<io::Error> for CustomError {
    fn from(e: io::Error) -> Self {
        CustomError::Err(e.to_string())
    }
}

impl From<reqwest::Error> for CustomError {
    fn from(e: reqwest::Error) -> Self {
        CustomError::Err(e.description().to_string())
    }
}

impl From<serde_json::Error> for CustomError {
    fn from(s: serde_json::Error) -> Self {
        CustomError::Err(s.to_string())
    }
}

impl From<std::num::ParseIntError> for CustomError {
    fn from(s: std::num::ParseIntError) -> Self {
        CustomError::Err(s.to_string())
    }
}

impl From<reqwest::header::InvalidHeaderValue> for CustomError {
    fn from(e: reqwest::header::InvalidHeaderValue) -> Self {
        CustomError::Err(format!("InvalidHeaderValue {}", e.to_string()))
    }
}

impl From<opener::OpenError> for CustomError {
    fn from(e: opener::OpenError) -> Self {
        CustomError::Err(format!("opener::OpenError {}", e.to_string()))
    }
}

impl From<std::boxed::Box<dyn std::any::Any + std::marker::Send>> for CustomError {
    fn from(e: std::boxed::Box<dyn std::any::Any + std::marker::Send>) -> Self {
        CustomError::Err(format!("error joining thread"))
    }
}
