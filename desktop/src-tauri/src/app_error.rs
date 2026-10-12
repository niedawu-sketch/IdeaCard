use std::fmt::{self, Display, Formatter};

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub recovery: String,
}

impl AppError {
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            recovery: recovery.into(),
        }
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(_error: rusqlite::Error) -> Self {
        Self::new(
            "database_operation_failed",
            "数据库操作失败",
            "重新打开应用；问题持续时检查数据目录",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;
    use serde_json::json;

    const DATABASE_ERROR_MESSAGE: &str = "数据库操作失败";
    const DATABASE_ERROR_RECOVERY: &str = "重新打开应用；问题持续时检查数据目录";
    const SENTINEL: &str = "SQL /Users/private/ideacard.sqlite3 SENTINEL";

    #[test]
    fn serializes_the_stable_camel_case_contract() {
        let error = AppError::new("capture_failed", "无法捕获内容", "请重试");

        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({
                "code": "capture_failed",
                "message": "无法捕获内容",
                "recovery": "请重试",
            })
        );
    }

    #[test]
    fn implements_the_standard_error_contract() {
        fn assert_standard_error<T: std::error::Error>() {}

        assert_standard_error::<AppError>();

        let error = AppError::new("capture_failed", "无法捕获内容", "请重试");
        assert_eq!(error.to_string(), "无法捕获内容");
    }

    #[test]
    fn converts_database_errors_without_exposing_internal_details() {
        let error = AppError::from(rusqlite::Error::InvalidPath(SENTINEL.into()));
        let display = error.to_string();
        let debug = format!("{error:?}");
        let json = serde_json::to_string(&error).unwrap();

        assert_eq!(error.code, "database_operation_failed");
        assert_eq!(error.message, DATABASE_ERROR_MESSAGE);
        assert_eq!(error.recovery, DATABASE_ERROR_RECOVERY);
        assert_eq!(display, DATABASE_ERROR_MESSAGE);
        assert!(!display.contains(SENTINEL));
        assert!(!debug.contains(SENTINEL));
        assert!(!json.contains(SENTINEL));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            json!({
                "code": "database_operation_failed",
                "message": DATABASE_ERROR_MESSAGE,
                "recovery": DATABASE_ERROR_RECOVERY,
            })
        );
    }
}
