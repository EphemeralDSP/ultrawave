# Agent Development Container Design Document

## Overview

A mutable development container that serves as a sandboxed environment for AI agents to safely execute code, run builds, and perform development tasks. The container functions as a lightweight development VM with all necessary tooling pre-installed.

## Goals

1. **Isolation**: Provide a safe sandbox where agents can execute arbitrary code without affecting the host system
2. **Persistence**: Support mutable state across container restarts (unlike ephemeral containers)
3. **Tooling**: Pre-install all development tools agents commonly need
4. **Identity**: Manage SSH keys for Git operations (clone, push, etc.)
5. **Reproducibility**: Consistent environment across different host systems

## Non-Goals

- GUI applications (headless only)
- GPU passthrough (CPU-only compute)
- Multi-user isolation (single user per container)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Host System                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              Agent Dev Container (systemd)                 │  │
│  │                                                            │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │  │
│  │  │   Dev Tools  │  │   Dotfiles   │  │  SSH Keys    │    │  │
│  │  │  - node/npm  │  │   (yadm)     │  │  (injected)  │    │  │
│  │  │  - uv/python │  │              │  │              │    │  │
│  │  │  - claude    │  │              │  │              │    │  │
│  │  │  - git/gh    │  │              │  │              │    │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘    │  │
│  │                                                            │  │
│  │  ┌──────────────────────────────────────────────────────┐ │  │
│  │  │                  Workspace Volume                     │ │  │
│  │  │  /home/dev/ws (bind mount or named volume)           │ │  │
│  │  └──────────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Components

### 1. Base Image

**Choice: Ubuntu 24.04 LTS (Noble)**

Rationale:
- Broad package availability
- Long-term support
- Familiar to most developers and agents
- Good systemd support in container mode

Alternative considered: Fedora (better systemd integration but smaller package ecosystem)

### 2. Container Runtime

**Choice: Docker with systemd init**

```dockerfile
# Enable systemd as init
CMD ["/sbin/init"]
```

Rationale:
- Systemd provides proper service management
- Enables running services like SSH daemon
- Better matches a real development environment
- Supports `systemctl` commands agents may use

Requirements:
- `--privileged` or appropriate capabilities
- `/run` and `/sys/fs/cgroup` mounts

### 3. Development Tools

| Tool | Purpose | Installation Method |
|------|---------|---------------------|
| **Node.js 22 LTS** | JavaScript runtime | NodeSource repo |
| **npm** | Package manager | Bundled with Node |
| **npx** | Package runner | Bundled with npm |
| **uv** | Python package manager | Official installer |
| **Python 3.12** | Python runtime | System package + uv |
| **Rust + Cargo** | Rust toolchain | rustup |
| **Clang/LLVM** | C/C++ compiler toolchain | System package (llvm-18) |
| **git** | Version control | System package |
| **gh** | GitHub CLI | GitHub repo |
| **claude** | Claude Code CLI | npm install -g @anthropic-ai/claude-code |
| **opencode** | OpenCode CLI | npm install -g opencode |
| **yadm** | Dotfile manager | System package |

### 3a. Build Toolchain Details

**Rust Toolchain (via rustup):**
- `rustc` - Rust compiler
- `cargo` - Package manager and build tool
- `rustfmt` - Code formatter
- `clippy` - Linter
- Default to stable channel, with ability to switch

**Clang/LLVM Toolchain:**
- `clang` / `clang++` - C/C++ compilers
- `lld` - Fast linker
- `lldb` - Debugger
- `clang-format` - Code formatter
- `clang-tidy` - Static analyzer
- `libc++` - C++ standard library

**Additional Build Tools:**
- `cmake` - Build system generator
- `ninja` - Fast build system
- `pkg-config` - Library configuration
- `meson` - Modern build system

### 4. Supporting Tools (from dotfiles/Brewfile)

These will be installed via the bootstrap script or directly:
- `bat` - Better cat
- `delta` - Better git diff
- `eza` - Better ls
- `fd` - Better find
- `fzf` - Fuzzy finder
- `jq` - JSON processor
- `lazygit` - Git TUI
- `ripgrep` - Better grep
- `tmux` - Terminal multiplexer
- `zoxide` - Smart cd
- `helix` / `neovim` - Editors

### 5. Dotfiles Integration

**Approach: yadm clone on first boot**

