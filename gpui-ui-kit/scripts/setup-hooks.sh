#!/bin/bash
# Setup git hooks for gpui-ui-kit

set -e

echo "🔧 Setting up git hooks for gpui-ui-kit..."

# Get repository root
REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/gpui-ui-kit/.githooks"
GIT_HOOKS_DIR="$REPO_ROOT/.git/modules/gpui-ui-kit/hooks"

# Check if we're in the right directory
if [ ! -d "$HOOKS_DIR" ]; then
    echo "❌ Error: Could not find hooks directory at $HOOKS_DIR"
    exit 1
fi

# Create git hooks directory if it doesn't exist
mkdir -p "$GIT_HOOKS_DIR"

# Install pre-commit hook
if [ -f "$HOOKS_DIR/pre-commit" ]; then
    cp "$HOOKS_DIR/pre-commit" "$GIT_HOOKS_DIR/pre-commit"
    chmod +x "$GIT_HOOKS_DIR/pre-commit"
    echo "✅ Installed pre-commit hook"
else
    echo "⚠️  Warning: pre-commit hook not found"
fi

# Alternative: Configure git to use .githooks directory
# This works if gpui-ui-kit is the main repo (not a submodule)
if [ -d "$REPO_ROOT/gpui-ui-kit/.git" ]; then
    cd "$REPO_ROOT/gpui-ui-kit"
    git config core.hooksPath .githooks
    echo "✅ Configured git to use .githooks directory"
fi

echo ""
echo "✅ Git hooks setup complete!"
echo ""
echo "The pre-commit hook will now:"
echo "  • Check code formatting (cargo fmt)"
echo "  • Run all tests (cargo test --lib --tests)"
echo ""
echo "To bypass hooks temporarily, use: git commit --no-verify"
