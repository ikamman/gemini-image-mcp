use std::fmt;

#[derive(Debug)]
pub enum McpError {
    NetworkError(reqwest::Error),
    IoError(std::io::Error),
    SerializationError(serde_json::Error),
    ConfigurationError(String),
    GeminiApiError { code: i32, message: String },
    FileSystemError(String),
    Base64Error(base64::DecodeError),
    InvalidInput(String),
    Timeout(String),
    AuthenticationError(String),
    RateLimitError(String),
    ContentTypeError(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::NetworkError(e) => write!(f, "Network error: {}", e),
            McpError::IoError(e) => write!(f, "IO error: {}", e),
            McpError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            McpError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            McpError::GeminiApiError { code, message } => {
                write!(f, "Gemini API error ({}): {}", code, message)
            }
            McpError::FileSystemError(msg) => write!(f, "File system error: {}", msg),
            McpError::Base64Error(e) => write!(f, "Base64 encoding error: {}", e),
            McpError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            McpError::Timeout(msg) => write!(f, "Timeout error: {}", msg),
            McpError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            McpError::RateLimitError(msg) => write!(f, "Rate limit exceeded: {}", msg),
            McpError::ContentTypeError(msg) => write!(f, "Content type error: {}", msg),
        }
    }
}

impl std::error::Error for McpError {}

impl From<reqwest::Error> for McpError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            McpError::Timeout(format!("Request timeout: {}", error))
        } else if error.is_status() {
            match error.status() {
                Some(status) if status == 401 => {
                    McpError::AuthenticationError("Invalid API key or authentication failed".to_string())
                }
                Some(status) if status == 429 => {
                    McpError::RateLimitError("API rate limit exceeded".to_string())
                }
                Some(_status) => {
                    McpError::NetworkError(error)
                }
                None => McpError::NetworkError(error)
            }
        } else {
            McpError::NetworkError(error)
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(error: std::io::Error) -> Self {
        McpError::IoError(error)
    }
}

impl From<serde_json::Error> for McpError {
    fn from(error: serde_json::Error) -> Self {
        McpError::SerializationError(error)
    }
}

impl From<base64::DecodeError> for McpError {
    fn from(error: base64::DecodeError) -> Self {
        McpError::Base64Error(error)
    }
}

pub type McpResult<T> = std::result::Result<T, McpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_display() {
        let error = McpError::InvalidInput("test message".to_string());
        assert_eq!(error.to_string(), "Invalid input: test message");
        
        let error = McpError::GeminiApiError { 
            code: 400, 
            message: "Bad request".to_string() 
        };
        assert_eq!(error.to_string(), "Gemini API error (400): Bad request");
    }

    #[test]
    fn test_error_conversions() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let mcp_error: McpError = io_error.into();
        assert!(matches!(mcp_error, McpError::IoError(_)));

        // Test JSON error conversion with a real parsing error
        let json_result: Result<serde_json::Value, serde_json::Error> = serde_json::from_str("{invalid json");
        if let Err(json_error) = json_result {
            let mcp_error: McpError = json_error.into();
            assert!(matches!(mcp_error, McpError::SerializationError(_)));
        }
    }
}