```bash
# Clone dotfiles
yadm clone https://github.com/jefffm/dotfiles --no-bootstrap

# Set class for container environment
yadm config local.class "home-linux"

# Run bootstrap
yadm bootstrap
```

The bootstrap script will:
1. Detect the environment as `home-linux`
2. Set up SSH socket directory
3. Install Rust (via rustup)
4. Skip macOS/Homebrew steps
5. Install Linux-specific packages if needed

### 6. SSH Key Management

**Option A: Generate on Boot (Recommended for ephemeral use)**
```bash
# Generate if not exists
if [ ! -f ~/.ssh/id_ed25519 ]; then
    ssh-keygen -t ed25519 -N "" -f ~/.ssh/id_ed25519
    echo "=== SSH Public Key ==="
    cat ~/.ssh/id_ed25519.pub
    echo "======================"
fi
```

**Option B: Inject Existing Key (Recommended for persistent use)**
```yaml
# docker-compose.yml
secrets:
  ssh_key:
    file: ./secrets/id_ed25519

services:
  dev:
    secrets:
      - source: ssh_key
        target: /home/dev/.ssh/id_ed25519
        mode: 0600
```

**Option C: Agenix Integration (For NixOS hosts)**

If running on a NixOS host with agenix, the SSH key can be decrypted and mounted:
```nix
# Add to secrets.nix
"secrets/agent-dev-ssh.age".publicKeys = users ++ systems;
```

Then mount the decrypted secret into the container.

**Recommendation**: Start with Option A for simplicity. The container outputs the public key on first boot, which can be added to GitHub. For production use, migrate to Option B or C.

---

## File Structure

```
agent-dev-container/
├── Dockerfile
├── docker-compose.yml
├── scripts/
│   ├── entrypoint.sh       # Main entrypoint
│   ├── setup-user.sh       # Create dev user
│   ├── install-tools.sh    # Install dev tools
│   └── setup-dotfiles.sh   # Clone and bootstrap dotfiles
├── config/
│   └── sshd_config         # SSH daemon config (optional)
└── secrets/
    └── .gitkeep            # For SSH keys (gitignored)
```

---

## Dockerfile Outline

```dockerfile
FROM ubuntu:24.04

# Metadata
LABEL maintainer="jeff"
LABEL description="Agent development sandbox"

# Prevent interactive prompts
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=America/New_York

# Install base packages
RUN apt-get update && apt-get install -y \
    # Core
    systemd systemd-sysv \
    sudo curl wget ca-certificates gnupg \
    # Development
    git build-essential \
    # Shell
    zsh tmux \
    # Utilities
    jq htop tree unzip \
    # SSH
    openssh-client openssh-server \
    && rm -rf /var/lib/apt/lists/*

# Install yadm
RUN apt-get update && apt-get install -y yadm && rm -rf /var/lib/apt/lists/*

# Install Node.js 22
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs

# Install GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
    && apt-get update && apt-get install -y gh

# Install uv (Python package manager)
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

# Install LLVM/Clang 18 toolchain
RUN apt-get update && apt-get install -y \
    clang-18 clang++-18 \
    lld-18 lldb-18 \
    clang-format-18 clang-tidy-18 \
    libc++-18-dev libc++abi-18-dev \
    && rm -rf /var/lib/apt/lists/* \
    # Set up alternatives for default clang/clang++
    && update-alternatives --install /usr/bin/clang clang /usr/bin/clang-18 100 \
    && update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-18 100 \
    && update-alternatives --install /usr/bin/lld lld /usr/bin/lld-18 100 \
    && update-alternatives --install /usr/bin/lldb lldb /usr/bin/lldb-18 100 \
    && update-alternatives --install /usr/bin/clang-format clang-format /usr/bin/clang-format-18 100 \
    && update-alternatives --install /usr/bin/clang-tidy clang-tidy /usr/bin/clang-tidy-18 100

# Install build tools
RUN apt-get update && apt-get install -y \
    cmake ninja-build pkg-config meson \
    && rm -rf /var/lib/apt/lists/*

# Install modern CLI tools
RUN apt-get update && apt-get install -y \
    bat fd-find ripgrep fzf zoxide \
    && rm -rf /var/lib/apt/lists/*

# Create dev user
RUN useradd -m -s /bin/zsh -G sudo dev \
    && echo "dev ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers.d/dev

# Switch to dev user for remaining setup
USER dev
WORKDIR /home/dev

# Install Rust via rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . "$HOME/.cargo/env" \
    && rustup component add rustfmt clippy

# Add cargo to PATH
ENV PATH="/home/dev/.cargo/bin:$PATH"

# Install global npm packages
RUN npm install -g @anthropic-ai/claude-code opencode

# Set up PATH for uv
ENV PATH="/home/dev/.local/bin:$PATH"

# Copy entrypoint
COPY --chown=dev:dev scripts/entrypoint.sh /home/dev/entrypoint.sh
RUN chmod +x /home/dev/entrypoint.sh

# Volumes
VOLUME ["/home/dev/ws"]

# Expose SSH (optional, for remote access)
EXPOSE 22

# Use systemd as init
STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
```

