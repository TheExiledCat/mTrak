use std::{
    error::Error,
    fmt::{self, Display},
    io,
};

use bincode::error::{DecodeError, EncodeError};

/// Everything in mTrak that can fail returns this, so errors can be passed up with `?` and
/// shown to the user in one place.
#[derive(Debug)]
pub enum MTrakError {
    Io(io::Error),
    Encode(EncodeError),
    Decode(DecodeError),
    Midi(String),
    /// Text that should have been a note, command or value but wasn't.
    Parse(String),
    /// A file that isn't an mTrak project, or is damaged.
    InvalidProject(String),
    UnsupportedProjectVersion {
        found: u16,
        supported: u16,
    },
    /// A standard folder, e.g. the music folder, that the OS doesn't provide.
    MissingDirectory(&'static str),
    /// A feature that isn't implemented yet.
    Unsupported(&'static str),
    /// When pasting patterns only, if the pattern buffer is larger than the current patterns length. Not thrown for track or block buffers as those are truncated.
    PatternPasteOverflow {
        overflow_length: u8,
    },
}

pub type Result<T> = std::result::Result<T, MTrakError>;

impl MTrakError {
    /// A short title for the dialog that shows this error.
    pub fn title(&self) -> &'static str {
        return match self {
            MTrakError::Io(_) | MTrakError::MissingDirectory(_) => "File error",
            MTrakError::Encode(_)
            | MTrakError::Decode(_)
            | MTrakError::InvalidProject(_)
            | MTrakError::UnsupportedProjectVersion { .. } => "Project error",
            MTrakError::Midi(_) => "MIDI error",
            MTrakError::Parse(_) => "Invalid input",
            MTrakError::Unsupported(_) => "Not supported",
            MTrakError::PatternPasteOverflow { .. } => "Paste overflow",
        };
    }
}

impl Display for MTrakError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        return match self {
            MTrakError::Io(e) => write!(f, "{}", e),
            MTrakError::Encode(e) => write!(f, "Could not write the project: {}", e),
            MTrakError::Decode(e) => write!(f, "Could not read the project: {}", e),
            MTrakError::Midi(message) => write!(f, "{}", message),
            MTrakError::Parse(text) => write!(f, "Could not parse: {}", text),
            MTrakError::InvalidProject(reason) => write!(f, "Not a valid project: {}", reason),
            MTrakError::UnsupportedProjectVersion { found, supported } => write!(
                f,
                "This project was made with a newer mTrak (file version {}, supported up to {})",
                found, supported
            ),
            MTrakError::MissingDirectory(name) => {
                write!(f, "Could not find the {} folder on this system", name)
            }
            MTrakError::Unsupported(feature) => write!(f, "{} is not supported yet", feature),
            MTrakError::PatternPasteOverflow { overflow_length } => write!(
                f,
                "Pattern buffer is larger than current pattern by {} rows",
                overflow_length
            ),
        };
    }
}

impl Error for MTrakError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        return match self {
            MTrakError::Io(e) => Some(e),
            MTrakError::Encode(e) => Some(e),
            MTrakError::Decode(e) => Some(e),
            _ => None,
        };
    }
}

impl From<io::Error> for MTrakError {
    fn from(error: io::Error) -> Self {
        return MTrakError::Io(error);
    }
}

impl From<EncodeError> for MTrakError {
    fn from(error: EncodeError) -> Self {
        return MTrakError::Encode(error);
    }
}

impl From<DecodeError> for MTrakError {
    fn from(error: DecodeError) -> Self {
        return MTrakError::Decode(error);
    }
}

impl From<midir::InitError> for MTrakError {
    fn from(error: midir::InitError) -> Self {
        return MTrakError::Midi(error.to_string());
    }
}
