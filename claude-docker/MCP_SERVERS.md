# MCP Server Management

This document explains how to add and manage MCP (Model Context Protocol) servers in the Docker Claude setup.

## Quick Start

To add a new MCP server:

1. Edit `mcp-servers.txt`
2. Add your MCP server installation command
3. Rebuild the Docker image

## File Structure

- `mcp-servers.txt` - List of MCP server installation commands
- `install-mcp-servers.sh` - Script that processes and installs MCP servers
- `.env` - Environment variables (for MCP servers that need API keys)

## Adding MCP Servers

### Simple MCP Servers (No Environment Variables)

⚠️ **IMPORTANT**: Always use `-s user` flag to make MCPs available across all projects!

Add a line like this to `mcp-servers.txt`:
```bash
claude mcp add -s user <name> -- <command> <args>
```

Example:
```bash
claude mcp add -s user filesystem -- npx -y @modelcontextprotocol/server-filesystem
```

**Without `-s user`**: MCP will only be available in the Docker build directory (`/app`)
**With `-s user`**: MCP will be available in any project directory (`/workspace`, etc.)

### MCP Servers with Environment Variables

For servers that need API keys or configuration:
```bash
claude mcp add-json <name> -s user '{"command":"...","args":[...],"env":{"KEY":"${ENV_VAR}"}}'
```

Example:
```bash
claude mcp add-json github -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"${GITHUB_TOKEN}"}}'
```

## Environment Variables

1. Add required variables to `.env`:
```env
GITHUB_TOKEN=your_token_here
ANTHROPIC_API_KEY=your_key_here
```

2. Reference them in `mcp-servers.txt` using `${VAR_NAME}` syntax

3. The install script will:
   - Skip servers with missing required env vars
   - Log which variables are missing
   - Continue installing other servers

## Currently Installed MCP Servers

- **Serena** - Powerful coding agent toolkit with project indexing and symbol manipulation
- **Context7** - Pulls up-to-date, version-specific documentation and code examples straight from the source into your prompt
- **Twilio** - SMS messaging for task completion notifications (requires TWILIO_* env vars)

## Examples of Popular MCP Servers

```bash
# Filesystem access
claude mcp add -s user filesystem -- npx -y @modelcontextprotocol/server-filesystem

# GitHub integration
claude mcp add-json github -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"${GITHUB_TOKEN}"}}'

# Browser automation
claude mcp add -s user browser -- npx -y @modelcontextprotocol/server-browser

# Memory/knowledge base
claude mcp add -s user memory -- npx -y @modelcontextprotocol/server-memory

# PostgreSQL database
claude mcp add-json postgres -s user '{"command":"npx","args":["-y","@modelcontextprotocol/server-postgres"],"env":{"POSTGRES_URL":"${DATABASE_URL}"}}'
```

## Troubleshooting

### MCP Server Not Installing
- Check if required environment variables are set in `.env`
- Run `docker-compose build --no-cache` to rebuild with latest changes
- Check Docker build logs for error messages

### Finding MCP Server Commands
Most MCP servers provide installation instructions on their GitHub pages. Look for:
- `claude mcp add` commands
- `npx` commands that can be wrapped in `claude mcp add`
- JSON configurations for servers with environment variables

### Debugging
The install script logs:
- Which servers are being installed
- Missing environment variables
- Success/failure for each installation
- Continues even if one server fails