#!/bin/sh
# Mock provider that always passes through. Reads stdin (discards), outputs passthrough.
cat > /dev/null
echo '{"decision": "passthrough"}'
