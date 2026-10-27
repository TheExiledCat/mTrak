use std::{
    error::Error,
    fmt::{self, Display},
    io,
};

use rmp_serde::{decode::Error as DecodeError, encode::Error as EncodeError};

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
    /// A config file, e.g. config.toml or a theme, that can't be read.
    InvalidConfig(String),
    /// A standard folder, e.g. the music folder, that the OS doesn't provide.
    MissingDirectory(&'static str),
    /// A feature that isn't implemented yet.
    Unsupported(&'static str),
    /// When pasting patterns only, if the pattern buffer's length differs from the current pattern's. Not thrown for track or block buffers as those are truncated.
    PatternPasteLength {
        copied_length: u16,
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
            MTrakError::InvalidConfig(_) => "Config error",
            MTrakError::Parse(_) => "Invalid input",
            MTrakError::Unsupported(_) => "Not supported",
            MTrakError::PatternPasteLength { .. } => "Paste length",
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
            MTrakError::UnsupportedProjectVersion { found, supported } if found > supported => {
                write!(
                    f,
                    "This project was made with a newer mTrak (file version {}, this version reads {})",
                    found, supported
                )
            }
            MTrakError::UnsupportedProjectVersion { found, supported } => write!(
                f,
                "This project was made with an older mTrak that this version can't read (file version {}, this version reads {})",
                found, supported
            ),
            MTrakError::InvalidConfig(reason) => write!(f, "Invalid config file {}", reason),
            MTrakError::MissingDirectory(name) => {
                write!(f, "Could not find the {} folder on this system", name)
            }
            MTrakError::Unsupported(feature) => write!(f, "{} is not supported yet", feature),
            MTrakError::PatternPasteLength { copied_length } => write!(
                f,
                "Pattern buffer is {} rows long, unlike the current pattern",
                copied_length
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
