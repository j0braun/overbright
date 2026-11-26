#!/bin/bash
# studio_checks.sh [repo] [part]: the Studio preview's budget, lint and registry-coverage checks (studio_checks.py).
exec python3 -I "$(dirname "$0")/studio_checks.py" "$@"
