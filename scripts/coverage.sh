#!/bin/bash
# Author: Luis Amaral
# Created: 2024-11-28
# Description: Run code coverage analysis using cargo-llvm-cov

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "cargo-llvm-cov not found. Installing..."
    # Use version compatible with Rust 1.75
    cargo install cargo-llvm-cov@0.6.15 --locked
fi

# Parse arguments
FORMAT="${1:-summary}"
OUTPUT_DIR="target/coverage"

case "$FORMAT" in
    summary)
        echo "=== Code Coverage Summary ==="
        cargo llvm-cov --summary-only
        ;;
    html)
        echo "=== Generating HTML Coverage Report ==="
        mkdir -p "$OUTPUT_DIR"
        cargo llvm-cov --html --output-dir "$OUTPUT_DIR"
        echo ""
        echo "Report generated at: $OUTPUT_DIR/html/index.html"
        echo "Opening in browser..."
        open "$OUTPUT_DIR/html/index.html" 2>/dev/null || echo "Run: open $OUTPUT_DIR/html/index.html"
        ;;
    lcov)
        echo "=== Generating LCOV Report ==="
        mkdir -p "$OUTPUT_DIR"
        cargo llvm-cov --lcov --output-path "$OUTPUT_DIR/lcov.info"
        echo "LCOV report: $OUTPUT_DIR/lcov.info"
        ;;
    json)
        echo "=== Generating JSON Report ==="
        mkdir -p "$OUTPUT_DIR"
        cargo llvm-cov --json --output-path "$OUTPUT_DIR/coverage.json"
        echo "JSON report: $OUTPUT_DIR/coverage.json"
        ;;
    *)
        echo "Usage: $0 [summary|html|lcov|json]"
        echo ""
        echo "  summary  - Print coverage summary to terminal (default)"
        echo "  html     - Generate HTML report and open in browser"
        echo "  lcov     - Generate LCOV format for CI integration"
        echo "  json     - Generate JSON format for programmatic use"
        exit 1
        ;;
esac
