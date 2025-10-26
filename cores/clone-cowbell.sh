#!/bin/bash
# Script to clone the cowbell repository locally
# This checks out https://github.com/rdaum/cowbell as a regular git clone, not as a submodule

set -e  # Exit on any error

COWBELL_DIR="cowbell"
REPO_URL="https://github.com/rdaum/cowbell.git"

# Check if cowbell directory already exists
if [ -d "$COWBELL_DIR" ]; then
    echo "Cowbell directory already exists at $COWBELL_DIR"
    echo "To update, run: cd $COWBELL_DIR && git pull"
    exit 0
fi

# Clone the repository
echo "Cloning cowbell repository from $REPO_URL..."
git clone "$REPO_URL" "$COWBELL_DIR"

echo "Cowbell repository cloned successfully to $COWBELL_DIR"
echo "To update later, run: cd $COWBELL_DIR && git pull"