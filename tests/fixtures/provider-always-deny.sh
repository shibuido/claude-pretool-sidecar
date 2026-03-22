#!/bin/sh
# Mock provider that always denies. Reads stdin (discards), outputs deny.
cat > /dev/null
echo '{"decision": "deny", "reason": "denied by test provider"}'
