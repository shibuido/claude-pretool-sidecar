#!/bin/sh
# Mock provider that always allows. Reads stdin (discards), outputs allow.
cat > /dev/null
echo '{"decision": "allow"}'