---

## Entrypoint Script

```bash
#!/bin/bash
set -e

# SSH Key Setup
if [ ! -f ~/.ssh/id_ed25519 ]; then
    echo "Generating SSH key..."
    mkdir -p ~/.ssh
    chmod 700 ~/.ssh
    ssh-keygen -t ed25519 -N "" -f ~/.ssh/id_ed25519 -C "agent-dev-container"

    echo ""
    echo "=========================================="
    echo "  SSH PUBLIC KEY (add to GitHub)"
    echo "=========================================="
    cat ~/.ssh/id_ed25519.pub
    echo "=========================================="
    echo ""
fi

# Dotfiles Setup (only on first run)
if [ ! -d ~/.config/yadm ]; then
    echo "Setting up dotfiles..."
    yadm clone https://github.com/jefffm/dotfiles --no-bootstrap || true
    yadm config local.class "home-linux"
    yadm bootstrap || true
fi

# Start shell or passed command
exec "$@"
```

---

## Docker Compose Configuration

```yaml
version: '3.8'

services:
  agent-dev:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: agent-dev
    hostname: agent-dev

    # Required for systemd
    privileged: true

    # Volumes
    volumes:
      - dev-home:/home/dev
      - ./workspace:/home/dev/ws
      - /sys/fs/cgroup:/sys/fs/cgroup:rw

    # Networking
    ports:
      - "2222:22"  # SSH access (optional)

    # Environment
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - GITHUB_TOKEN=${GITHUB_TOKEN}

    # Keep running
    stdin_open: true
    tty: true

    # Restart policy
    restart: unless-stopped

volumes:
  dev-home:
    name: agent-dev-home
```

---

## Usage

### Building
```bash
cd agent-dev-container
docker compose build
```

### Starting
```bash
docker compose up -d
```

### Accessing
```bash
# Interactive shell
docker compose exec agent-dev zsh

# Or via SSH (if enabled)
ssh -p 2222 dev@localhost
```

### Stopping
```bash
docker compose down
```

### Resetting (fresh start)
```bash
docker compose down -v  # Removes volumes
docker compose up -d
```

---

## Security Considerations

1. **Privileged Mode**: Required for systemd but grants full host access
   - Mitigation: Run on dedicated VM or use rootless Docker

2. **API Keys**: Passed via environment variables
   - Mitigation: Use Docker secrets for production

3. **SSH Keys**: Generated in container
   - Mitigation: Use short-lived keys, rotate regularly

4. **Network Access**: Container has full network access
   - Mitigation: Use Docker network policies if needed

---

## Future Enhancements

1. **NixOS Container**: Replace Dockerfile with NixOS container for reproducibility
2. **Devcontainer Spec**: Add `.devcontainer.json` for VS Code Remote Containers
3. **Multi-arch Support**: Build for both amd64 and arm64
4. **Health Checks**: Add Docker health checks for monitoring
5. **Resource Limits**: Add CPU/memory limits for safety
6. **Audit Logging**: Log all commands executed in container

---

## Implementation Phases

### Phase 1: Basic Container
- [ ] Dockerfile with core tools
- [ ] Basic entrypoint with SSH key generation
- [ ] docker-compose.yml

### Phase 2: Dotfiles Integration
- [ ] yadm clone and bootstrap
- [ ] Environment class detection
- [ ] Shell configuration

### Phase 3: Agent Tools
- [ ] Claude Code installation and configuration
- [ ] OpenCode installation
- [ ] API key management

### Phase 4: Polish
- [ ] Documentation
- [ ] Health checks
- [ ] Resource limits
- [ ] Cleanup scripts
