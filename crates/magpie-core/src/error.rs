use serde::ser::{Serialize, SerializeStruct, Serializer};

/// Errors returned by core operations. Serialised to the frontend as
/// `{ code, message, existingId? }` so the UI can react to specific cases.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Something went wrong with the local database.")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    InvalidUrl(String),
    #[error("This URL is already in your library.")]
    Duplicate { existing_id: String },
    #[error("{0} not found.")]
    NotFound(&'static str),
    #[error("{0}")]
    Validation(String),
    #[error("Could not read or write a file.")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Db(_) => "DATABASE_ERROR",
            Error::InvalidUrl(_) => "INVALID_URL",
            Error::Duplicate { .. } => "DUPLICATE_BOOKMARK",
            Error::NotFound(_) => "NOT_FOUND",
            Error::Validation(_) => "VALIDATION_ERROR",
            Error::Io(_) => "IO_ERROR",
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Error", 3)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        match self {
            Error::Duplicate { existing_id } => st.serialize_field("existingId", existing_id)?,
            _ => st.serialize_field("existingId", &Option::<String>::None)?,
        }
        st.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
