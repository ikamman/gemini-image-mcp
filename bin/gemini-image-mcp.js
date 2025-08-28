#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');

function findBinary() {
  const platform = process.platform;
  const ext = platform === 'win32' ? '.exe' : '';
  const binaryName = `gemini-image-mcp${ext}`;
  
  // Try to find the binary in various locations
  const possiblePaths = [
    path.join(__dirname, '..', 'target', 'release', binaryName),
    path.join(__dirname, '..', 'target', 'x86_64-apple-darwin', 'release', binaryName),
    path.join(__dirname, '..', 'target', 'x86_64-unknown-linux-gnu', 'release', binaryName),
    path.join(__dirname, '..', 'target', 'x86_64-pc-windows-gnu', 'release', binaryName)
  ];
  
  for (const binaryPath of possiblePaths) {
    try {
      require('fs').accessSync(binaryPath, require('fs').constants.F_OK);
      return binaryPath;
    } catch (e) {
      // Continue to next path
    }
  }
  
  throw new Error(`Binary not found. Expected one of: ${possiblePaths.join(', ')}`);
}

try {
  const binaryPath = findBinary();
  const child = spawn(binaryPath, process.argv.slice(2), { 
    stdio: 'inherit',
    shell: false 
  });

  child.on('exit', (code) => {
    process.exit(code || 0);
  });

  child.on('error', (err) => {
    console.error('Failed to start gemini-image-mcp:', err.message);
    process.exit(1);
  });

  // Handle Ctrl+C
  process.on('SIGINT', () => {
    child.kill('SIGINT');
  });

  process.on('SIGTERM', () => {
    child.kill('SIGTERM');
  });

} catch (error) {
  console.error('Error:', error.message);
  console.error('Make sure to run `npm run build` to compile the binary first.');
  process.exit(1);
}