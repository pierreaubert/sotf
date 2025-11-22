#!/bin/bash
#
# Run regression tests for audio recording with hardware loopback
#
# Usage:
#   ./run_regression_tests.sh [test_name]
#
# Examples:
#   ./run_regression_tests.sh                          # Run all tests
#   ./run_regression_tests.sh test_playback_valid_config
#   ./run_regression_tests.sh test_loopback_accuracy
#

set -e

# Configuration (can be overridden with environment variables)
export AEQ_E2E=${AEQ_E2E:-1}
export AEQ_E2E_SEND_CH=${AEQ_E2E_SEND_CH:-1}
export AEQ_E2E_RECORD_CH=${AEQ_E2E_RECORD_CH:-1}
export AEQ_E2E_SR=${AEQ_E2E_SR:-48000}

echo "========================================="
echo "Audio Regression Tests"
echo "========================================="
echo ""
echo "Configuration:"
echo "  Sample rate:    $AEQ_E2E_SR Hz"
echo "  Send channel:   $AEQ_E2E_SEND_CH"
echo "  Record channel: $AEQ_E2E_RECORD_CH"
echo ""
echo "Requirements:"
echo "  - Audio interface with loopback"
echo "  - CamillaDSP in PATH"
echo "  - Output channel $AEQ_E2E_SEND_CH connected to input channel $AEQ_E2E_RECORD_CH"
echo ""
echo "========================================="
echo ""

# Run tests
cd "$(dirname "$0")"

if [ -z "$1" ]; then
    echo "Running all regression tests..."
    echo ""
    cargo test --test regression_loopback -- --nocapture
else
    echo "Running specific test: $1"
    echo ""
    cargo test --test regression_loopback "$1" -- --nocapture
fi

echo ""
echo "========================================="
echo "Tests complete!"
echo ""
echo "Test outputs saved to:"
echo "  $(pwd)/target/regression-tests/"
echo "========================================="
