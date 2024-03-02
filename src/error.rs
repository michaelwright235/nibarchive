/// Variants of error that may occur during encoding/decoding a NIB Archive.
#[derive(Debug)]
pub enum Error {
    /// An IO error that may occur during opening/reading/writing a file.
    IOError(std::io::Error),

    /// A format error that may occur only during decoding a NIB Archive.
    /// Usually it indicates a malformed file.
    FormatError(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IOError(e) => f.write_fmt(format_args!("IOError: {e}")),
            Error::FormatError(e) => f.write_fmt(format_args!("NIB Archive format error: {e}")),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::FormatError(format!("Unable to parse UTF-8 string. {value}"))
    }
}
