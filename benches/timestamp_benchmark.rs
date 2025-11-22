use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

fn benchmark_direct_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("Command Execution");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark for small output (single line)
    group.bench_function("direct_command_small", |b| {
        b.iter(|| {
            let output = Command::new("echo")
                .arg("Hello, World!")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    // Benchmark for medium output (100 lines)
    group.bench_function("direct_command_medium", |b| {
        b.iter(|| {
            let output = Command::new("bash")
                .arg("-c")
                .arg("for i in {1..100}; do echo \"Line $i\"; done")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    // Benchmark for large output (1000 lines)
    group.bench_function("direct_command_large", |b| {
        b.iter(|| {
            let output = Command::new("bash")
                .arg("-c")
                .arg("for i in {1..1000}; do echo \"Line $i\"; done")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    group.finish();
}

fn benchmark_qrun_command(c: &mut Criterion) {
    let mut group = c.benchmark_group("Qrun Command Execution");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark for small output (single line)
    group.bench_function("qrun_command_small", |b| {
        b.iter(|| {
            let output = Command::new("target/release/qrun")
                .arg("echo")
                .arg("Hello, World!")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    // Benchmark for medium output (100 lines)
    group.bench_function("qrun_command_medium", |b| {
        b.iter(|| {
            let output = Command::new("target/release/qrun")
                .arg("bash")
                .arg("-c")
                .arg("for i in {1..100}; do echo \"Line $i\"; done")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    // Benchmark for large output (1000 lines)
    group.bench_function("qrun_command_large", |b| {
        b.iter(|| {
            let output = Command::new("target/release/qrun")
                .arg("bash")
                .arg("-c")
                .arg("for i in {1..1000}; do echo \"Line $i\"; done")
                .output()
                .expect("Failed to execute command");
            black_box(output);
        });
    });

    group.finish();
}

fn benchmark_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("Command Comparison");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    for size in ["small", "medium", "large"].iter() {
        let cmd_args = match *size {
            "small" => vec!["echo", "Hello, World!"],
            "medium" => vec!["bash", "-c", "for i in {1..100}; do echo \"Line $i\"; done"],
            "large" => vec!["bash", "-c", "for i in {1..1000}; do echo \"Line $i\"; done"],
            _ => unreachable!(),
        };

        group.bench_with_input(BenchmarkId::new("direct", size), &cmd_args, |b, args| {
            b.iter(|| {
                let mut cmd = Command::new(args[0]);
                for arg in &args[1..] {
                    cmd.arg(arg);
                }
                let output = cmd.output().expect("Failed to execute command");
                black_box(output);
            });
        });

        group.bench_with_input(BenchmarkId::new("qrun", size), &cmd_args, |b, args| {
            b.iter(|| {
                let mut cmd = Command::new("target/release/qrun");
                for arg in args.iter() {
                    cmd.arg(arg);
                }
                let output = cmd.output().expect("Failed to execute command");
                black_box(output);
            });
        });
    }

    group.finish();
}

fn benchmark_stdin_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("Stdin Processing");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark for small input (single line)
    group.bench_function("direct_stdin_small", |b| {
        b.iter(|| {
            let mut child = Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to spawn command");
            
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            stdin.write_all(b"Hello, World!\n").expect("Failed to write to stdin");
            drop(stdin);
            
            let output = child.wait_with_output().expect("Failed to wait for command");
            black_box(output);
        });
    });

    // Benchmark for small input with qrun (single line)
    group.bench_function("qrun_stdin_small", |b| {
        b.iter(|| {
            let mut child = Command::new("target/release/qrun")
                .arg("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to spawn command");
            
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            stdin.write_all(b"Hello, World!\n").expect("Failed to write to stdin");
            drop(stdin);
            
            let output = child.wait_with_output().expect("Failed to wait for command");
            black_box(output);
        });
    });

    // Benchmark for medium input (100 lines)
    group.bench_function("direct_stdin_medium", |b| {
        b.iter(|| {
            let mut child = Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to spawn command");
            
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            for i in 1..=100 {
                writeln!(stdin, "Line {}", i).expect("Failed to write to stdin");
            }
            drop(stdin);
            
            let output = child.wait_with_output().expect("Failed to wait for command");
            black_box(output);
        });
    });

    // Benchmark for medium input with qrun (100 lines)
    group.bench_function("qrun_stdin_medium", |b| {
        b.iter(|| {
            let mut child = Command::new("target/release/qrun")
                .arg("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to spawn command");
            
            let mut stdin = child.stdin.take().expect("Failed to open stdin");
            for i in 1..=100 {
                writeln!(stdin, "Line {}", i).expect("Failed to write to stdin");
            }
            drop(stdin);
            
            let output = child.wait_with_output().expect("Failed to wait for command");
            black_box(output);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_direct_command,
    benchmark_qrun_command,
    benchmark_comparison,
    benchmark_stdin_processing
);
criterion_main!(benches);