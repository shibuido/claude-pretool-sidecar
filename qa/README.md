# QA — Quality Assurance Suite

This directory contains everything needed to validate `claude-pretool-sidecar`.

**Two test categories** are clearly distinguished by filename:

* **`standalone-*`** — Tests that run without Claude Code CLI. Only need the sidecar binary.
* **`live-claude-code-*`** — Tests that require a working Claude Code CLI + `ANTHROPIC_API_KEY`. Make real API calls.

## Directory Structure

```
qa/
├── README.md
├── checklists/
│   ├── standalone-manual.md               # Manual QA — no Claude Code needed
│   ├── standalone-programmatic.md         # Coverage map for standalone scripts
│   └── live-claude-code-manual.md         # Manual QA — requires Claude Code CLI
├── scripts/
│   ├── run-all-standalone.sh              # Master runner: all standalone tests
│   ├── run-all-live-claude-code.sh        # Master runner: all live CC tests
│   ├── standalone-config.sh               # Config loading tests
│   ├── standalone-providers.sh            # Provider execution tests
│   ├── standalone-quorum.sh              # Quorum logic tests
│   ├── standalone-audit.sh               # Audit logging tests
│   ├── standalone-hook-format.sh         # Hook format compliance tests
│   ├── live-claude-code-hook-install.sh   # Hook installation with CC
│   └── live-claude-code-hook-execution.sh # Hook execution via CC CLI
├── helpers/
│   ├── gen-payload.sh                     # Generate hook payloads
│   ├── gen-config.sh                      # Generate TOML configs
│   ├── provider-echo.sh                   # Configurable mock provider
│   └── check-audit-log.sh                # Validate audit log format
├── fixtures/
│   └── payloads/                          # Sample hook payloads
└── docker/
    ├── Dockerfile.standalone              # QA image WITHOUT Claude Code
    ├── Dockerfile.claude-code             # QA image WITH Claude Code CLI
    ├── cpts-standalone.sh                 # Manage standalone Docker env
    └── cpts-claude-code.sh               # Manage Claude Code Docker env
```

## Quick Start

### Standalone Tests (no Claude Code needed)

```bash
# Locally
qa/scripts/run-all-standalone.sh

# In Docker
qa/docker/cpts-standalone.sh build
qa/docker/cpts-standalone.sh test
```

### Live Claude Code Tests (requires API key)

```bash
# Locally
export ANTHROPIC_API_KEY="sk-ant-..."
qa/scripts/run-all-live-claude-code.sh

# In Docker
export ANTHROPIC_API_KEY="sk-ant-..."
qa/docker/cpts-claude-code.sh build
qa/docker/cpts-claude-code.sh test
```

## Docker Management Commands

Both `cpts-standalone.sh` and `cpts-claude-code.sh` support:

| Command | Description |
|---------|-------------|
| `build` | Build the Docker image |
| `test` | Run tests in container |
| `shell` | Interactive bash shell |
| `exec <cmd>` | Run arbitrary command |
| `status` | Show images and containers |
| `logs` | Show logs from last run |
| `destroy` | Remove all containers and images |

`cpts-claude-code.sh` additionally supports:

| Command | Description |
|---------|-------------|
| `test-standalone` | Run standalone tests (no API key) in CC image |

### Container Prefixes

* `cpts-standalone-*` — Standalone environment artifacts
* `cpts-claude-code-*` — Claude Code environment artifacts

Override with `CPTS_DOCKER_PREFIX` env var.

## Conventions

* Scripts exit 0 on success, non-zero on failure
* Each test prints `PASS: <description>` or `FAIL: <description>`
* Live tests print `SKIP: <reason>` when prerequisites missing
* Filenames tell you what's needed: `standalone-` vs `live-claude-code-`
