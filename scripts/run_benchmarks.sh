#!/bin/bash

# Build the release version for benchmarking
cargo build --release

# Run the benchmarks
cargo bench

# Generate a summary report
echo "Benchmark Summary Report" > benchmark_summary.md
echo "=======================" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "## Overview" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "This report compares the performance of running commands directly versus using qrun with timestamp functionality." >> benchmark_summary.md
echo "" >> benchmark_summary.md

# Extract and format the benchmark results
echo "## Results" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "### Command Execution Comparison" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "| Test Case | Direct Command | Qrun Command | Overhead |" >> benchmark_summary.md
echo "|-----------|---------------|--------------|----------|" >> benchmark_summary.md

# Parse the benchmark results
BENCH_DIR="target/criterion/Command Comparison"
for size in small medium large; do
    direct_time=$(grep -A 3 "time:" "$BENCH_DIR/direct/$size/new/estimates.json" | grep "mean" | awk '{print $2}' | tr -d ',')
    qrun_time=$(grep -A 3 "time:" "$BENCH_DIR/qrun/$size/new/estimates.json" | grep "mean" | awk '{print $2}' | tr -d ',')
    
    # Calculate overhead percentage
    overhead=$(echo "scale=2; ($qrun_time - $direct_time) / $direct_time * 100" | bc)
    
    # Convert to milliseconds for better readability
    direct_ms=$(echo "scale=3; $direct_time * 1000" | bc)
    qrun_ms=$(echo "scale=3; $qrun_time * 1000" | bc)
    
    echo "| $size | ${direct_ms}ms | ${qrun_ms}ms | ${overhead}% |" >> benchmark_summary.md
done

echo "" >> benchmark_summary.md
echo "### Stdin Processing Comparison" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "| Test Case | Direct Command | Qrun Command | Overhead |" >> benchmark_summary.md
echo "|-----------|---------------|--------------|----------|" >> benchmark_summary.md

# Parse the stdin benchmark results
BENCH_DIR="target/criterion/Stdin Processing"
for size in small medium; do
    direct_time=$(grep -A 3 "time:" "$BENCH_DIR/direct_stdin_$size/new/estimates.json" | grep "mean" | awk '{print $2}' | tr -d ',')
    qrun_time=$(grep -A 3 "time:" "$BENCH_DIR/qrun_stdin_$size/new/estimates.json" | grep "mean" | awk '{print $2}' | tr -d ',')
    
    # Calculate overhead percentage
    overhead=$(echo "scale=2; ($qrun_time - $direct_time) / $direct_time * 100" | bc)
    
    # Convert to milliseconds for better readability
    direct_ms=$(echo "scale=3; $direct_time * 1000" | bc)
    qrun_ms=$(echo "scale=3; $qrun_time * 1000" | bc)
    
    echo "| $size | ${direct_ms}ms | ${qrun_ms}ms | ${overhead}% |" >> benchmark_summary.md
done

echo "" >> benchmark_summary.md
echo "## Conclusion" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "The benchmark results show the performance overhead introduced by the timestamp functionality in qrun." >> benchmark_summary.md
echo "This overhead includes:" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "1. Forking process overhead" >> benchmark_summary.md
echo "2. Timestamp generation and formatting" >> benchmark_summary.md
echo "3. Stream processing and line buffering" >> benchmark_summary.md
echo "" >> benchmark_summary.md
echo "For most interactive use cases, this overhead should be negligible compared to the benefits of having timestamped output." >> benchmark_summary.md

echo "Benchmark summary report generated: benchmark_summary.md"