#!/bin/bash
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

# Wrapper script to run Elle consistency test and extract results

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
OUTPUT_DIR="${OUTPUT_DIR:-$WORKSPACE_ROOT/elle-results}"

echo "════════════════════════════════════════════════════════════"
echo "Elle Consistency Test Runner"
echo "════════════════════════════════════════════════════════════"
echo ""

# Parse arguments for test parameters
NUM_PROPS="${NUM_PROPS:-3}"
NUM_CONCURRENT="${NUM_CONCURRENT:-20}"
NUM_ITERATIONS="${NUM_ITERATIONS:-1000}"

while [[ $# -gt 0 ]]; do
    case $1 in
        --num-props)
            NUM_PROPS="$2"
            shift 2
            ;;
        --num-concurrent)
            NUM_CONCURRENT="$2"
            shift 2
            ;;
        --num-iterations)
            NUM_ITERATIONS="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--num-props N] [--num-concurrent N] [--num-iterations N] [--output-dir DIR]"
            exit 1
            ;;
    esac
done

# Create output directory with timestamp
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RUN_OUTPUT_DIR="$OUTPUT_DIR/run-$TIMESTAMP"
mkdir -p "$RUN_OUTPUT_DIR"

echo "Test Configuration:"
echo "  Properties: $NUM_PROPS"
echo "  Concurrent workloads: $NUM_CONCURRENT"
echo "  Iterations per workload: $NUM_ITERATIONS"
echo "  Output directory: $RUN_OUTPUT_DIR"
echo ""

# Build the Docker image if needed
echo "Building Docker image..."
cd "$WORKSPACE_ROOT"
docker build -f Dockerfile.elle -t moor-elle-test . || {
    echo "ERROR: Docker build failed"
    exit 1
}

echo ""
echo "Running Elle consistency test..."
echo "════════════════════════════════════════════════════════════"
echo ""

# Run the container with a unique name
CONTAINER_NAME="elle-test-$TIMESTAMP"

# Run and capture exit code
if docker run --name "$CONTAINER_NAME" \
    -e NUM_PROPS="$NUM_PROPS" \
    -e NUM_CONCURRENT="$NUM_CONCURRENT" \
    -e NUM_ITERATIONS="$NUM_ITERATIONS" \
    moor-elle-test; then
    TEST_RESULT="PASSED"
    EXIT_CODE=0
else
    TEST_RESULT="FAILED"
    EXIT_CODE=1
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo "Extracting results..."

# Extract all result files
docker cp "$CONTAINER_NAME:/output/workload.edn" "$RUN_OUTPUT_DIR/workload.edn" 2>/dev/null || echo "Warning: Could not extract workload.edn"
docker cp "$CONTAINER_NAME:/tmp/elle-verbose.txt" "$RUN_OUTPUT_DIR/elle-output.txt" 2>/dev/null || echo "Warning: Could not extract elle-output.txt"
docker cp "$CONTAINER_NAME:/tmp/anomaly-info.txt" "$RUN_OUTPUT_DIR/anomaly-info.txt" 2>/dev/null || echo "Note: No anomaly info file (test may have passed)"
docker cp "$CONTAINER_NAME:/tmp/elle-output" "$RUN_OUTPUT_DIR/" 2>/dev/null && echo "Extracted cycle graphs" || echo "Note: No cycle graphs generated (test may have passed)"

# Clean up container
docker rm "$CONTAINER_NAME" > /dev/null

# Create a summary file
cat > "$RUN_OUTPUT_DIR/summary.txt" <<EOF
Elle Consistency Test Results
==============================

Timestamp: $TIMESTAMP
Result: $TEST_RESULT

Configuration:
  Properties: $NUM_PROPS
  Concurrent workloads: $NUM_CONCURRENT
  Iterations per workload: $NUM_ITERATIONS

Files:
  - workload.edn: Full transaction history
  - elle-output.txt: Elle analysis output
  - anomaly-info.txt: Extracted anomaly information (if any)
  - summary.txt: This file

EOF

# If test failed, add anomaly details to summary
if [ $EXIT_CODE -ne 0 ] && [ -f "$RUN_OUTPUT_DIR/anomaly-info.txt" ]; then
    echo "Anomaly Details:" >> "$RUN_OUTPUT_DIR/summary.txt"
    echo "----------------" >> "$RUN_OUTPUT_DIR/summary.txt"
    cat "$RUN_OUTPUT_DIR/anomaly-info.txt" >> "$RUN_OUTPUT_DIR/summary.txt"
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo "Test Result: $TEST_RESULT"
echo ""
echo "Results saved to: $RUN_OUTPUT_DIR"
echo ""
echo "Files:"
ls -lh "$RUN_OUTPUT_DIR"
echo ""

if [ $EXIT_CODE -ne 0 ]; then
    echo "════════════════════════════════════════════════════════════"
    echo "ANOMALY DETECTED! Summary:"
    echo "════════════════════════════════════════════════════════════"
    cat "$RUN_OUTPUT_DIR/summary.txt"
    echo ""
    echo "Review files in: $RUN_OUTPUT_DIR"
fi

exit $EXIT_CODE
