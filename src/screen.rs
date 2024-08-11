pub mod dashboard;

#[derive(Debug, Clone)]
pub enum Notification {
    Error(String),
    Info(String),
    Warn(String),
}

#[derive(Debug, Clone)]
pub enum Error {
    FetchError(String),
    ParseError(String),
    PaneSetError(String),
    StreamError(String),
    UnknownError(String),
}