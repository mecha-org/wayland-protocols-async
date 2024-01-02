use std::fmt;

#[derive(Debug, Default, Clone, Copy)]
pub enum OutputManagementHandlerErrorCodes {
    #[default]
    UnknownError,
    RemoveModeError,
    HeadNotFoundError,
    ModeNotFoundError,
    NoSerialManagerNotReady
}

impl fmt::Display for OutputManagementHandlerErrorCodes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OutputManagementHandlerErrorCodes::UnknownError => write!(f, "OutputManagementHandlerErrorCodes: UnknownError"),
            OutputManagementHandlerErrorCodes::RemoveModeError => {
                write!(f, "OutputManagementHandlerErrorCodes: RemoveModeError")
            },
            OutputManagementHandlerErrorCodes::HeadNotFoundError => {
                write!(f, "OutputManagementHandlerErrorCodes: HeadNotFoundError")
            },
            OutputManagementHandlerErrorCodes::ModeNotFoundError => {
                write!(f, "OutputManagementHandlerErrorCodes: ModeNotFoundError")
            },
            OutputManagementHandlerErrorCodes::NoSerialManagerNotReady => {
                write!(f, "OutputManagementHandlerErrorCodes: NoSerialManagerNotReady")
            }
        }
    }
}

#[derive(Debug)]
pub struct OutputManagementHandlerError {
    pub code: OutputManagementHandlerErrorCodes,
    pub message: String,
}

impl std::fmt::Display for OutputManagementHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OutputManagementHandlerErrorCodes:(code: {:?}, message: {})",
            self.code, self.message
        )
    }
}

impl OutputManagementHandlerError {
    pub fn new(code: OutputManagementHandlerErrorCodes, message: String) -> Self {
        Self { code, message }
    }
}