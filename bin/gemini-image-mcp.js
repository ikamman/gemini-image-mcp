#!/usr/bin/env node
const { spawn } = require('child_process');
const path = require('path');

const binaryPath = path.join(__dirname, '..', 'target/release/gemini-image-mcp');
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
