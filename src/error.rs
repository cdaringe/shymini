use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Service not found")]
    ServiceNotFound,

    #[error("Session not found")]
    SessionNotFound,

    #[error("Invalid origin")]
    InvalidOrigin,

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("Invalid IP address: {0}")]
    InvalidIp(String),

    #[error("Invalid date range")]
    InvalidDateRange,

    #[error("GeoIP error: {0}")]
    GeoIp(#[from] maxminddb::MaxMindDBError),

    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::ServiceNotFound | Error::SessionNotFound => StatusCode::NOT_FOUND,
            Error::InvalidOrigin => StatusCode::FORBIDDEN,
            Error::InvalidUuid(_) | Error::InvalidIp(_) | Error::InvalidDateRange => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn test_error_display_service_not_found() {
        let err = Error::ServiceNotFound;
        assert_eq!(err.to_string(), "Service not found");
    }

    #[test]
    fn test_error_display_session_not_found() {
        let err = Error::SessionNotFound;
        assert_eq!(err.to_string(), "Session not found");
    }

    #[test]
    fn test_error_display_invalid_origin() {
        let err = Error::InvalidOrigin;
        assert_eq!(err.to_string(), "Invalid origin");
    }

    #[test]
    fn test_error_display_invalid_ip() {
        let err = Error::InvalidIp("bad-ip".to_string());
        assert_eq!(err.to_string(), "Invalid IP address: bad-ip");
    }

    #[test]
    fn test_error_display_invalid_date_range() {
        let err = Error::InvalidDateRange;
        assert_eq!(err.to_string(), "Invalid date range");
    }

    #[test]
    fn test_error_display_internal() {
        let err = Error::Internal("something went wrong".to_string());
        assert_eq!(err.to_string(), "Internal error: something went wrong");
    }

    #[tokio::test]
    async fn test_error_into_response_not_found() {
        let err = Error::ServiceNotFound;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_error_into_response_session_not_found() {
        let err = Error::SessionNotFound;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_error_into_response_forbidden() {
        let err = Error::InvalidOrigin;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_error_into_response_bad_request_uuid() {
        let uuid_err = uuid::Uuid::parse_str("not-a-uuid").unwrap_err();
        let err = Error::InvalidUuid(uuid_err);
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_error_into_response_bad_request_ip() {
        let err = Error::InvalidIp("bad-ip".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_error_into_response_bad_request_date() {
        let err = Error::InvalidDateRange;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_error_into_response_internal() {
        let err = Error::Internal("test".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_error_from_json_error() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    #[allow(clippy::invalid_regex)]
    fn test_error_from_regex_error() {
        let regex_err = regex::Regex::new("[invalid").unwrap_err();
        let err: Error = regex_err.into();
        assert!(matches!(err, Error::Regex(_)));
    }
}
