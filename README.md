# Gemini Image MCP Server

<div align="center">

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Google Gemini](https://img.shields.io/badge/Google%20Gemini-8E75B2?style=for-the-badge&logo=googlegemini&logoColor=white)
[![MCP](https://img.shields.io/badge/Model%20Context%20Protocol-000000?style=for-the-badge)](https://modelcontextprotocol.io/)

**A powerful MCP server for image analysis, generation, and editing using Google's Gemini API**

[Installation](#installation) • [Usage](#usage) • [API Reference](#api-reference) • [Examples](#examples)

</div>

## ✨ Features

- 🖼️ **Image Analysis** - Analyze images from URLs or local files using Gemini 2.5 Flash
- 🎨 **Image Generation** - Generate high-quality images from text prompts
- ✏️ **Image Editing** - Edit existing images with natural language instructions
- 🔍 **Custom Prompts** - Use system and user prompts for specific analysis needs
- 🚀 **High Performance** - Built with Rust for speed and reliability
- 🛡️ **Robust Error Handling** - Comprehensive error handling and validation
- 📡 **MCP Protocol** - Seamless integration with MCP-compatible clients
- 🌐 **Multi-Format Support** - JPEG, PNG, GIF, WebP, and more

## 🚀 Quick Start

### Using npx (Recommended)

```bash
npx @ikamman/gemini-image-mcp --gemini-api-key "your-api-key"
```

### Global Installation

```bash
npm install -g @ikamman/gemini-image-mcp
gemini-image-mcp --help
```

## 📦 Installation

### Prerequisites

- **Gemini API Key**: [Get one here](https://aistudio.google.com/app/apikey)
- **Node.js**: 14+ (for npm installation)
- **Rust**: 1.70+ (for building from source)

### Option 1: Install via npm

```bash
npm install -g @ikamman/gemini-image-mcp
```

### Option 2: Build from Source

```bash
git clone https://github.com/your-username/gemini-image-mcp.git
cd gemini-image-mcp
cargo build --release
```

## 🔧 Configuration

Set your Gemini API key using one of these methods:

### Environment Variable
```bash
export GEMINI_API_KEY="your-api-key-here"
```

### Command Line Argument
```bash
gemini-image-mcp --gemini-api-key "your-api-key"
```

### Using .env File
```bash
echo "GEMINI_API_KEY=your-api-key-here" > .env
```

## 📖 Usage

### As MCP Server

The server communicates via JSON-RPC over stdio:

```bash
gemini-image-mcp
```

### Integration with Claude Desktop

#### Using npx (No Installation Required)

Add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gemini-image-mcp": {
      "command": "npx",
      "args": ["@ikamman/gemini-image-mcp"],
      "env": {
        "GEMINI_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

#### Using Global Installation

First install globally:
```bash
npm install -g @ikamman/gemini-image-mcp
```

Then add to your `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gemini-image-mcp": {
      "command": "gemini-image-mcp",
      "env": {
        "GEMINI_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

#### Alternative: Using Full Path

For more reliability, you can use the full npx path:

```json
{
  "mcpServers": {
    "gemini-image-mcp": {
      "command": "/usr/local/bin/npx",
      "args": ["@ikamman/gemini-image-mcp"],
      "env": {
        "GEMINI_API_KEY": "your-api-key-here"
      }
    }
  }
}
```

### Manual Testing

```bash
# Test image analysis
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"analyze_image","arguments":{"image_source":"https://example.com/image.jpg","user_prompt":"What do you see?"}}}' | gemini-image-mcp

# Test image generation
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"generate_image","arguments":{"user_prompt":"A sunset over mountains","output_path":"./sunset.png"}}}' | gemini-image-mcp
```

## 🔗 API Reference

### 🔍 `analyze_image`

Analyzes images using Google's Gemini API.

**Parameters:**
- `image_source` (required) - Image URL or local file path
- `system_prompt` (optional) - System instructions for analysis
- `user_prompt` (optional) - Analysis question (default: "Caption this image.")

**Example:**
```json
{
  "image_source": "https://example.com/photo.jpg",
  "system_prompt": "You are a professional photographer.",
  "user_prompt": "Analyze the composition and lighting of this image."
}
```

### 🎨 `generate_image`

Generates images from text descriptions.

**Parameters:**
- `user_prompt` (required) - Description of the image to generate
- `output_path` (required) - Path where the image should be saved
- `system_prompt` (optional) - Additional generation guidelines

**Example:**
```json
{
  "user_prompt": "A cyberpunk cityscape at night with neon lights",
  "output_path": "./generated_city.png",
  "system_prompt": "Create a high-quality, detailed image."
}
```

### ✏️ `edit_image`

Edits existing images using natural language instructions.

**Parameters:**
- `image_source` (required) - Source image URL or file path
- `user_prompt` (required) - Editing instructions
- `output_path` (required) - Path for the edited image
- `system_prompt` (optional) - Additional editing guidelines

**Example:**
```json
{
  "image_source": "./my_photo.jpg",
  "user_prompt": "Add a vintage filter and increase the warmth",
  "output_path": "./edited_photo.jpg"
}
```

## 💡 Examples

### Image Analysis Examples

```bash
# Analyze a webpage screenshot
gemini-image-mcp analyze "https://example.com/screenshot.png" "What UI elements do you see?"

# Analyze a local photo
gemini-image-mcp analyze "./vacation.jpg" "Describe the location and activities"

# Technical analysis
gemini-image-mcp analyze "./chart.png" "Extract the key data points and trends"
```

### Image Generation Examples

```bash
# Generate artwork
gemini-image-mcp generate "Abstract watercolor painting of a forest" "./forest.png"

# Generate technical diagrams
gemini-image-mcp generate "Network architecture diagram showing microservices" "./diagram.png"

# Generate marketing assets
gemini-image-mcp generate "Modern logo for a tech startup, minimalist design" "./logo.png"
```

### Image Editing Examples

```bash
# Basic editing
gemini-image-mcp edit "./portrait.jpg" "Remove the background" "./portrait_nobg.png"

# Style changes
gemini-image-mcp edit "./photo.jpg" "Convert to black and white with high contrast" "./photo_bw.jpg"

# Object manipulation
gemini-image-mcp edit "./room.jpg" "Add a plant in the corner" "./room_with_plant.jpg"
```

## 🏗️ Supported Image Formats

| Format | Extensions | Analysis | Generation | Editing |
|--------|------------|----------|------------|---------|
| JPEG   | `.jpg`, `.jpeg` | ✅ | ✅ | ✅ |
| PNG    | `.png` | ✅ | ✅ | ✅ |
| GIF    | `.gif` | ✅ | ❌ | ✅ |
| WebP   | `.webp` | ✅ | ❌ | ✅ |

## ⚡ Performance & Limits

- **Image Size**: Up to 20MB per image
- **Concurrent Requests**: Handled via async Rust runtime
- **Rate Limits**: Follows Gemini API rate limits
- **Response Time**: Typically 2-10 seconds depending on image size and complexity

## 🛠️ Development

### Building from Source

```bash
git clone https://github.com/your-username/gemini-image-mcp.git
cd gemini-image-mcp
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Testing with Sample Images

```bash
cargo test -- --nocapture
```

### Project Structure

```
gemini-image-mcp/
├── src/
│   ├── main.rs              # Application entry point
│   ├── jsonrpc.rs          # JSON-RPC handler
│   ├── gemini_client.rs    # Gemini API client
│   ├── image_service.rs    # Image processing service
│   ├── validation.rs       # Input validation
│   └── error.rs            # Error handling
├── test/                   # Sample images for testing
├── Cargo.toml              # Rust dependencies
├── package.json            # npm package configuration
└── bin/                    # CLI wrapper scripts
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Troubleshooting

### Common Issues

**❌ "Missing GEMINI_API_KEY"**
```bash
export GEMINI_API_KEY="your-api-key-here"
```

**❌ "Image not found" for URLs**
- Ensure the URL is publicly accessible
- Check your internet connection
- Verify the image format is supported

**❌ "Binary not found" after npm install**
```bash
npm run build
```

**❌ Rate limit errors**
- Wait a moment before retrying
- Consider implementing exponential backoff in your client

### Getting Help

- 📚 [Documentation](https://github.com/your-username/gemini-image-mcp/wiki)
- 🐛 [Report Issues](https://github.com/your-username/gemini-image-mcp/issues)
- 💬 [Discussions](https://github.com/your-username/gemini-image-mcp/discussions)

## 🙏 Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io/) - For the excellent MCP standard
- [Google Gemini](https://ai.google.dev/) - For the powerful AI capabilities
- [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) - For the Rust implementation

---

<div align="center">

**Made with ❤️ using Rust and Google Gemini**

[⭐ Star this repo](https://github.com/your-username/gemini-image-mcp) • [🐛 Report Bug](https://github.com/your-username/gemini-image-mcp/issues) • [✨ Request Feature](https://github.com/your-username/gemini-image-mcp/issues)

</div>