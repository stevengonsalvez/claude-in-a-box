#!/usr/bin/env node
// ABOUTME: PTY WebSocket service for claude-in-a-box
// Provides terminal access over WebSocket for the TUI

const WebSocket = require('ws');
const pty = require('node-pty');
const os = require('os');

const PORT = process.env.PTY_PORT || 8080;
const shell = os.platform() === 'win32' ? 'powershell.exe' : 'bash';

console.log(`[PTY Service] Starting on port ${PORT}...`);

// Create WebSocket server
const wss = new WebSocket.Server({ 
  port: PORT,
  perMessageDeflate: {
    zlibDeflateOptions: {
      chunkSize: 1024,
      memLevel: 7,
      level: 3
    },
    zlibInflateOptions: {
      chunkSize: 10 * 1024
    },
    clientNoContextTakeover: true,
    serverNoContextTakeover: true,
    serverMaxWindowBits: 10,
    concurrencyLimit: 10,
    threshold: 1024
  }
});

console.log(`[PTY Service] WebSocket server listening on port ${PORT}`);

// Track active sessions
const sessions = new Map();

wss.on('connection', (ws, req) => {
  console.log(`[PTY Service] New connection from ${req.socket.remoteAddress}`);
  
  let term = null;
  const sessionId = Date.now().toString();
  
  // Create PTY instance
  try {
    term = pty.spawn(shell, [], {
      name: 'xterm-256color',
      cols: 120,
      rows: 40,
      cwd: process.env.HOME || '/workspace',
      env: process.env
    });
    
    console.log(`[PTY Service] Created PTY process with PID ${term.pid}`);
    sessions.set(sessionId, { ws, term });
    
    // Send session init message
    ws.send(JSON.stringify({
      type: 'session_init',
      session_id: sessionId,
      buffer: []
    }));
    
    // Handle PTY output
    term.onData((data) => {
      try {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({
            type: 'output',
            data: data
          }));
        }
      } catch (ex) {
        console.error('[PTY Service] Error sending data:', ex);
      }
    });
    
    term.onExit((code, signal) => {
      console.log(`[PTY Service] PTY process exited with code ${code}, signal ${signal}`);
      ws.send(JSON.stringify({
        type: 'session_ended',
        exit_code: code,
        signal: signal
      }));
      ws.close();
    });
    
  } catch (error) {
    console.error('[PTY Service] Failed to create PTY:', error);
    ws.send(JSON.stringify({
      type: 'error',
      error: error.message
    }));
    ws.close();
    return;
  }
  
  // Handle WebSocket messages
  ws.on('message', (message) => {
    try {
      const msg = JSON.parse(message.toString());
      
      switch (msg.type) {
        case 'input':
          if (term) {
            term.write(msg.data);
          }
          break;
          
        case 'resize':
          if (term && msg.cols && msg.rows) {
            term.resize(msg.cols, msg.rows);
            console.log(`[PTY Service] Resized terminal to ${msg.cols}x${msg.rows}`);
          }
          break;
          
        case 'heartbeat':
          ws.send(JSON.stringify({ type: 'heartbeat_ack' }));
          break;
          
        default:
          console.log(`[PTY Service] Unknown message type: ${msg.type}`);
      }
    } catch (ex) {
      console.error('[PTY Service] Error processing message:', ex);
    }
  });
  
  // Handle WebSocket close
  ws.on('close', () => {
    console.log(`[PTY Service] Connection closed for session ${sessionId}`);
    if (term) {
      term.kill();
      sessions.delete(sessionId);
    }
  });
  
  // Handle WebSocket errors
  ws.on('error', (error) => {
    console.error(`[PTY Service] WebSocket error for session ${sessionId}:`, error);
    if (term) {
      term.kill();
      sessions.delete(sessionId);
    }
  });
});

// Handle server errors
wss.on('error', (error) => {
  console.error('[PTY Service] Server error:', error);
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log('[PTY Service] Received SIGTERM, shutting down...');
  sessions.forEach(({ term }) => {
    if (term) term.kill();
  });
  wss.close(() => {
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  console.log('[PTY Service] Received SIGINT, shutting down...');
  sessions.forEach(({ term }) => {
    if (term) term.kill();
  });
  wss.close(() => {
    process.exit(0);
  });
});

console.log('[PTY Service] Ready to accept connections');