# ABOUTME: Claude development environment Dockerfile based on claude-docker
# Provides a complete development environment with Claude CLI and MCP servers

ARG BASE_IMAGE=node:20-slim
FROM ${BASE_IMAGE}

# Build arguments
ARG HOST_UID=1000
ARG HOST_GID=1000
ARG PACKAGES=""

# Update and install base packages including gh CLI
RUN apt-get update && apt-get install -y \
    git \
    curl \
    python3 \
    python3-pip \
    build-essential \
    sudo \
    gettext-base \
    gnupg \
    software-properties-common \
    ${PACKAGES} \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update \
    && apt-get install -y gh \
    && rm -rf /var/lib/apt/lists/*

# Create claude-user with matching host UID/GID for proper file permissions
# Check if GID exists and create group accordingly
RUN if getent group ${HOST_GID} >/dev/null 2>&1; then \
        groupmod -n claude-user $(getent group ${HOST_GID} | cut -d: -f1); \
    else \
        groupadd -g ${HOST_GID} claude-user; \
    fi && \
    if getent passwd ${HOST_UID} >/dev/null 2>&1; then \
        usermod -l claude-user -d /home/claude-user -m $(getent passwd ${HOST_UID} | cut -d: -f1) && \
        usermod -g ${HOST_GID} claude-user; \
    else \
        useradd -u ${HOST_UID} -g ${HOST_GID} -m -s /bin/bash claude-user; \
    fi && \
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

# Install Claude CLI
ENV PATH="/home/claude-user/.npm-global/bin:$PATH"
RUN npm install -g @anthropic-ai/claude-code

# Install default MCP servers
RUN /app/scripts/install-mcp-servers-simple.sh
# Note: Using simple script for now. Complex script in install-mcp-servers.sh has minor exit code issue

# Authentication and environment variables are handled at runtime
# via volume mounts and environment variables - no build-time copying needed

# Set working directory to workspace mount point
WORKDIR /workspace

# Default command runs the startup script which handles Claude execution
ENTRYPOINT ["/app/scripts/startup.sh"]