#!/usr/bin/env bash
set -euo pipefail

# Readiness check script for shymini
# Runs formatting, tests, linting, and docker build checks

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() {
    echo -e "${GREEN}✓ $1${NC}"
}

fail() {
    echo -e "${RED}✗ $1${NC}"
    exit 1
}

info() {
    echo -e "${YELLOW}→ $1${NC}"
}

echo "================================"
echo "  shymini readiness check"
echo "================================"
echo

# 1. Format check
info "Checking code formatting..."
if cargo fmt --check; then
    pass "Code formatting OK"
else
    fail "Code formatting failed. Run 'cargo fmt' to fix."
fi
echo

# 2. Tests
info "Running tests..."
if cargo test; then
    pass "Tests passed"
else
    fail "Tests failed"
fi
echo

# 3. Clippy
info "Running clippy with all features..."
if cargo clippy --all-features --all-targets -- -D warnings; then
    pass "Clippy passed"
else
    fail "Clippy found issues"
fi
echo

# 4. Docker build
info "Building Docker image..."
if docker build -t shymini .; then
    pass "Docker build succeeded"
else
    fail "Docker build failed"
fi
echo

echo "================================"
echo -e "${GREEN}  All checks passed!${NC}"
echo "================================"
