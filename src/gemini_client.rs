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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InpaintImageInput {
    /// Input image source: can be a URL (http/https) or a local file path
    pub image_source: String,
    /// Optional system prompt to guide the inpainting
    pub system_prompt: Option<String>,
    /// User prompt describing what to add/replace in specific regions
    pub user_prompt: String,
    /// Optional mask description for semantic masking (e.g., "the dog", "background", "person's face")
    pub mask_description: Option<String>,
    /// Output file path where the inpainted image will be saved
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct StyleTransferInput {
    /// Source image to apply style to: can be a URL (http/https) or a local file path
    pub source_image: String,
    /// Style reference image: can be a URL (http/https) or a local file path
    pub style_image: String,
    /// Optional system prompt to guide the style transfer
    pub system_prompt: Option<String>,
    /// User prompt describing the desired style transfer (defaults to "Apply the style of the second image to the first image")
    pub user_prompt: Option<String>,
    /// Output file path where the style-transferred image will be saved
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ComposeImagesInput {
    /// Primary image source: can be a URL (http/https) or a local file path
    pub primary_image: String,
    /// Secondary images to compose with the primary image
    pub secondary_images: Vec<String>,
    /// Optional system prompt to guide the composition
    pub system_prompt: Option<String>,
    /// User prompt describing how to compose the images
    pub user_prompt: String,
    /// Output file path where the composed image will be saved
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RefineImageInput {
    /// Input image source: can be a URL (http/https) or a local file path
    pub image_source: String,
    /// Optional system prompt to guide the refinement
    pub system_prompt: Option<String>,
    /// User prompt describing the refinements to make
    pub user_prompt: String,
    /// Previous conversation context for iterative refinement
    pub conversation_history: Option<Vec<String>>,
    /// Output file path where the refined image will be saved
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

    pub async fn inpaint_image(&self, input: &InpaintImageInput) -> McpResult<String> {
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

        // Construct user prompt with mask description if provided
        let full_prompt = if let Some(ref mask_desc) = input.mask_description {
            format!(
                "Focus on the region containing '{}'. {}",
                mask_desc, input.user_prompt
            )
        } else {
            input.user_prompt.clone()
        };

        parts.push(GeminiPart::Text { text: full_prompt });

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

        self.generate_image_from_request(request, &input.output_path)
            .await
    }

    pub async fn style_transfer(&self, input: &StyleTransferInput) -> McpResult<String> {
        self.image_source_validator.validate(&input.source_image)?;
        self.image_source_validator.validate(&input.style_image)?;
        self.output_path_validator.validate(&input.output_path)?;

        if let Some(ref system_prompt) = input.system_prompt {
            self.prompt_validator.validate(system_prompt)?;
        }

        let (source_mime, source_encoded) = self
            .image_service
            .fetch_and_encode(&input.source_image)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch and encode source image '{}': {}",
                    input.source_image, e
                );
                e
            })?;

        let (style_mime, style_encoded) = self
            .image_service
            .fetch_and_encode(&input.style_image)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch and encode style image '{}': {}",
                    input.style_image, e
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

        // Add source image
        parts.push(GeminiPart::InlineData {
            inline_data: InlineData {
                mime_type: source_mime,
                data: source_encoded,
            },
        });

        // Add style image
        parts.push(GeminiPart::InlineData {
            inline_data: InlineData {
                mime_type: style_mime,
                data: style_encoded,
            },
        });

        // Add user prompt or default
        let prompt = input
            .user_prompt
            .as_deref()
            .unwrap_or("Apply the style of the second image to the first image");
        parts.push(GeminiPart::Text {
            text: prompt.to_string(),
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent { parts }],
        };

        self.generate_image_from_request(request, &input.output_path)
            .await
    }

    pub async fn compose_images(&self, input: &ComposeImagesInput) -> McpResult<String> {
        self.image_source_validator.validate(&input.primary_image)?;
        for secondary_image in &input.secondary_images {
            self.image_source_validator.validate(secondary_image)?;
        }
        self.prompt_validator.validate(&input.user_prompt)?;
        self.output_path_validator.validate(&input.output_path)?;

        if let Some(ref system_prompt) = input.system_prompt {
            self.prompt_validator.validate(system_prompt)?;
        }

        let mut parts = vec![];

        // Add system prompt if provided
        if let Some(ref system_prompt) = input.system_prompt {
            parts.push(GeminiPart::Text {
                text: system_prompt.clone(),
            });
        }

        // Add primary image
        let (primary_mime, primary_encoded) = self
            .image_service
            .fetch_and_encode(&input.primary_image)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch and encode primary image '{}': {}",
                    input.primary_image, e
                );
                e
            })?;

        parts.push(GeminiPart::InlineData {
            inline_data: InlineData {
                mime_type: primary_mime,
                data: primary_encoded,
            },
        });

        // Add secondary images
        for secondary_image in &input.secondary_images {
            let (secondary_mime, secondary_encoded) = self
                .image_service
                .fetch_and_encode(secondary_image)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to fetch and encode secondary image '{}': {}",
                        secondary_image, e
                    );
                    e
                })?;

            parts.push(GeminiPart::InlineData {
                inline_data: InlineData {
                    mime_type: secondary_mime,
                    data: secondary_encoded,
                },
            });
        }

        // Add user prompt
        parts.push(GeminiPart::Text {
            text: input.user_prompt.clone(),
        });

        let request = GeminiRequest {
            contents: vec![GeminiContent { parts }],
        };

        self.generate_image_from_request(request, &input.output_path)
            .await
    }

    pub async fn refine_image(&self, input: &RefineImageInput) -> McpResult<String> {
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

        // Add conversation history if provided
        if let Some(ref history) = input.conversation_history {
            for (i, context) in history.iter().enumerate() {
                parts.push(GeminiPart::Text {
                    text: format!("Previous iteration {}: {}", i + 1, context),
                });
            }
        }

        // Add current refinement prompt
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

        self.generate_image_from_request(request, &input.output_path)
            .await
    }

    async fn generate_image_from_request(
        &self,
        request: GeminiRequest,
        output_path: &str,
    ) -> McpResult<String> {
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

        let response_text = response.text().await.map_err(|e| {
            error!("Failed to get response text: {}", e);
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
        fs::write(output_path, &image_bytes).await.map_err(|e| {
            error!("Failed to write image to '{}': {}", output_path, e);
            McpError::FileSystemError(format!("Failed to write image file: {}", e))
        })?;

        Ok(output_path.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_inpaint_image_input_serialization() {
        let input = InpaintImageInput {
            image_source: "./test/image.jpg".to_string(),
            system_prompt: Some("System guidance".to_string()),
            user_prompt: "Replace with a dog".to_string(),
            mask_description: Some("the cat".to_string()),
            output_path: "./test/output.png".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("image_source"));
        assert!(json.contains("system_prompt"));
        assert!(json.contains("user_prompt"));
        assert!(json.contains("mask_description"));
        assert!(json.contains("output_path"));

        // Test deserialization
        let deserialized: InpaintImageInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.image_source, "./test/image.jpg");
        assert_eq!(deserialized.user_prompt, "Replace with a dog");
        assert_eq!(deserialized.mask_description, Some("the cat".to_string()));
    }

    #[test]
    fn test_style_transfer_input_serialization() {
        let input = StyleTransferInput {
            source_image: "./test/source.jpg".to_string(),
            style_image: "./test/style.jpg".to_string(),
            system_prompt: None,
            user_prompt: Some("Transfer artistic style".to_string()),
            output_path: "./test/output.png".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("source_image"));
        assert!(json.contains("style_image"));
        assert!(json.contains("output_path"));

        // Test deserialization
        let deserialized: StyleTransferInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source_image, "./test/source.jpg");
        assert_eq!(deserialized.style_image, "./test/style.jpg");
        assert_eq!(
            deserialized.user_prompt,
            Some("Transfer artistic style".to_string())
        );
    }

    #[test]
    fn test_compose_images_input_serialization() {
        let input = ComposeImagesInput {
            primary_image: "./test/primary.jpg".to_string(),
            secondary_images: vec![
                "./test/secondary1.jpg".to_string(),
                "./test/secondary2.jpg".to_string(),
            ],
            system_prompt: Some("Compose creatively".to_string()),
            user_prompt: "Create a collage".to_string(),
            output_path: "./test/composed.png".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("primary_image"));
        assert!(json.contains("secondary_images"));
        assert!(json.contains("user_prompt"));

        // Test deserialization
        let deserialized: ComposeImagesInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.primary_image, "./test/primary.jpg");
        assert_eq!(deserialized.secondary_images.len(), 2);
        assert_eq!(deserialized.secondary_images[0], "./test/secondary1.jpg");
        assert_eq!(deserialized.user_prompt, "Create a collage");
    }

    #[test]
    fn test_refine_image_input_serialization() {
        let input = RefineImageInput {
            image_source: "./test/draft.jpg".to_string(),
            system_prompt: None,
            user_prompt: "Make it better".to_string(),
            conversation_history: Some(vec![
                "First iteration: adjusted colors".to_string(),
                "Second iteration: improved lighting".to_string(),
            ]),
            output_path: "./test/refined.png".to_string(),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("image_source"));
        assert!(json.contains("conversation_history"));
        assert!(json.contains("user_prompt"));

        // Test deserialization
        let deserialized: RefineImageInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.image_source, "./test/draft.jpg");
        assert_eq!(deserialized.user_prompt, "Make it better");
        assert_eq!(deserialized.conversation_history.as_ref().unwrap().len(), 2);
        assert_eq!(
            deserialized.conversation_history.as_ref().unwrap()[0],
            "First iteration: adjusted colors"
        );
    }

    #[test]
    fn test_inpaint_image_input_without_optional_fields() {
        let json = r#"{
            "image_source": "./test/image.jpg",
            "user_prompt": "Replace background",
            "output_path": "./test/output.png"
        }"#;

        let input: InpaintImageInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.image_source, "./test/image.jpg");
        assert_eq!(input.user_prompt, "Replace background");
        assert!(input.system_prompt.is_none());
        assert!(input.mask_description.is_none());
        assert_eq!(input.output_path, "./test/output.png");
    }

    #[test]
    fn test_style_transfer_input_without_optional_fields() {
        let json = r#"{
            "source_image": "./test/source.jpg",
            "style_image": "./test/style.jpg",
            "output_path": "./test/output.png"
        }"#;

        let input: StyleTransferInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.source_image, "./test/source.jpg");
        assert_eq!(input.style_image, "./test/style.jpg");
        assert!(input.system_prompt.is_none());
        assert!(input.user_prompt.is_none());
        assert_eq!(input.output_path, "./test/output.png");
    }

    #[test]
    fn test_compose_images_input_with_empty_secondary_images() {
        let json = r#"{
            "primary_image": "./test/primary.jpg",
            "secondary_images": [],
            "user_prompt": "Show only primary",
            "output_path": "./test/output.png"
        }"#;

        let input: ComposeImagesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.primary_image, "./test/primary.jpg");
        assert!(input.secondary_images.is_empty());
        assert_eq!(input.user_prompt, "Show only primary");
    }

    #[test]
    fn test_refine_image_input_without_conversation_history() {
        let json = r#"{
            "image_source": "./test/draft.jpg",
            "user_prompt": "Enhance colors",
            "output_path": "./test/refined.png"
        }"#;

        let input: RefineImageInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.image_source, "./test/draft.jpg");
        assert_eq!(input.user_prompt, "Enhance colors");
        assert!(input.conversation_history.is_none());
        assert_eq!(input.output_path, "./test/refined.png");
    }

    #[test]
    fn test_input_structs_with_json_schema() {
        use schemars::schema_for;

        // Test that all input structs can generate JSON schemas
        let _inpaint_schema = schema_for!(InpaintImageInput);
        let _style_transfer_schema = schema_for!(StyleTransferInput);
        let _compose_schema = schema_for!(ComposeImagesInput);
        let _refine_schema = schema_for!(RefineImageInput);

        // These should not panic, indicating schemas are properly generated
    }

    #[test]
    fn test_input_validation_edge_cases() {
        // Test input with very long paths
        let long_path = "x".repeat(3000);
        let input = InpaintImageInput {
            image_source: long_path.clone(),
            system_prompt: None,
            user_prompt: "Test".to_string(),
            mask_description: None,
            output_path: "./test/output.png".to_string(),
        };

        // Should serialize properly even with long paths (validation happens elsewhere)
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.len() > 3000);
    }
}
