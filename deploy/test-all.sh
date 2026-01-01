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

# Master test runner for all deployment configurations

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results tracking
declare -a PASSED_TESTS
declare -a FAILED_TESTS

log_header() {
    echo
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
}

log_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ $2 PASSED${NC}"
        PASSED_TESTS+=("$2")
    else
        echo -e "${RED}✗ $2 FAILED${NC}"
        FAILED_TESTS+=("$2")
    fi
}

# Function to run a single deployment test
run_deployment_test() {
    local test_name=$1
    local test_dir=$2

    log_header "Testing: $test_name"

    if [ ! -f "$test_dir/test.sh" ]; then
        echo -e "${YELLOW}⚠ No test script found for $test_name (skipping)${NC}"
        return 0
    fi

    # Make test script executable
    chmod +x "$test_dir/test.sh"

    # Run the test in a subshell to isolate environment
    (
        cd "$test_dir"
        bash test.sh
    )
    local result=$?

    log_result $result "$test_name"
    return $result
}

# Main test execution
main() {
    log_header "mooR Deployment Test Suite"
    echo "Testing all deployment configurations..."
    echo

    # List of deployments to test
    declare -a DEPLOYMENTS=(
        "telnet-only"
        "web-basic"
        "web-ssl"
    )

    # Add debian-packages if prerequisites are available
    if command -v cargo-deb &> /dev/null && command -v incus &> /dev/null; then
        DEPLOYMENTS+=("debian-packages")
    else
        echo -e "${YELLOW}⚠ Skipping debian-packages test (requires cargo-deb and incus)${NC}"
        echo
    fi

    # Add kubernetes if prerequisites are available
    if command -v kind &> /dev/null && command -v kubectl &> /dev/null; then
        DEPLOYMENTS+=("kubernetes")
    else
        echo -e "${YELLOW}⚠ Skipping kubernetes test (requires kind and kubectl)${NC}"
        echo
    fi

    # Run each deployment test
    for deployment in "${DEPLOYMENTS[@]}"; do
        run_deployment_test "$deployment" "$SCRIPT_DIR/$deployment" || true
        echo
    done

    # Print summary
    log_header "Test Summary"
    echo "Total tests run: $((${#PASSED_TESTS[@]} + ${#FAILED_TESTS[@]}))"
    echo -e "${GREEN}Passed: ${#PASSED_TESTS[@]}${NC}"

    if [ ${#PASSED_TESTS[@]} -gt 0 ]; then
        for test in "${PASSED_TESTS[@]}"; do
            echo -e "  ${GREEN}✓${NC} $test"
        done
    fi

    if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
        echo -e "${RED}Failed: ${#FAILED_TESTS[@]}${NC}"
        for test in "${FAILED_TESTS[@]}"; do
            echo -e "  ${RED}✗${NC} $test"
        done
        echo
        echo -e "${RED}Some tests failed. See output above for details.${NC}"
        exit 1
    else
        echo
        echo -e "${GREEN}All tests passed! 🎉${NC}"
        exit 0
    fi
}

# Check prerequisites
check_prerequisites() {
    local missing=0

    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Error: docker is not installed${NC}"
        missing=1
    fi

    if ! command -v docker compose &> /dev/null; then
        echo -e "${RED}Error: docker compose is not installed${NC}"
        missing=1
    fi

    if ! command -v nc &> /dev/null; then
        echo -e "${YELLOW}Warning: netcat (nc) is not installed - some tests may be limited${NC}"
    fi

    if ! command -v telnet &> /dev/null; then
        echo -e "${YELLOW}Warning: telnet is not installed - telnet tests may fail${NC}"
    fi

    if ! command -v curl &> /dev/null; then
        echo -e "${RED}Error: curl is not installed${NC}"
        missing=1
    fi

    if ! command -v jq &> /dev/null; then
        echo -e "${YELLOW}Warning: jq is not installed - some service health checks may be limited${NC}"
    fi

    if [ $missing -eq 1 ]; then
        exit 1
    fi
}

# Run prerequisite check
check_prerequisites

# Run main test suite
main "$@"
