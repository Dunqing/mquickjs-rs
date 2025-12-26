#!/bin/bash
# Benchmark comparison script for MQuickJS-RS vs original MQuickJS
#
# Usage:
#   ./benches/compare.sh [path-to-original-mqjs]
#
# If no path is provided, only runs Rust benchmarks.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BENCH_SCRIPTS="$SCRIPT_DIR/scripts"
ORIGINAL_MQJS="${1:-}"

echo "=== MQuickJS Benchmark Comparison ==="
echo ""

# Build Rust version
echo "Building mquickjs-rs (release)..."
cd "$PROJECT_DIR"
cargo build --release --quiet
RUST_MQJS="$PROJECT_DIR/target/release/mqjs"

if [ ! -f "$RUST_MQJS" ]; then
    echo "Error: Failed to build mquickjs-rs"
    exit 1
fi

echo "Built: $RUST_MQJS"
echo ""

# Function to run a benchmark
run_bench() {
    local script="$1"
    local binary="$2"
    local runs=5
    local total=0

    for i in $(seq 1 $runs); do
        local start=$(python3 -c 'import time; print(time.time())')
        "$binary" "$script" > /dev/null 2>&1
        local end=$(python3 -c 'import time; print(time.time())')
        local elapsed=$(python3 -c "print($end - $start)")
        total=$(python3 -c "print($total + $elapsed)")
    done

    python3 -c "print(f'{$total / $runs:.4f}')"
}

# Run benchmarks
echo "Running benchmarks (5 runs each, showing average)..."
echo ""
echo "Benchmark               Rust (s)    C (s)      Ratio"
echo "-----------------------------------------------------"

for script in "$BENCH_SCRIPTS"/*.js; do
    name=$(basename "$script" .js)

    # Rust version
    rust_time=$(run_bench "$script" "$RUST_MQJS")

    if [ -n "$ORIGINAL_MQJS" ] && [ -x "$ORIGINAL_MQJS" ]; then
        # Original version
        orig_time=$(run_bench "$script" "$ORIGINAL_MQJS")
        ratio=$(python3 -c "print(f'{$rust_time / $orig_time:.2f}x' if $orig_time > 0 else 'N/A')")
        printf "%-20s %10s %10s %10s\n" "$name" "$rust_time" "$orig_time" "$ratio"
    else
        printf "%-20s %10s %10s %10s\n" "$name" "$rust_time" "N/A" "N/A"
    fi
done

echo ""
if [ -z "$ORIGINAL_MQJS" ]; then
    echo "Note: Pass path to original mqjs binary for comparison"
    echo "  Usage: $0 /path/to/mqjs"
fi

echo ""
echo "For detailed Rust benchmarks, run:"
echo "  cargo bench"
