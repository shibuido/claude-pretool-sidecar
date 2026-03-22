# Claude Code Docker Install Methods

*2026-03-23 — Researched for Dockerfile.claude-code*

## Summary

Four install methods exist. Only the native installer is recommended for Docker.

## Methods

### 1. Native Installer (Recommended)

```bash
curl -fsSL https://claude.ai/install.sh | bash
```

* Downloads standalone binary — **no Node.js needed**
* Binary lands at `~/.local/bin/claude`
* Verifies SHA256 checksums
* Supports pinning: `bash -s 1.0.58`
* Handles glibc vs musl Linux
* Disable auto-updater in containers: `DISABLE_AUTOUPDATER=1` in settings.json

### 2. npm (Deprecated)

```bash
npm install -g @anthropic-ai/claude-code
```

* Requires Node.js 18+
* Package `@anthropic-ai/claude-code` on npm
* Binary at `/usr/local/bin/claude`
* **Officially deprecated** — migration path is the native installer

### 3. Dev Container Feature

```json
{"features": {"ghcr.io/anthropics/devcontainer-features/claude-code:1": {}}}
```

* Only works with devcontainer CLI tooling, **not plain `docker build`**
* Internally uses npm install
* Installs Node.js 18 if absent

### 4. Docker Desktop Sandbox

* Managed by Docker Desktop app
* Not a public Dockerfile — opaque image
* Not useful for CI/CD or custom Docker builds

## Key Settings for Non-Interactive Docker

```bash
claude -p "query"                      # Non-interactive mode
--dangerously-skip-permissions         # Skip tool approval prompts
--bare                                 # Skip hook/plugin discovery (clean room)
--settings '{"hooks":...}'             # Inject settings inline
ANTHROPIC_API_KEY=sk-ant-...           # Auth via env var
DISABLE_AUTOUPDATER=1                  # No background updates
CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1  # Minimize network traffic
```

## Alpine Linux Caveat

Native binary needs glibc. For Alpine (musl):

```dockerfile
RUN apk add --no-cache libgcc libstdc++ ripgrep
# Then set USE_BUILTIN_RIPGREP=0 in settings.json
```

## References

* https://claude.com/download
* https://code.claude.com/docs/en/overview
* https://code.claude.com/docs/en/setup (deprecation notice)
* https://github.com/anthropics/devcontainer-features/blob/main/src/claude-code/install.sh
