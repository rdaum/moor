#!/bin/bash

# `mdbook` and its plugins expect to be installed to the system,
# using `cargo install` (or by downloading binaries).
# This script takes care of that; for use in both CI and local development.

set -euo pipefail

cargo install --vers "^0.4" mdbook
cargo install --vers "^0.7" mdbook-linkcheck
cargo install --vers "^0.2" mdbook-pagetoc
