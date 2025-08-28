use anyhow::Result;
use clap::Parser;
use std::io::{self, BufRead, Write};
use tracing::{error, info, warn};

mod error;
mod validation;
mod image_service;
mod jsonrpc;
mod gemini_client;

use jsonrpc::{JsonRpcHandler, JsonRpcRequest, JsonRpcResponse};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Override GEMINI_API_KEY environment variable with this API key
    #[arg(long, value_name = "KEY")]
    gemini_api_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();
    
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    info!("Starting Gemini Image Analysis MCP Server");
    
    // Determine API key: command line takes precedence over environment variable
    let api_key = if let Some(key) = args.gemini_api_key {
        if key.trim().is_empty() {
            warn!("Command line API key is empty - image analysis will fail");
            None
        } else {
            info!("Using API key provided via command line");
            Some(key)
        }
    } else {
        let env_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
        if env_key.trim().is_empty() {
            warn!("GEMINI_API_KEY environment variable not set - image analysis will fail");
            None
        } else {
            info!("Using API key from GEMINI_API_KEY environment variable");
            Some(env_key)
        }
    };
    
    let handler = JsonRpcHandler::new(api_key);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                
                match serde_json::from_str::<JsonRpcRequest>(&line) {
                    Ok(request) => {
                        let response = handler.handle_request(request).await;
                        match serde_json::to_string(&response) {
                            Ok(response_json) => {
                                if let Err(e) = writeln!(stdout, "{}", response_json) {
                                    error!("Failed to write response: {}", e);
                                    break;
                                }
                                if let Err(e) = stdout.flush() {
                                    error!("Failed to flush stdout: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to serialize response: {}", e);
                                let fallback_error = JsonRpcResponse::error(
                                    response.id.clone(),
                                    -32603,
                                    "Internal error - serialization failed".to_string()
                                );
                                if let Ok(fallback_json) = serde_json::to_string(&fallback_error) {
                                    let _ = writeln!(stdout, "{}", fallback_json);
                                    let _ = stdout.flush();
                                }
                            }
                        }
                    }
                    Err(parse_error) => {
                        error!("Failed to parse JSON-RPC request: {}", parse_error);
                        let error_response = JsonRpcResponse::error(
                            None,
                            -32700,
                            format!("Parse error: {}", parse_error)
                        );
                        if let Ok(response_json) = serde_json::to_string(&error_response) {
                            let _ = writeln!(stdout, "{}", response_json);
                            let _ = stdout.flush();
                        }
                    }
                }
            }
            Err(io_error) => {
                error!("Failed to read from stdin: {}", io_error);
                break;
            }
        }
    }
    
    info!("Shutting down Gemini Image Analysis MCP Server");
    Ok(())
}