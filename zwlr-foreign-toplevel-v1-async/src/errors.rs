use std::fmt;

#[derive(Debug, Default, Clone, Copy)]
pub enum ToplevelHandlerErrorCodes {
    #[default]
    UnknownError,
    ToplevelNotFound,
}

impl fmt::Display for ToplevelHandlerErrorCodes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ToplevelHandlerErrorCodes::UnknownError => write!(f, "ToplevelHandlerErrorCodes: UnknownError"),
            ToplevelHandlerErrorCodes::ToplevelNotFound => {
                write!(f, "ToplevelHandlerErrorCodes: ToplevelNotFound")
            }
        }
    }
}

#[derive(Debug)]
pub struct ToplevelHandlerError {
    pub code: ToplevelHandlerErrorCodes,
    pub message: String,
}

impl std::fmt::Display for ToplevelHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ToplevelHandlerErrorCodes:(code: {:?}, message: {})",
            self.code, self.message
        )
    }
}

impl ToplevelHandlerError {
    pub fn new(code: ToplevelHandlerErrorCodes, message: String) -> Self {
        Self { code, message }
    }
}