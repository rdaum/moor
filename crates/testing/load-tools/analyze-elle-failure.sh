#!/bin/bash
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

# Script to analyze Elle consistency check failures

WORKLOAD_FILE="${1:-/output/workload.edn}"
RESULT_FILE="${2:-/output/elle-result.txt}"

echo "════════════════════════════════════════════════════════════"
echo "Elle Consistency Analysis"
echo "════════════════════════════════════════════════════════════"
echo ""

# Check if workload exists
if [ ! -f "$WORKLOAD_FILE" ]; then
    echo "ERROR: Workload file not found: $WORKLOAD_FILE"
    exit 1
fi

# Basic stats
TOTAL_OPS=$(grep -c "^{" "$WORKLOAD_FILE")
INVOKE_OPS=$(grep -c ":type :invoke" "$WORKLOAD_FILE")
OK_OPS=$(grep -c ":type :ok" "$WORKLOAD_FILE")

echo "Workload Statistics:"
echo "  Total operations: $TOTAL_OPS"
echo "  Invoke operations: $INVOKE_OPS"
echo "  OK operations: $OK_OPS"
echo ""

# Run Elle with detailed output including cycle graphs
echo "Running Elle analysis..."
echo ""

# Create directory for Elle output artifacts (cycle graphs, etc.)
mkdir -p /tmp/elle-output

java -jar /opt/elle-cli/target/elle-cli-0.1.9-standalone.jar \
    --model list-append \
    --directory /tmp/elle-output \
    --plot-format svg \
    --cycle-search-timeout 5000 \
    "$WORKLOAD_FILE" 2>&1 | tee /tmp/elle-verbose.txt

echo ""
echo "════════════════════════════════════════════════════════════"

# Check result
if grep -q "true" /tmp/elle-verbose.txt; then
    echo "✓ No anomalies detected"
    exit 0
else
    echo "✗ Anomalies detected!"
    echo ""

    # Try to extract cycle information
    echo "Searching for cycle information in output..."
    if grep -i "cycle\|anomal\|G[0-2]" /tmp/elle-verbose.txt > /tmp/anomaly-info.txt; then
        echo ""
        echo "Anomaly details:"
        cat /tmp/anomaly-info.txt
    fi

    # Extract some transactions from the workload for inspection
    echo ""
    echo "Sample transactions (first 10):"
    head -n 10 "$WORKLOAD_FILE"

    echo ""
    echo "Sample transactions (last 10):"
    tail -n 10 "$WORKLOAD_FILE"

    # List any cycle graphs that were generated
    echo ""
    if ls /tmp/elle-output/*.svg 2>/dev/null | head -5; then
        echo ""
        echo "Cycle graphs generated (showing first 5):"
        ls -lh /tmp/elle-output/*.svg | head -5
    fi

    echo ""
    echo "════════════════════════════════════════════════════════════"
    echo "Files available for inspection:"
    echo "  - Workload: $WORKLOAD_FILE"
    echo "  - Elle output: /tmp/elle-verbose.txt"
    echo "  - Anomaly info: /tmp/anomaly-info.txt"
    echo "  - Cycle graphs: /tmp/elle-output/*.svg"
    echo ""
    echo "To extract files from container:"
    echo "  docker cp <container>:$WORKLOAD_FILE ."
    echo "  docker cp <container>:/tmp/elle-verbose.txt ."
    echo "  docker cp <container>:/tmp/elle-output ."
    echo "════════════════════════════════════════════════════════════"

    exit 1
fi
