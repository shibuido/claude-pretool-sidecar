# QA — Quality Assurance Suite

This directory contains everything needed to validate `claude-pretool-sidecar` — checklists, automated tests, helper scripts, and a Docker environment for isolated testing.

## Directory Structure

```
qa/
├── README.md                    # This file
├── checklists/
│   ├── manual-tests.md          # Manual QA checklist for human testers
│   └── programmatic-tests.md    # Automated test checklist and coverage map
├── scripts/
│   ├── run-all-qa.sh            # Master QA runner (all automated tests)
│   ├── test-config.sh           # Config loading and validation tests
│   ├── test-providers.sh        # Provider execution and communication tests
│   ├── test-quorum.sh           # Quorum logic end-to-end tests
│   ├── test-audit.sh            # Audit logging and log rotation tests
│   └── test-hook-integration.sh # Claude Code hook format compliance tests
├── helpers/
│   ├── gen-payload.sh           # Generate hook payloads for testing
│   ├── gen-config.sh            # Generate test config files
│   ├── check-audit-log.sh       # Validate audit log entries
│   └── provider-echo.sh         # Configurable mock provider
├── fixtures/
│   ├── configs/                 # Sample configs for QA scenarios
│   └── payloads/                # Sample hook payloads
└── docker/
    ├── Dockerfile               # Isolated QA environment
    └── qa-docker.sh             # Docker build/run/test/cleanup orchestration
```

## Quick Start

### Run All Tests Locally

```bash
cd qa/
./scripts/run-all-qa.sh
```

### Run Tests in Docker (Isolated)

```bash
cd qa/docker/
./qa-docker.sh build    # Build the QA image
./qa-docker.sh test     # Run all QA tests in container
./qa-docker.sh cleanup  # Remove containers and images
```

### Manual Testing

Open `checklists/manual-tests.md` and work through each scenario.

## Conventions

* All scripts are POSIX-compatible (`#!/bin/sh`) where possible, `bash` where needed
* Scripts exit 0 on success, non-zero on failure
* Each test prints `PASS: <description>` or `FAIL: <description>`
* Colors used only when stdout is a terminal
