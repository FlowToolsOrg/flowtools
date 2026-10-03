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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serializes_as_a_safe_message_object() {
        let error = AppError::from("Plugin id is invalid.");
        let value = serde_json::to_value(error).expect("app error should serialize");

        assert_eq!(
            value,
            serde_json::json!({ "message": "Plugin id is invalid." })
        );
    }
}
