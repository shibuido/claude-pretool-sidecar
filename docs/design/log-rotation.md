# Design: Log Rotation and Size Management

*Date: 2026-03-22*

## Problem

Audit logs can grow unbounded over time. Users need:

1. **Date-based chunking** — log files named with dates for easy identification
2. **Size limits** — automatic cleanup when total log size exceeds threshold
3. **Graceful degradation** — when the current file is too large, retain only recent lines

## Log File Naming

```
{output_dir}/audit-{YYYY-MM-DD}.jsonl
```

Examples:
```
/var/log/claude-pretool-sidecar/audit-2026-03-22.jsonl
/var/log/claude-pretool-sidecar/audit-2026-03-23.jsonl
```

When `audit.output` is a file path, we interpret it as the **directory** for date-chunked files.
When it contains `%date%`, we substitute with the current date.

## Configuration

```toml
[audit]
enabled = true
output_dir = "/var/log/claude-pretool-sidecar"
# Or specific file: output = "/var/log/audit.jsonl" (no rotation)

# Size management
max_total_bytes = 10485760   # 10 MB total across all log files (default)
max_file_bytes = 5242880     # 5 MB per individual file (default)
```

## Rotation Algorithm

On each write:

1. Determine today's log file: `audit-{YYYY-MM-DD}.jsonl`
2. Append the new entry
3. **Check current file size**:
   - If > `max_file_bytes`: truncate to last N lines (keep most recent)
4. **Check total directory size** (periodically, not every write):
   - Sum sizes of all `audit-*.jsonl` files
   - If > `max_total_bytes`: delete oldest files until under limit
   - If only one file remains and it's still over limit: truncate to recent lines

## Truncation Strategy

When truncating a file to fit within size limits:

1. Read the file
2. Keep only the last N lines such that the file is within `max_file_bytes`
3. Write a sentinel line at the top: `{"_truncated": true, "timestamp": ..., "lines_removed": N}`
4. Write the retained recent lines

This ensures:
- Most recent data is always preserved
- Users can see that truncation occurred
- Approximate line count of removed data is available

## Size Check Frequency

To avoid filesystem overhead on every write, we check total size only:
- Every 100 writes (counter in memory)
- On startup
- When explicitly requested (`--cleanup-logs`)

## Environment Variable Override

`CPTS_MAX_LOG_BYTES` overrides `max_total_bytes` for quick adjustment.
