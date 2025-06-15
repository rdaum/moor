#!/bin/bash

# Build a single PDF from the book's Markdown files.

set -euo pipefail

check_dep() {
    dep="$1"; shift
    if ! command -v "$dep" >/dev/null 2>&1; then
        echo >&2 "Error: $dep is not installed. Please install it to build the PDF."
        exit 1
    fi
}

check_dep pandoc

# Common pandoc options
export MDBOOK_output__pandoc__hosted_html=https://rdaum.github.io/moor/
# Disable pagetoc
export MDBOOK_preprocessor__pagetoc__renderers=[]
# Disable our custom theme files; the `pagetoc`-generated `@media` rules confuse mdbook-pandoc
export MDBOOK_output__html__additional_js=[]
export MDBOOK_output__html__additional_css=[]

# Configure enabled backends
got_backend=no
while [[ $# -gt 0 ]]; do
    case "$1" in
        --html)
            export MDBOOK_output__pandoc__profile__html__output_file=moor.html
            export MDBOOK_output__pandoc__profile__html__to=html
            got_backend=yes
            shift
            ;;
        --pdf)
            check_dep pdflatex
            export MDBOOK_output__pandoc__profile__pdf__output_file=moor.pdf
            export MDBOOK_output__pandoc__profile__pdf__to=pdf
            got_backend=yes
            shift
            ;;
        *)
            echo >&2 "Unknown option: $1. Try --html and/or --pdf."
            exit 1
            ;;
    esac
done

if [[ "$got_backend" == "no" ]]; then
    echo "Error: No output backend specified. Use --html and/or --pdf."
    exit 1
fi

# Install dependencies
./install-tools.sh
cargo install --vers "^0.10" mdbook-pandoc

# Do the thing
mdbook build
