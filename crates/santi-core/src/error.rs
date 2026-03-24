pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NotFound { resource: &'static str },
    Busy { resource: &'static str },
    InvalidInput { message: String },
    Upstream { message: String },
    Internal { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockError {
    Busy,
    Lost,
    Backend { message: String },
}
