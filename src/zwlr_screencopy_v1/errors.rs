use std::fmt;

#[derive(Debug, Default, Clone, Copy)]
pub enum ScreencopyHandlerErrorCodes {
    #[default]
    UnknownError,
    WLOutputIsNotSet,
    WlShmIsNotSet,
    ScreencopyFrameIsNotSet,
    ScreencopyBufferIsNotSet,
    ScreencopyFrameBufferFormatError,
    ScreencopyFrameBufferFormatUnsupported,
    SharedMemFdCreateFailed,
}

impl fmt::Display for ScreencopyHandlerErrorCodes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ScreencopyHandlerErrorCodes::UnknownError => write!(f, "ScreencopyHandlerErrorCodes: UnknownError"),
            ScreencopyHandlerErrorCodes::WLOutputIsNotSet => write!(f, "ScreencopyHandlerErrorCodes: WLOutputIsNotSet"),
            ScreencopyHandlerErrorCodes::WlShmIsNotSet => write!(f, "ScreencopyHandlerErrorCodes: WlShmIsNotSet"),
            ScreencopyHandlerErrorCodes::ScreencopyFrameIsNotSet => write!(f, "ScreencopyHandlerErrorCodes: ScreencopyFrameIsNotSet"),
            ScreencopyHandlerErrorCodes::ScreencopyBufferIsNotSet => write!(f, "ScreencopyHandlerErrorCodes: ScreencopyBufferIsNotSet"),
            ScreencopyHandlerErrorCodes::ScreencopyFrameBufferFormatError => write!(f, "ScreencopyHandlerErrorCodes: ScreencopyFrameBufferFormatError"),
            ScreencopyHandlerErrorCodes::ScreencopyFrameBufferFormatUnsupported => write!(f, "ScreencopyHandlerErrorCodes: ScreencopyFrameBufferFormatUnsupported"),
            ScreencopyHandlerErrorCodes::SharedMemFdCreateFailed => write!(f, "ScreencopyHandlerErrorCodes: SharedMemFdCreateFailed"),

        }
    }
}

#[derive(Debug)]
pub struct ScreencopyHandlerError {
    pub code: ScreencopyHandlerErrorCodes,
    pub message: String,
}

impl std::fmt::Display for ScreencopyHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ScreencopyHandlerErrorCodes:(code: {:?}, message: {})",
            self.code, self.message
        )
    }
}

impl ScreencopyHandlerError {
    pub fn new(code: ScreencopyHandlerErrorCodes, message: String) -> Self {
        Self { code, message }
    }
}