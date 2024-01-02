use std::fmt;

#[derive(Debug, Default, Clone, Copy)]
pub enum SessionLockhandlerErrorCodes {
    #[default]
    UnknownError,
    ManagerIsNotSet,
    WLOutputIsNotSet,
    WLSurfaceIsNotSet,
    SessionLockIsNotSet,
    SessionLockSurfaceIsNotSet,
    SessionLockSurfaceSerialIsNotSet,
}

impl fmt::Display for SessionLockhandlerErrorCodes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SessionLockhandlerErrorCodes::UnknownError => write!(f, "SessionLockhandlerErrorCodes: UnknownError"),
            SessionLockhandlerErrorCodes::ManagerIsNotSet => write!(f, "SessionLockhandlerErrorCodes: ManagerIsNotSet"),
            SessionLockhandlerErrorCodes::WLOutputIsNotSet => write!(f, "SessionLockhandlerErrorCodes: NoWLOutputFound"),
            SessionLockhandlerErrorCodes::WLSurfaceIsNotSet => write!(f, "SessionLockhandlerErrorCodes: WLSurfaceIsNotSet"),
            SessionLockhandlerErrorCodes::SessionLockIsNotSet => write!(f, "SessionLockhandlerErrorCodes: SessionLockIsNotSet"),
            SessionLockhandlerErrorCodes::SessionLockSurfaceIsNotSet => write!(f, "SessionLockhandlerErrorCodes: SessionLockSurfaceIsNotSet"),
            SessionLockhandlerErrorCodes::SessionLockSurfaceSerialIsNotSet => write!(f, "SessionLockhandlerErrorCodes: SessionLockSurfaceSerialIsNotSet"),
        }
    }
}

#[derive(Debug)]
pub struct SessionLockhandlerError {
    pub code: SessionLockhandlerErrorCodes,
    pub message: String,
}

impl std::fmt::Display for SessionLockhandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SessionLockhandlerErrorCodes:(code: {:?}, message: {})",
            self.code, self.message
        )
    }
}

impl SessionLockhandlerError {
    pub fn new(code: SessionLockhandlerErrorCodes, message: String) -> Self {
        Self { code, message }
    }
}