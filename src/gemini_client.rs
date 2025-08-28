use crate::error::{McpError, McpResult};
use crate::image_service::ImageService;
use crate::validation::{ImageSourceValidator, OutputPathValidator, PromptValidator, Validator};
use base64::{Engine as _, engine::general_purpose};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::error;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzeImageInput {
    /// Image source: can be a URL (http/https) or a local file path
    pub image_source: String,
    /// Optional system prompt to guide the image analysis
    pub system_prompt: Option<String>,
    /// User prompt for analysis. Defaults to "Caption this image."
    pub user_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GenerateImageInput {
    /// Optional system prompt to guide the image generation
    pub system_prompt: Option<String>,
    /// User prompt describing the image to generate
    pub user_prompt: String,
    /// Output file path where the generated image will be saved
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditImageInput {
    /// Input image source: can be a URL (http/https) or a local file path
    pub image_source: String,
    /// Optional system prompt to guide the image editing
    pub system_prompt: Option<String>,
    /// User prompt describing the desired edits to the image
    pub user_prompt: String,
    /// Output file path where the edited image will be saved
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    InlineData { inline_data: InlineData },
    Text { text: String },
}

#[derive(Debug, Serialize, Deserialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    error: Option<GeminiError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiImageResponse {
    candidates: Option<Vec<GeminiImageCandidate>>,
    error: Option<GeminiError>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiImageCandidate {
    content: GeminiImageContent,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiImageContent {
    parts: Vec<GeminiImagePart>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiImagePart {
    InlineData { inline_data: InlineData },
}

pub struct GeminiClient {
    client: reqwest::Client,
    image_service: ImageService,
    prompt_validator: PromptValidator,
    output_path_validator: OutputPathValidator,
    image_source_validator: ImageSourceValidator,
    api_key: String,
}

impl GeminiClient {
    pub fn new(api_key: String) -> McpResult<Self> {
        if api_key.trim().is_empty() {
            return Err(McpError::AuthenticationError(
                "API key is empty".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(McpError::NetworkError)?;

        Ok(Self {
            client,
            image_service: ImageService::new()?,
            prompt_validator: PromptValidator,
            output_path_validator: OutputPathValidator,
            image_source_validator: ImageSourceValidator,
            api_key,
        })
    }

    pub async fn analyze_image(&self, input: &AnalyzeImageInput) -> McpResult<String> {
        let user_prompt = input
            .user_prompt
            .as_deref()
            .unwrap_or("Caption this image.");
        self.prompt_validator.validate(&user_prompt.to_string())?;

        if let Some(ref system_prompt) = input.system_prompt {
            self.prompt_validator.validate(system_prompt)?;
        }

        let (mime_type, encoded_image) = self
            .image_service
            .fetch_and_encode(&input.image_source)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch and encode image '{}': {}",
                    input.image_source, e
                );
                e
            })?;

        let mut parts = vec![];

        // Add system prompt if provided
        if let Some(ref system_prompt) = input.system_prompt {
            parts.push(GeminiPart::Text {
                text: system_prompt.clone(),
            });
        }

        // Add the image
        parts.push(GeminiPart::InlineData {
            inline_data: InlineData {
                mime_type,
                data: encoded_image,
            },
        });

        // Add user prompt
        parts.push(GeminiPart::Text {
            text: user_prompt.to_string(),
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent { parts }],
        };

        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";

        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to Gemini API: {}", e);
                e
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Gemini API returned error status {}: {}",
                status, error_text
            );

            return match status.as_u16() {
                401 => Err(McpError::AuthenticationError("Invalid API key".to_string())),
                429 => Err(McpError::RateLimitError(
                    "Gemini API rate limit exceeded".to_string(),
                )),
                _ => Err(McpError::GeminiApiError {
                    code: status.as_u16() as i32,
                    message: error_text,
                }),
            };
        }

        let gemini_response: GeminiResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Gemini API response: {}", e);
            McpError::NetworkError(e)
        })?;

        if let Some(error) = gemini_response.error {
            error!(
                "Gemini API returned error: {} - {}",
                error.code, error.message
            );
            return Err(McpError::GeminiApiError {
                code: error.code,
                message: error.message,
            });
        }

        self.extract_text_from_response(gemini_response)
    }

    pub async fn generate_image(&self, input: &GenerateImageInput) -> McpResult<String> {
        self.prompt_validator.validate(&input.user_prompt)?;
        self.output_path_validator.validate(&input.output_path)?;

        if let Some(ref system_prompt) = input.system_prompt {
            self.prompt_validator.validate(system_prompt)?;
        }

        let mut parts = vec![];

        if let Some(ref system_prompt) = input.system_prompt {
            parts.push(GeminiPart::Text {
                text: system_prompt.clone(),
            });
        }

        parts.push(GeminiPart::Text {
            text: input.user_prompt.clone(),
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent { parts }],
        };

        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image-preview:generateContent";

        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!(
                    "Failed to send request to Gemini API for image generation: {}",
                    e
                );
                e
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Gemini API returned error status {}: {}",
                status, error_text
            );

            return match status.as_u16() {
                401 => Err(McpError::AuthenticationError("Invalid API key".to_string())),
                429 => Err(McpError::RateLimitError(
                    "Gemini API rate limit exceeded".to_string(),
                )),
                _ => Err(McpError::GeminiApiError {
                    code: status.as_u16() as i32,
                    message: error_text,
                }),
            };
        }

        // Parse as generic JSON first to inspect the structure
        let response_text = response.text().await.map_err(|e| {
            error!("Failed to get response text for image generation: {}", e);
            McpError::NetworkError(e)
        })?;

        let json_value: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse response as JSON: {}", e);
            error!("Response text was: {}", response_text);
            McpError::InvalidInput(format!("Failed to parse API response as JSON: {}", e))
        })?;

        // Check for API errors in the response
        if let Some(error) = json_value.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            error!("Gemini API returned error: {} - {}", code, message);
            return Err(McpError::GeminiApiError { code, message });
        }

        let base64_image_data = self.extract_image_from_json(json_value)?;

        // Decode the base64 image data
        let image_bytes = general_purpose::STANDARD
            .decode(&base64_image_data)
            .map_err(|e| {
                error!("Failed to decode base64 image data: {}", e);
                McpError::Base64Error(e)
            })?;

        // Save the image to the specified path
        fs::write(&input.output_path, &image_bytes)
            .await
            .map_err(|e| {
                error!("Failed to write image to '{}': {}", input.output_path, e);
                McpError::FileSystemError(format!("Failed to write image file: {}", e))
            })?;

        Ok(input.output_path.clone())
    }

    pub async fn edit_image(&self, input: &EditImageInput) -> McpResult<String> {
        self.image_source_validator.validate(&input.image_source)?;
        self.prompt_validator.validate(&input.user_prompt)?;
        self.output_path_validator.validate(&input.output_path)?;

        if let Some(ref system_prompt) = input.system_prompt {
            self.prompt_validator.validate(system_prompt)?;
        }

        let (mime_type, encoded_image) = self
            .image_service
            .fetch_and_encode(&input.image_source)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch and encode image '{}': {}",
                    input.image_source, e
                );
                e
            })?;

        let mut parts = vec![];

        // Add system prompt if provided
        if let Some(ref system_prompt) = input.system_prompt {
            parts.push(GeminiPart::Text {
                text: system_prompt.clone(),
            });
        }

        // Add user prompt
        parts.push(GeminiPart::Text {
            text: input.user_prompt.clone(),
        });

        // Add the image
        parts.push(GeminiPart::InlineData {
            inline_data: InlineData {
                mime_type,
                data: encoded_image,
            },
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent { parts }],
        };

        let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image-preview:generateContent";

        let response = self
            .client
            .post(url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!(
                    "Failed to send request to Gemini API for image editing: {}",
                    e
                );
                e
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Gemini API returned error status {}: {}",
                status, error_text
            );

            return match status.as_u16() {
                401 => Err(McpError::AuthenticationError("Invalid API key".to_string())),
                429 => Err(McpError::RateLimitError(
                    "Gemini API rate limit exceeded".to_string(),
                )),
                _ => Err(McpError::GeminiApiError {
                    code: status.as_u16() as i32,
                    message: error_text,
                }),
            };
        }

        // Parse as generic JSON first to inspect the structure
        let response_text = response.text().await.map_err(|e| {
            error!("Failed to get response text for image editing: {}", e);
            McpError::NetworkError(e)
        })?;

        let json_value: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse response as JSON: {}", e);
            error!("Response text was: {}", response_text);
            McpError::InvalidInput(format!("Failed to parse API response as JSON: {}", e))
        })?;

        // Check for API errors in the response
        if let Some(error) = json_value.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0) as i32;
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            error!("Gemini API returned error: {} - {}", code, message);
            return Err(McpError::GeminiApiError { code, message });
        }

        let base64_image_data = self.extract_image_from_json(json_value)?;

        // Decode the base64 image data
        let image_bytes = general_purpose::STANDARD
            .decode(&base64_image_data)
            .map_err(|e| {
                error!("Failed to decode base64 image data: {}", e);
                McpError::Base64Error(e)
            })?;

        // Save the edited image to the specified path
        fs::write(&input.output_path, &image_bytes)
            .await
            .map_err(|e| {
                error!(
                    "Failed to write edited image to '{}': {}",
                    input.output_path, e
                );
                McpError::FileSystemError(format!("Failed to write edited image file: {}", e))
            })?;

        Ok(input.output_path.clone())
    }

    fn extract_text_from_response(&self, response: GeminiResponse) -> McpResult<String> {
        if let Some(candidates) = response.candidates {
            if candidates.is_empty() {
                return Err(McpError::GeminiApiError {
                    code: 0,
                    message: "No candidates in response".to_string(),
                });
            }

            let candidate = &candidates[0];
            if candidate.content.parts.is_empty() {
                return Err(McpError::GeminiApiError {
                    code: 0,
                    message: "No parts in candidate content".to_string(),
                });
            }

            for part in &candidate.content.parts {
                if let GeminiPart::Text { text } = part
                    && !text.trim().is_empty()
                {
                    return Ok(text.clone());
                }
            }

            Err(McpError::GeminiApiError {
                code: 0,
                message: "No valid text found in response".to_string(),
            })
        } else {
            Err(McpError::GeminiApiError {
                code: 0,
                message: "No candidates in Gemini API response".to_string(),
            })
        }
    }

    fn extract_image_from_json(&self, json_value: serde_json::Value) -> McpResult<String> {
        // Try to extract image data from JSON response
        // The response structure could be: candidates[0].content.parts[0].inline_data.data
        if let Some(candidates) = json_value.get("candidates").and_then(|c| c.as_array()) {
            if candidates.is_empty() {
                return Err(McpError::GeminiApiError {
                    code: 0,
                    message: "No candidates in response".to_string(),
                });
            }

            let candidate = &candidates[0];
            if let Some(content) = candidate.get("content")
                && let Some(parts) = content.get("parts").and_then(|p| p.as_array())
            {
                if parts.is_empty() {
                    return Err(McpError::GeminiApiError {
                        code: 0,
                        message: "No parts in candidate content".to_string(),
                    });
                }

                for part in parts {
                    // Try both camelCase and snake_case since API might use either
                    if let Some(inline_data) =
                        part.get("inline_data").or_else(|| part.get("inlineData"))
                        && let Some(data) = inline_data.get("data").and_then(|d| d.as_str())
                    {
                        return Ok(data.to_string());
                    }
                }
            }
        }

        Err(McpError::GeminiApiError {
            code: 0,
            message: "No image data found in Gemini API response".to_string(),
        })
    }
}
