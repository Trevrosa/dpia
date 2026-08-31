use core::{num::ParseIntError, str::Utf8Error};

use reqwless::response::StatusCode;

#[derive(defmt::Format)]
pub enum RequestError {
    Reqwless(reqwless::Error),
    Http(StatusCode),
}

impl From<reqwless::Error> for RequestError {
    fn from(value: reqwless::Error) -> Self {
        Self::Reqwless(value)
    }
}

#[derive(defmt::Format)]
pub enum SyncError<T> {
    Request(RequestError),
    Parse(T),
}

impl<E, T> From<E> for SyncError<T>
where
    E: Into<RequestError>,
{
    fn from(value: E) -> Self {
        Self::Request(value.into())
    }
}

impl From<ParseIntError> for SyncError<()> {
    fn from(_value: ParseIntError) -> Self {
        Self::Parse(())
    }
}

impl From<ParseError> for SyncError<ParseError> {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<Utf8Error> for SyncError<ParseError> {
    fn from(value: Utf8Error) -> Self {
        Self::Parse(value.into())
    }
}

pub enum ParseError {
    Deserialize(miniserde::Error),
    /// error msg
    Format(&'static str),
}

impl defmt::Format for ParseError {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            ParseError::Deserialize(_) => defmt::write!(fmt, r#"Deserialize("miniserde error")"#),
            ParseError::Format(e) => defmt::write!(fmt, "Format({:?})", e),
        }
    }
}

impl From<Utf8Error> for ParseError {
    fn from(_: Utf8Error) -> Self {
        Self::Format("invalid utf8")
    }
}

impl From<miniserde::Error> for ParseError {
    fn from(value: miniserde::Error) -> Self {
        Self::Deserialize(value)
    }
}

impl From<()> for ParseError {
    fn from(_: ()) -> Self {
        Self::Format("unexpected value")
    }
}
