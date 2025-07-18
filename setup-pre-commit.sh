#!/bin/bash
# Script to set up pre-commit hooks for the project

set -e

echo "🔧 Setting up pre-commit hooks..."

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    echo "📦 Installing pre-commit..."

    # Try different installation methods
    if command -v pip &> /dev/null; then
        echo "Using pip to install pre-commit..."
        pip install pre-commit
    elif command -v pipx &> /dev/null; then
        echo "Using pipx to install pre-commit..."
        pipx install pre-commit
    elif command -v brew &> /dev/null; then
        echo "Using homebrew to install pre-commit..."
        brew install pre-commit
    else
        echo "❌ Could not find pip, pipx, or brew to install pre-commit"
        echo "Please install pre-commit manually: https://pre-commit.com/#install"
        exit 1
    fi
fi

# Install the git hook scripts
echo "🔗 Installing git hooks..."
pre-commit install

# Install hooks for different stages
pre-commit install --hook-type pre-push

# Run against all files to check current state
echo "🏃 Running pre-commit on all files (this may take a while)..."
pre-commit run --all-files || true

echo ""
echo "✅ Pre-commit is now set up!"
echo ""
echo "The following hooks will run:"
echo "  - On commit: formatting, linting, compilation, quick tests"
echo "  - On push: full test suite, security audit"
echo ""
echo "To run manually:"
echo "  pre-commit run --all-files    # Run on all files"
echo "  pre-commit run               # Run on staged files"
echo ""
echo "To skip hooks (not recommended):"
echo "  git commit --no-verify"
echo "  git push --no-verify"
