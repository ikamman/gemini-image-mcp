use crate::error::McpError;
use crate::gemini_client::{
    AnalyzeImageInput, ComposeImagesInput, EditImageInput, GeminiClient, GenerateImageInput,
    InpaintImageInput, RefineImageInput, StyleTransferInput,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

pub struct JsonRpcHandler {
    gemini_client: Option<GeminiClient>,
}

impl JsonRpcHandler {
    pub fn new(api_key: Option<String>) -> Self {
        let gemini_client = match api_key {
            Some(key) if !key.trim().is_empty() => match GeminiClient::new(key) {
                Ok(client) => Some(client),
                Err(e) => {
                    error!("Failed to create Gemini client: {}", e);
                    None
                }
            },
            _ => None,
        };

        Self { gemini_client }
    }

    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                }),
            },
        }
    }

    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let result = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "gemini-image-mcp",
                "version": "1.1.0"
            }
        });
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        }
    }

    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let tools = json!([
            {
                "name": "analyze_image",
                "description": "Analyze an image using Google's Gemini API. Supports both URLs (http/https) and local file paths.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(AnalyzeImageInput)).unwrap()
            },
            {
                "name": "generate_image",
                "description": "Generate an image using Google's Gemini API with optional system prompt and required user prompt.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(GenerateImageInput)).unwrap()
            },
            {
                "name": "edit_image",
                "description": "Edit an existing image using Google's Gemini API by providing both an input image and a text prompt describing the desired changes.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(EditImageInput)).unwrap()
            },
            {
                "name": "inpaint_image",
                "description": "Inpaint/modify specific regions of an image using semantic masking. Supports focusing on specific elements or regions.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(InpaintImageInput)).unwrap()
            },
            {
                "name": "style_transfer",
                "description": "Transfer the artistic style from one image to another using Google's Gemini API.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(StyleTransferInput)).unwrap()
            },
            {
                "name": "compose_images",
                "description": "Compose multiple images into a single new image using Google's Gemini API.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(ComposeImagesInput)).unwrap()
            },
            {
                "name": "refine_image",
                "description": "Iteratively refine an image with conversation history for progressive improvement using Google's Gemini API.",
                "inputSchema": serde_json::to_value(schemars::schema_for!(RefineImageInput)).unwrap()
            }
        ]);
        let result = json!({ "tools": tools });
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(result),
            error: None,
        }
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if let Some(params) = request.params
            && let Ok(tool_call) = serde_json::from_value::<Value>(params)
        {
            if let Some(name) = tool_call.get("name").and_then(|v| v.as_str()) {
                if name == "analyze_image" {
                    return self.handle_analyze_image(request.id, tool_call).await;
                } else if name == "generate_image" {
                    return self.handle_generate_image(request.id, tool_call).await;
                } else if name == "edit_image" {
                    return self.handle_edit_image(request.id, tool_call).await;
                } else if name == "inpaint_image" {
                    return self.handle_inpaint_image(request.id, tool_call).await;
                } else if name == "style_transfer" {
                    return self.handle_style_transfer(request.id, tool_call).await;
                } else if name == "compose_images" {
                    return self.handle_compose_images(request.id, tool_call).await;
                } else if name == "refine_image" {
                    return self.handle_refine_image(request.id, tool_call).await;
                } else {
                    return JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -1,
                            message: format!("Unknown tool: {}", name),
                        }),
                    };
                }
            } else {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -1,
                        message: "Missing tool name".to_string(),
                    }),
                };
            }
        }

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -1,
                message: "Invalid params".to_string(),
            }),
        }
    }

    async fn handle_analyze_image(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        // Check if client is available
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<AnalyzeImageInput>(arguments.clone()) {
                Ok(input) => match client.analyze_image(&input).await {
                    Ok(analysis) => {
                        info!("Successfully analyzed image: {}", input.image_source);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": analysis
                                }
                            ]
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!("Failed to analyze image '{}': {}", input.image_source, e);
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for analyze_image: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }

    async fn handle_generate_image(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<GenerateImageInput>(arguments.clone()) {
                Ok(input) => match client.generate_image(&input).await {
                    Ok(file_path) => {
                        info!("Successfully generated and saved image to: {}", file_path);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Image successfully generated and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to generate image with prompt '{}': {}",
                            input.user_prompt, e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for generate_image: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }

    async fn handle_edit_image(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<EditImageInput>(arguments.clone()) {
                Ok(input) => match client.edit_image(&input).await {
                    Ok(file_path) => {
                        info!("Successfully edited and saved image to: {}", file_path);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Image successfully edited and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to edit image '{}' with prompt '{}': {}",
                            input.image_source, input.user_prompt, e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for edit_image: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }
}

fn convert_mcp_error_to_jsonrpc(error: McpError) -> JsonRpcError {
    match error {
        McpError::InvalidInput(msg) => JsonRpcError {
            code: -32602,
            message: format!("Invalid params: {}", msg),
        },
        McpError::AuthenticationError(msg) => JsonRpcError {
            code: -32001,
            message: format!("Authentication error: {}", msg),
        },
        McpError::ConfigurationError(msg) => JsonRpcError {
            code: -32001,
            message: format!("Configuration error: {}", msg),
        },
        McpError::RateLimitError(msg) => JsonRpcError {
            code: -32002,
            message: format!("Rate limit exceeded: {}", msg),
        },
        McpError::NetworkError(_) => JsonRpcError {
            code: -32003,
            message: "Network error occurred".to_string(),
        },
        McpError::FileSystemError(msg) => JsonRpcError {
            code: -32004,
            message: format!("File system error: {}", msg),
        },
        McpError::GeminiApiError { code, message } => JsonRpcError {
            code: -32005,
            message: format!("Gemini API error ({}): {}", code, message),
        },
        McpError::Timeout(msg) => JsonRpcError {
            code: -32006,
            message: format!("Timeout: {}", msg),
        },
        _ => JsonRpcError {
            code: -1,
            message: format!("Internal error: {}", error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_handle_initialize() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "initialize".to_string(),
            params: None,
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "gemini-image-mcp");
        assert_eq!(result["serverInfo"]["version"], "1.1.0");
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 7);

        assert_eq!(tools[0]["name"], "analyze_image");
        assert!(
            tools[0]["description"]
                .as_str()
                .unwrap()
                .contains("Gemini API")
        );
        assert!(tools[0]["inputSchema"]["properties"]["image_source"].is_object());
        assert!(tools[0]["inputSchema"]["properties"]["system_prompt"].is_object());
        assert!(tools[0]["inputSchema"]["properties"]["user_prompt"].is_object());

        assert_eq!(tools[1]["name"], "generate_image");
        assert!(
            tools[1]["description"]
                .as_str()
                .unwrap()
                .contains("Generate an image")
        );
        assert!(tools[1]["inputSchema"]["properties"]["user_prompt"].is_object());
        assert!(tools[1]["inputSchema"]["properties"]["system_prompt"].is_object());

        assert_eq!(tools[2]["name"], "edit_image");
        assert!(
            tools[2]["description"]
                .as_str()
                .unwrap()
                .contains("Edit an existing image")
        );
        assert!(tools[2]["inputSchema"]["properties"]["image_source"].is_object());
        assert!(tools[2]["inputSchema"]["properties"]["system_prompt"].is_object());
        assert!(tools[2]["inputSchema"]["properties"]["user_prompt"].is_object());
        assert!(tools[2]["inputSchema"]["properties"]["output_path"].is_object());

        // Test new tools
        assert_eq!(tools[3]["name"], "inpaint_image");
        assert!(
            tools[3]["description"]
                .as_str()
                .unwrap()
                .contains("Inpaint/modify specific regions")
        );

        assert_eq!(tools[4]["name"], "style_transfer");
        assert!(
            tools[4]["description"]
                .as_str()
                .unwrap()
                .contains("Transfer the artistic style")
        );

        assert_eq!(tools[5]["name"], "compose_images");
        assert!(
            tools[5]["description"]
                .as_str()
                .unwrap()
                .contains("Compose multiple images")
        );

        assert_eq!(tools[6]["name"], "refine_image");
        assert!(
            tools[6]["description"]
                .as_str()
                .unwrap()
                .contains("Iteratively refine an image")
        );
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "unknown/method".to_string(),
            params: None,
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    #[tokio::test]
    async fn test_handle_generate_image_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "generate_image",
                "arguments": {
                    "user_prompt": "A beautiful sunset over mountains",
                    "output_path": "./test/test_output.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_generate_image_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "generate_image",
                "arguments": {
                    // missing required user_prompt and output_path
                    "system_prompt": "Create something artistic"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_generate_image_invalid_output_path() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "generate_image",
                "arguments": {
                    "user_prompt": "A beautiful sunset",
                    "output_path": "/nonexistent/directory/image.jpg"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        // Should be either config error (no API key) or validation error for path
        assert!(error.code == -32001 || error.code == -32602);
    }

    #[test]
    fn test_convert_mcp_error_to_jsonrpc() {
        let error = McpError::InvalidInput("test".to_string());
        let jsonrpc_error = convert_mcp_error_to_jsonrpc(error);
        assert_eq!(jsonrpc_error.code, -32602);
        assert!(jsonrpc_error.message.contains("Invalid params"));

        let error = McpError::AuthenticationError("auth failed".to_string());
        let jsonrpc_error = convert_mcp_error_to_jsonrpc(error);
        assert_eq!(jsonrpc_error.code, -32001);
        assert!(jsonrpc_error.message.contains("Authentication error"));
    }

    #[test]
    fn test_json_rpc_response_error_constructor() {
        let response = JsonRpcResponse::error(Some(json!(1)), -32700, "Parse error".to_string());

        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(json!(1)));
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32700);
        assert_eq!(error.message, "Parse error");
    }

    #[tokio::test]
    async fn test_handle_edit_image_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "edit_image",
                "arguments": {
                    "image_source": "./test/cat_image.jpg",
                    "user_prompt": "Add a hat to the cat",
                    "output_path": "./test/test_edited.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_edit_image_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "edit_image",
                "arguments": {
                    // missing required fields
                    "user_prompt": "Add a hat to the cat"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_edit_image_unknown_tool() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "unknown_tool",
                "arguments": {
                    "image_source": "./test/cat_image.jpg",
                    "user_prompt": "Add a hat",
                    "output_path": "./test/edited.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -1);
        assert!(error.message.contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_handle_inpaint_image_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "inpaint_image",
                "arguments": {
                    "image_source": "./test/cat_image.jpg",
                    "user_prompt": "Replace with a golden retriever",
                    "mask_description": "the cat",
                    "output_path": "./test/inpainted.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_inpaint_image_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "inpaint_image",
                "arguments": {
                    // missing required fields
                    "user_prompt": "Replace with a golden retriever"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_style_transfer_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "style_transfer",
                "arguments": {
                    "source_image": "./test/cat_image.jpg",
                    "style_image": "./test/painting.jpg",
                    "output_path": "./test/stylized.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_style_transfer_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "style_transfer",
                "arguments": {
                    // missing required fields
                    "source_image": "./test/cat_image.jpg"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_compose_images_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "compose_images",
                "arguments": {
                    "primary_image": "./test/background.jpg",
                    "secondary_images": ["./test/cat_image.jpg", "./test/dog.jpg"],
                    "user_prompt": "Place the cat and dog in the garden scene",
                    "output_path": "./test/composed.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_compose_images_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "compose_images",
                "arguments": {
                    // missing required fields
                    "primary_image": "./test/background.jpg"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_compose_images_empty_secondary_images() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "compose_images",
                "arguments": {
                    "primary_image": "./test/background.jpg",
                    "secondary_images": [],
                    "user_prompt": "Show just the background",
                    "output_path": "./test/composed.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001); // Config error (no API key)
    }

    #[tokio::test]
    async fn test_handle_refine_image_no_api_key() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "refine_image",
                "arguments": {
                    "image_source": "./test/draft.jpg",
                    "user_prompt": "Make the colors more vibrant",
                    "conversation_history": ["Previous: adjusted composition"],
                    "output_path": "./test/refined.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001);
        assert!(error.message.contains("Configuration error"));
    }

    #[tokio::test]
    async fn test_handle_refine_image_invalid_args() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "refine_image",
                "arguments": {
                    // missing required fields
                    "user_prompt": "Make the colors more vibrant"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert!(error.code == -32001 || error.code == -32602); // Either config error or invalid params
    }

    #[tokio::test]
    async fn test_handle_refine_image_no_conversation_history() {
        let handler = JsonRpcHandler::new(None);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "refine_image",
                "arguments": {
                    "image_source": "./test/draft.jpg",
                    "user_prompt": "Make the colors more vibrant",
                    "output_path": "./test/refined.png"
                    // conversation_history is optional
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32001); // Config error (no API key)
    }

    #[tokio::test]
    async fn test_new_tools_missing_arguments() {
        let handler = JsonRpcHandler::new(None);

        let tool_names = [
            "inpaint_image",
            "style_transfer",
            "compose_images",
            "refine_image",
        ];

        for tool_name in tool_names {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": tool_name
                    // missing arguments field
                })),
            };

            let response = handler.handle_request(request).await;

            assert_eq!(response.jsonrpc, "2.0");
            assert!(response.result.is_none());
            assert!(response.error.is_some());

            let error = response.error.unwrap();
            // With no API key, we get a config error before argument validation
            assert_eq!(error.code, -32001);
            assert!(error.message.contains("Configuration error"));
        }
    }

    #[tokio::test]
    async fn test_new_tools_invalid_image_paths() {
        let handler = JsonRpcHandler::new(Some("test-api-key".to_string()));

        // Test inpaint_image with invalid image path
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "inpaint_image",
                "arguments": {
                    "image_source": "invalid_file.txt",
                    "user_prompt": "Test prompt",
                    "output_path": "./test/output.png"
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        // Since we have an API key but invalid input, this should be a validation error
        // The actual error will depend on which validation fails first
        assert!(error.code == -32602 || error.code == -32004); // Invalid params or file system error
        assert!(
            error.message.contains("Invalid arguments")
                || error.message.contains("Unsupported file extension")
        );
    }

    #[tokio::test]
    async fn test_style_transfer_invalid_output_path() {
        let handler = JsonRpcHandler::new(Some("test-api-key".to_string()));

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(1))),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "style_transfer",
                "arguments": {
                    "source_image": "./test/valid.jpg",
                    "style_image": "./test/style.jpg",
                    "output_path": "/nonexistent/directory/output.txt" // invalid extension
                }
            })),
        };

        let response = handler.handle_request(request).await;

        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        // Since we have an API key but invalid input, this should be a validation error
        assert_eq!(error.code, -32602); // Invalid params 
        // The error message should indicate the unsupported file extension
        assert!(error.message.contains("Unsupported output file extension"));
    }
}

impl JsonRpcHandler {
    async fn handle_inpaint_image(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<InpaintImageInput>(arguments.clone()) {
                Ok(input) => match client.inpaint_image(&input).await {
                    Ok(file_path) => {
                        info!("Successfully inpainted and saved image to: {}", file_path);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Image successfully inpainted and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to inpaint image '{}' with prompt '{}': {}",
                            input.image_source, input.user_prompt, e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for inpaint_image: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }

    async fn handle_style_transfer(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<StyleTransferInput>(arguments.clone()) {
                Ok(input) => match client.style_transfer(&input).await {
                    Ok(file_path) => {
                        info!(
                            "Successfully applied style transfer and saved image to: {}",
                            file_path
                        );
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Style transfer completed and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to apply style transfer from '{}' to '{}': {}",
                            input.style_image, input.source_image, e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for style_transfer: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }

    async fn handle_compose_images(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<ComposeImagesInput>(arguments.clone()) {
                Ok(input) => match client.compose_images(&input).await {
                    Ok(file_path) => {
                        info!("Successfully composed images and saved to: {}", file_path);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Images successfully composed and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to compose images with primary '{}' and {} secondary images: {}",
                            input.primary_image,
                            input.secondary_images.len(),
                            e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for compose_images: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }

    async fn handle_refine_image(&self, id: Option<Value>, tool_call: Value) -> JsonRpcResponse {
        let client = match &self.gemini_client {
            Some(client) => client,
            None => {
                return JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(convert_mcp_error_to_jsonrpc(McpError::ConfigurationError(
                        "GEMINI_API_KEY environment variable not set".to_string(),
                    ))),
                };
            }
        };

        if let Some(arguments) = tool_call.get("arguments") {
            match serde_json::from_value::<RefineImageInput>(arguments.clone()) {
                Ok(input) => match client.refine_image(&input).await {
                    Ok(file_path) => {
                        info!("Successfully refined image and saved to: {}", file_path);
                        let result = json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Image successfully refined and saved to: {}", file_path)
                                }
                            ],
                            "file_path": file_path
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to refine image '{}' with prompt '{}': {}",
                            input.image_source, input.user_prompt, e
                        );
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: None,
                            error: Some(convert_mcp_error_to_jsonrpc(e)),
                        }
                    }
                },
                Err(e) => {
                    error!("Invalid arguments for refine_image: {}", e);
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                            format!("Invalid arguments: {}", e),
                        ))),
                    }
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(convert_mcp_error_to_jsonrpc(McpError::InvalidInput(
                    "Missing arguments".to_string(),
                ))),
            }
        }
    }
}
