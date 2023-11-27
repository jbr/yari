use thiserror::Error;
use trillium_client::ClientSerdeError;
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    String(String),

    #[error("{0}")]
    Str(&'static str),

    #[error(transparent)]
    Http(#[from] trillium_http::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    TomlDe(#[from] toml::de::Error),

    #[error(transparent)]
    BincodeDe(#[from] bincode::Error),

    #[error(transparent)]
    UnexpectedStatus(#[from] trillium_client::UnexpectedStatusError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::Str(s)
    }
}

impl From<ClientSerdeError> for Error {
    fn from(cse: ClientSerdeError) -> Self {
        match cse {
            ClientSerdeError::HttpError(h) => h.into(),
            ClientSerdeError::JsonError(j) => j.into(),
        }
    }
}
