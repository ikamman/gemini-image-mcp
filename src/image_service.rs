use crate::error::{McpError, McpResult};
use crate::validation::{get_mime_type_from_extension, Validator, ImageSourceValidator};
use base64::{engine::general_purpose, Engine as _};
use reqwest;
use std::path::Path;
use tokio::fs;
use tracing::warn;

pub struct ImageService {
    client: reqwest::Client,
    validator: ImageSourceValidator,
}

impl ImageService {
    pub fn new() -> McpResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(McpError::NetworkError)?;
            
        Ok(Self {
            client,
            validator: ImageSourceValidator,
        })
    }

    pub async fn fetch_and_encode(&self, source: &str) -> McpResult<(String, String)> {
        self.validator.validate(&source.to_string())?;
        
        let (mime_type, image_bytes) = if self.is_url(source) {
            self.fetch_from_url(source).await?
        } else {
            self.fetch_from_file(source).await?
        };
        
        if image_bytes.is_empty() {
            return Err(McpError::InvalidInput("Image file is empty".to_string()));
        }
        
        if image_bytes.len() > 20 * 1024 * 1024 {
            return Err(McpError::InvalidInput("Image file too large (max 20MB)".to_string()));
        }
        
        let encoded = general_purpose::STANDARD.encode(&image_bytes);
        Ok((mime_type, encoded))
    }

    fn is_url(&self, source: &str) -> bool {
        source.starts_with("http://") || source.starts_with("https://")
    }

    async fn fetch_from_url(&self, url: &str) -> McpResult<(String, Vec<u8>)> {
        let mime_type = self.detect_mime_type_from_url(url).await?;
        
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(McpError::NetworkError(
                reqwest::Error::from(response.error_for_status().unwrap_err())
            ));
        }
        
        let content_length = response.content_length().unwrap_or(0);
        if content_length > 20 * 1024 * 1024 {
            return Err(McpError::InvalidInput("Image too large (max 20MB)".to_string()));
        }
        
        let bytes = response.bytes().await?;
        Ok((mime_type, bytes.to_vec()))
    }

    async fn fetch_from_file(&self, file_path: &str) -> McpResult<(String, Vec<u8>)> {
        let path = Path::new(file_path);
        
        if !path.exists() {
            return Err(McpError::FileSystemError(format!("File not found: {}", file_path)));
        }
        
        if !path.is_file() {
            return Err(McpError::FileSystemError(format!("Path is not a file: {}", file_path)));
        }
        
        let metadata = fs::metadata(path).await
            .map_err(|e| McpError::FileSystemError(format!("Cannot read file metadata: {}", e)))?;
            
        if metadata.len() > 20 * 1024 * 1024 {
            return Err(McpError::InvalidInput("Image file too large (max 20MB)".to_string()));
        }
        
        let mime_type = get_mime_type_from_extension(file_path);
        let bytes = fs::read(path).await
            .map_err(|e| McpError::FileSystemError(format!("Cannot read file: {}", e)))?;
            
        Ok((mime_type, bytes))
    }

    async fn detect_mime_type_from_url(&self, url: &str) -> McpResult<String> {
        let response = self.client.head(url).send().await?;
        
        if !response.status().is_success() {
            return Err(McpError::NetworkError(
                reqwest::Error::from(response.error_for_status().unwrap_err())
            ));
        }
        
        if let Some(content_type) = response.headers().get("content-type") {
            let content_type_str = content_type.to_str()
                .map_err(|_| McpError::ContentTypeError("Invalid content-type header".to_string()))?;
            if content_type_str.starts_with("image/") {
                return Ok(content_type_str.to_string());
            } else {
                warn!("URL content-type is not an image: {}", content_type_str);
            }
        }
        
        Ok(get_mime_type_from_extension(url))
    }
}