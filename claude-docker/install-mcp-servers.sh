#!/bin/bash
# ABOUTME: Install MCP servers from configuration file

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCP_SERVERS_FILE="$SCRIPT_DIR/mcp-servers.txt"

echo "Installing MCP servers from configuration..."

if [ ! -f "$MCP_SERVERS_FILE" ]; then
    echo "ERROR: MCP servers configuration file not found: $MCP_SERVERS_FILE"
    exit 1
fi

# Read and process each line
while IFS= read -r line || [ -n "$line" ]; do
    # Skip empty lines and comments
    if [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]]; then
        continue
    fi
    
    # Check if line contains Twilio configuration
    if [[ "$line" =~ twilio ]]; then
        # Only install Twilio MCP if environment variables are set
        if [ -n "$TWILIO_ACCOUNT_SID" ] && [ -n "$TWILIO_AUTH_TOKEN" ]; then
            echo "Installing Twilio MCP server..."
            # Substitute environment variables in the command
            expanded_line=$(echo "$line" | envsubst)
            echo "Running: $expanded_line"
            eval "$expanded_line" || {
                echo "WARNING: Failed to install Twilio MCP server"
                continue
            }
        else
            echo "Skipping Twilio MCP server (environment variables not set)"
            continue
        fi
    else
        # Install other MCP servers
        echo "Running: $line"
        eval "$line" || {
            echo "WARNING: Failed to install MCP server: $line"
            continue
        }
    fi
done < "$MCP_SERVERS_FILE"

echo "MCP server installation completed"