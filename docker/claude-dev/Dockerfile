# ABOUTME: Claude development environment Dockerfile based on claude-docker
# Provides a complete development environment with Claude CLI and MCP servers

ARG BASE_IMAGE=node:20-slim
FROM ${BASE_IMAGE}

# Build arguments
ARG HOST_UID=1000
ARG HOST_GID=1000
ARG PACKAGES=""

# Update and install base packages
RUN apt-get update && apt-get install -y \
    git \
    curl \
    python3 \
    python3-pip \
    build-essential \
    sudo \
    ${PACKAGES} \
    && rm -rf /var/lib/apt/lists/*

# Create claude-user with matching host UID/GID for proper file permissions
RUN groupadd -g ${HOST_GID} claude-user && \
    useradd -u ${HOST_UID} -g ${HOST_GID} -m -s /bin/bash claude-user && \
    echo "claude-user ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Set up npm global directory for claude-user
RUN mkdir -p /home/claude-user/.npm-global && \
    chown -R claude-user:claude-user /home/claude-user/.npm-global

# Switch to claude-user
USER claude-user
WORKDIR /home/claude-user

# Configure npm to use global directory
RUN npm config set prefix '/home/claude-user/.npm-global' && \
    echo 'export PATH=/home/claude-user/.npm-global/bin:$PATH' >> ~/.bashrc

# Create app directory and copy scripts
COPY --chown=claude-user:claude-user scripts/ /app/scripts/
COPY --chown=claude-user:claude-user config/ /app/config/

# Make scripts executable
RUN chmod +x /app/scripts/*.sh

# Install AI CLI tools
ENV PATH="/home/claude-user/.npm-global/bin:$PATH"
RUN npm install -g @anthropic-ai/claude-code @google/gemini-cli

# Install default MCP servers
RUN /app/scripts/install-mcp-servers.sh

# Copy authentication and environment (if available during build)
COPY --chown=claude-user:claude-user .claude.json /home/claude-user/.claude.json 2>/dev/null || true
COPY --chown=claude-user:claude-user .env /app/.env 2>/dev/null || true

# Set working directory to workspace mount point
WORKDIR /workspace

# Default command runs the startup script
ENTRYPOINT ["/app/scripts/startup.sh"]
CMD ["claude", "--no-git-ignore"]