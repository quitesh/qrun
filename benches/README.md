# Qrun Benchmarks

This directory contains benchmarks for measuring the performance overhead of the timestamp functionality in qrun.

## Running the Benchmarks

To run the benchmarks, use the provided script:

```bash
./scripts/run_benchmarks.sh
```

This will:
1. Build qrun in release mode
2. Run all benchmarks
3. Generate a summary report in `benchmark_summary.md`

## Benchmark Categories

The benchmarks are divided into several categories:

1. **Direct Command Execution**: Measures the performance of running commands directly without qrun.
2. **Qrun Command Execution**: Measures the performance of running commands through qrun with timestamp functionality.
3. **Command Comparison**: Directly compares the performance of direct commands vs. qrun commands for different output sizes.
4. **Stdin Processing**: Compares the performance of stdin processing with and without qrun.

## Output Sizes

The benchmarks test different output sizes:

- **Small**: Single line output (e.g., "Hello, World!")
- **Medium**: 100 lines of output
- **Large**: 1000 lines of output

## Interpreting Results

The benchmark results will show the time taken for each command execution and the overhead introduced by the timestamp functionality. The overhead is calculated as a percentage increase in execution time compared to running the command directly.

For most interactive use cases, this overhead should be negligible compared to the benefits of having timestamped output.