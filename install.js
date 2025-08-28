#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

function getPlatform() {
  const platform = process.platform;
  const arch = process.arch;
  
  if (platform === 'darwin' && (arch === 'x64' || arch === 'arm64')) {
    return { platform: 'x86_64-apple-darwin', ext: '' };
  }
  if (platform === 'linux' && arch === 'x64') {
    return { platform: 'x86_64-unknown-linux-gnu', ext: '' };
  }
  if (platform === 'win32' && arch === 'x64') {
    return { platform: 'x86_64-pc-windows-gnu', ext: '.exe' };
  }
  
  throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

function findBinary() {
  const { platform, ext } = getPlatform();
  const binaryName = `gemini-image-mcp${ext}`;
  
  // Try platform-specific target directory first
  const platformBinary = path.join(__dirname, 'target', platform, 'release', binaryName);
  if (fs.existsSync(platformBinary)) {
    return platformBinary;
  }
  
  // Fall back to default release directory
  const defaultBinary = path.join(__dirname, 'target', 'release', binaryName);
  if (fs.existsSync(defaultBinary)) {
    return defaultBinary;
  }
  
  throw new Error(`Binary not found for platform ${platform}`);
}

function createWrapper() {
  try {
    const binaryPath = findBinary();
    
    // Ensure the binary has execute permissions
    fs.chmodSync(binaryPath, '755');
    
    const wrapperPath = path.join(__dirname, 'bin', 'gemini-image-mcp.js');
    
    const wrapperContent = `#!/usr/bin/env node
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

function getPlatform() {
  const platform = process.platform;
  const arch = process.arch;
  
  if (platform === 'darwin' && (arch === 'x64' || arch === 'arm64')) {
    return { platform: 'x86_64-apple-darwin', ext: '' };
  }
  if (platform === 'linux' && arch === 'x64') {
    return { platform: 'x86_64-unknown-linux-gnu', ext: '' };
  }
  if (platform === 'win32' && arch === 'x64') {
    return { platform: 'x86_64-pc-windows-gnu', ext: '.exe' };
  }
  
  throw new Error(\`Unsupported platform: \${platform}-\${arch}\`);
}

function findBinary() {
  const { platform, ext } = getPlatform();
  const binaryName = \`gemini-image-mcp\${ext}\`;
  
  // Try platform-specific target directory first
  const platformBinary = path.join(__dirname, '..', 'target', platform, 'release', binaryName);
  if (fs.existsSync(platformBinary)) {
    return platformBinary;
  }
  
  // Fall back to default release directory
  const defaultBinary = path.join(__dirname, '..', 'target', 'release', binaryName);
  if (fs.existsSync(defaultBinary)) {
    return defaultBinary;
  }
  
  throw new Error(\`Binary not found for platform \${platform}\`);
}

try {
  const binaryPath = findBinary();
  
  // Ensure binary has execute permissions
  try {
    fs.chmodSync(binaryPath, '755');
  } catch (chmodErr) {
    // Ignore permission errors, binary might already have correct permissions
  }
  
  const child = spawn(binaryPath, process.argv.slice(2), { 
    stdio: 'inherit',
    shell: false
  });

  child.on('exit', (code) => {
    process.exit(code);
  });

  child.on('error', (err) => {
    console.error('Failed to start binary:', err);
    process.exit(1);
  });
} catch (error) {
  console.error('Error:', error.message);
  process.exit(1);
}
`;

    fs.writeFileSync(wrapperPath, wrapperContent);
    fs.chmodSync(wrapperPath, '755');
    
    console.log('✅ Binary wrapper created successfully');
  } catch (error) {
    console.error('❌ Failed to create binary wrapper:', error.message);
    process.exit(1);
  }
}

if (require.main === module) {
  createWrapper();
}

module.exports = { createWrapper };