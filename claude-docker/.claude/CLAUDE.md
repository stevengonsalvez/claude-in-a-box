# Claude-in-a-Box Container Environment

## Welcome to Claude-in-a-Box!

This container provides a complete development environment with:

- **Claude Code CLI** - AI-powered coding assistant
- **MCP Servers** - Model Context Protocol servers for enhanced functionality
- **Development Tools** - Git, Node.js, Python, and build tools

## Available MCP Servers

### Serena (Coding Agent Toolkit)
- **Purpose**: AI coding agent with advanced development capabilities
- **Command**: Available through Claude CLI
- **Features**: IDE assistance, code generation, debugging help

### Context7 (Documentation & Examples)
- **Purpose**: Up-to-date documentation and code examples
- **Command**: Available through Claude CLI
- **Features**: Real-time library documentation, code examples

### Twilio SMS (Optional)
- **Purpose**: Send SMS notifications from your development environment
- **Requirements**: TWILIO_ACCOUNT_SID, TWILIO_AUTH_TOKEN, TWILIO_FROM_NUMBER
- **Features**: SMS notifications for builds, deployments, alerts

## Usage

This container is designed to be used with Claude-in-a-Box TUI application.

### Environment Variables

The container expects these environment variables to be set:

- `ANTHROPIC_API_KEY` - Required for Claude Code CLI
- `TWILIO_ACCOUNT_SID` - Optional for SMS notifications
- `TWILIO_AUTH_TOKEN` - Optional for SMS notifications  
- `TWILIO_FROM_NUMBER` - Optional for SMS notifications

### File Structure

- `/workspace` - Your project files are mounted here
- `/app` - Container application files
- `/home/claude-user` - User home directory with Claude configuration

## Getting Started

1. Make sure you have authenticated with Claude Code on your host system
2. Set your API keys in the container environment
3. Start coding with AI assistance!

## Support

This container is part of the Claude-in-a-Box project for isolated development environments.