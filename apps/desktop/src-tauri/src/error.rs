use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct AppError {
    pub message: String,
}

impl From<toasty::Error> for AppError {
    fn from(error: toasty::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for AppError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}
