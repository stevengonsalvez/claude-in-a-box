#!/bin/bash
# ABOUTME: MCP server installation script for claude-dev container
# Installs MCP servers based on configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[mcp-install]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[mcp-install]${NC} $1"
}

error() {
    echo -e "${RED}[mcp-install]${NC} $1"
}

success() {
    echo -e "${GREEN}[mcp-install]${NC} $1"
}

# Default MCP servers to install
DEFAULT_SERVERS=(
    "npm:@ambergristle/serena:Serena - AI coding agent"
    "npm:context7:Context7 - Library documentation and examples"
    "npm:@twilio-labs/mcp-server-twilio:Twilio - SMS messaging (optional)"
)

# Function to install npm package
install_npm_package() {
    local package="$1"
    local description="$2"
    
    log "Installing ${package} (${description})"
    if npm install -g "${package}"; then
        success "Installed ${package}"
    else
        error "Failed to install ${package}"
        return 1
    fi
}

# Function to check if environment variable is set
check_env() {
    local var_name="$1"
    if [ -z "${!var_name}" ]; then
        return 1
    fi
    return 0
}

# Main installation
log "Installing MCP servers..."

# Install each default server
for server_def in "${DEFAULT_SERVERS[@]}"; do
    IFS=':' read -r type package description <<< "$server_def"
    
    case "$type" in
        "npm")
            # Check if this is an optional server with env requirements
            if [[ "$package" == *"twilio"* ]]; then
                if check_env "TWILIO_AUTH_TOKEN" && check_env "TWILIO_ACCOUNT_SID"; then
                    install_npm_package "$package" "$description"
                else
                    warn "Skipping ${package} - missing TWILIO_* environment variables"
                fi
            else
                install_npm_package "$package" "$description"
            fi
            ;;
        *)
            warn "Unknown server type: $type"
            ;;
    esac
done

# Install servers from custom config if it exists
if [ -f /app/config/mcp-servers.txt ]; then
    log "Installing additional servers from mcp-servers.txt"
    
    while IFS= read -r line; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
        
        # Parse line format: type:package:description:env_vars
        IFS=':' read -r type package description env_vars <<< "$line"
        
        # Check environment variables if specified
        if [ -n "$env_vars" ]; then
            env_check_failed=false
            IFS=',' read -ra ENV_VARS <<< "$env_vars"
            for env_var in "${ENV_VARS[@]}"; do
                if ! check_env "$env_var"; then
                    warn "Skipping ${package} - missing ${env_var} environment variable"
                    env_check_failed=true
                    break
                fi
            done
            
            if [ "$env_check_failed" = true ]; then
                continue
            fi
        fi
        
        case "$type" in
            "npm")
                install_npm_package "$package" "$description"
                ;;
            "pip")
                log "Installing Python package ${package} (${description})"
                if pip install "${package}"; then
                    success "Installed ${package}"
                else
                    error "Failed to install ${package}"
                fi
                ;;
            *)
                warn "Unknown server type: $type for package $package"
                ;;
        esac
    done < /app/config/mcp-servers.txt
fi

success "MCP server installation completed"

# Generate MCP configuration for Claude/Gemini
log "Generating MCP configuration..."

# Create .claude directory if it doesn't exist
mkdir -p /home/claude-user/.claude

# Generate basic MCP configuration
cat > /home/claude-user/.claude/mcp-config.json << 'EOF'
{
  "mcpServers": {
    "serena": {
      "command": "node",
      "args": ["/home/claude-user/.npm-global/lib/node_modules/@ambergristle/serena/out/index.js"]
    },
    "context7": {
      "command": "node", 
      "args": ["/home/claude-user/.npm-global/lib/node_modules/context7/index.js"]
    }
  }
}
EOF

# Add Twilio if environment variables are present
if check_env "TWILIO_AUTH_TOKEN" && check_env "TWILIO_ACCOUNT_SID" && check_env "TWILIO_FROM_PHONE"; then
    log "Adding Twilio MCP server to configuration"
    cat > /tmp/twilio-config.json << EOF
{
  "mcpServers": {
    "serena": {
      "command": "node",
      "args": ["/home/claude-user/.npm-global/lib/node_modules/@ambergristle/serena/out/index.js"]
    },
    "context7": {
      "command": "node", 
      "args": ["/home/claude-user/.npm-global/lib/node_modules/context7/index.js"]
    },
    "twilio": {
      "command": "node",
      "args": ["/home/claude-user/.npm-global/lib/node_modules/@twilio-labs/mcp-server-twilio/bin/run"],
      "env": {
        "TWILIO_AUTH_TOKEN": "${TWILIO_AUTH_TOKEN}",
        "TWILIO_ACCOUNT_SID": "${TWILIO_ACCOUNT_SID}",
        "TWILIO_FROM_PHONE": "${TWILIO_FROM_PHONE}"
      }
    }
  }
}
EOF
    mv /tmp/twilio-config.json /home/claude-user/.claude/mcp-config.json
fi

success "MCP configuration generated at /home/claude-user/.claude/mcp-config.json"