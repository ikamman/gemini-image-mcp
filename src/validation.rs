use crate::error::{McpError, McpResult};
use std::path::Path;

pub trait Validator<T> {
    fn validate(&self, input: &T) -> McpResult<()>;
}

pub struct ImageSourceValidator;

impl Validator<String> for ImageSourceValidator {
    fn validate(&self, source: &String) -> McpResult<()> {
        if source.trim().is_empty() {
            return Err(McpError::InvalidInput("Image source cannot be empty".to_string()));
        }
        
        if source.len() > 2048 {
            return Err(McpError::InvalidInput("Image source URL/path too long (max 2048 characters)".to_string()));
        }
        
        if is_url(source) {
            validate_url(source)?;
        } else {
            validate_file_path(source)?;
        }
        
        Ok(())
    }
}

pub struct PromptValidator;

impl Validator<String> for PromptValidator {
    fn validate(&self, prompt: &String) -> McpResult<()> {
        if prompt.len() > 2000 {
            return Err(McpError::InvalidInput("Prompt too long (max 2000 characters)".to_string()));
        }
        Ok(())
    }
}

pub struct OutputPathValidator;

impl Validator<String> for OutputPathValidator {
    fn validate(&self, path: &String) -> McpResult<()> {
        if path.trim().is_empty() {
            return Err(McpError::InvalidInput("Output path cannot be empty".to_string()));
        }
        
        if path.len() > 2048 {
            return Err(McpError::InvalidInput("Output path too long (max 2048 characters)".to_string()));
        }
        
        if path.contains("..") {
            return Err(McpError::InvalidInput("Path traversal not allowed in output path".to_string()));
        }
        
        let allowed_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"];
        let path_lower = path.to_lowercase();
        let has_valid_extension = allowed_extensions.iter().any(|&ext| {
            path_lower.ends_with(&format!(".{}", ext))
        });
        
        if !has_valid_extension {
            return Err(McpError::InvalidInput(
                format!("Unsupported output file extension. Allowed: {}", allowed_extensions.join(", "))
            ));
        }
        
        // Check if parent directory exists
        let path_obj = Path::new(path);
        if let Some(parent) = path_obj.parent() {
            // If parent is empty string, it means current directory
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(McpError::FileSystemError(
                    format!("Parent directory does not exist: {}", parent.display())
                ));
            }
        }
        
        Ok(())
    }
}


fn is_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn validate_url(url: &str) -> McpResult<()> {
    if !url.starts_with("https://") {
        return Err(McpError::InvalidInput("Only HTTPS URLs are allowed for security".to_string()));
    }
    
    if !url.contains('.') {
        return Err(McpError::InvalidInput("Invalid URL format".to_string()));
    }
    
    Ok(())
}

fn validate_file_path(path: &str) -> McpResult<()> {
    if path.contains("..") {
        return Err(McpError::InvalidInput("Path traversal not allowed".to_string()));
    }
    
    let allowed_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"];
    let path_lower = path.to_lowercase();
    let has_valid_extension = allowed_extensions.iter().any(|&ext| {
        path_lower.ends_with(&format!(".{}", ext))
    });
    
    if !has_valid_extension {
        return Err(McpError::InvalidInput(
            format!("Unsupported file extension. Allowed: {}", allowed_extensions.join(", "))
        ));
    }
    
    Ok(())
}

pub fn get_mime_type_from_extension(path: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "png" => "image/png".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "bmp" => "image/bmp".to_string(),
        "tiff" | "tif" => "image/tiff".to_string(),
        _ => "image/jpeg".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_source_validator() {
        let validator = ImageSourceValidator;
        
        assert!(validator.validate(&"test.jpg".to_string()).is_ok());
        assert!(validator.validate(&"https://example.com/image.png".to_string()).is_ok());
        
        assert!(validator.validate(&"".to_string()).is_err());
        assert!(validator.validate(&"   ".to_string()).is_err());
        assert!(validator.validate(&"test.txt".to_string()).is_err());
        assert!(validator.validate(&"../../../etc/passwd".to_string()).is_err());
        assert!(validator.validate(&"http://insecure.com/image.jpg".to_string()).is_err());
        
        let long_path = "x".repeat(3000);
        assert!(validator.validate(&long_path).is_err());
    }

    #[test]
    fn test_prompt_validator() {
        let validator = PromptValidator;
        
        assert!(validator.validate(&"Normal prompt".to_string()).is_ok());
        assert!(validator.validate(&"".to_string()).is_ok());
        
        let long_prompt = "x".repeat(3000);
        assert!(validator.validate(&long_prompt).is_err());
    }

    #[test]
    fn test_get_mime_type_from_extension() {
        assert_eq!(get_mime_type_from_extension("image.jpg"), "image/jpeg");
        assert_eq!(get_mime_type_from_extension("photo.jpeg"), "image/jpeg");
        assert_eq!(get_mime_type_from_extension("logo.png"), "image/png");
        assert_eq!(get_mime_type_from_extension("animation.gif"), "image/gif");
        assert_eq!(get_mime_type_from_extension("modern.webp"), "image/webp");
        assert_eq!(get_mime_type_from_extension("bitmap.bmp"), "image/bmp");
        assert_eq!(get_mime_type_from_extension("scan.tiff"), "image/tiff");
        assert_eq!(get_mime_type_from_extension("scan.tif"), "image/tiff");
        
        // Test case insensitive
        assert_eq!(get_mime_type_from_extension("IMAGE.JPG"), "image/jpeg");
        assert_eq!(get_mime_type_from_extension("PHOTO.PNG"), "image/png");
        
        // Test unknown extensions fall back to JPEG
        assert_eq!(get_mime_type_from_extension("file.unknown"), "image/jpeg");
        assert_eq!(get_mime_type_from_extension("no_extension"), "image/jpeg");
        
        // Test files with paths
        assert_eq!(get_mime_type_from_extension("/path/to/image.png"), "image/png");
        assert_eq!(get_mime_type_from_extension("./relative/path.webp"), "image/webp");
    }

    #[test]
    fn test_output_path_validator() {
        let validator = OutputPathValidator;
        
        // Valid paths (assuming current directory exists)
        assert!(validator.validate(&"./test.jpg".to_string()).is_ok());
        assert!(validator.validate(&"image.png".to_string()).is_ok());
        
        // Invalid cases
        assert!(validator.validate(&"".to_string()).is_err());
        assert!(validator.validate(&"   ".to_string()).is_err());
        assert!(validator.validate(&"../../../etc/passwd.jpg".to_string()).is_err());
        assert!(validator.validate(&"image.txt".to_string()).is_err());
        assert!(validator.validate(&"/nonexistent/directory/image.jpg".to_string()).is_err());
        
        let long_path = format!("{}.jpg", "x".repeat(3000));
        assert!(validator.validate(&long_path).is_err());
    }
}