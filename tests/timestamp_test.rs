use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_timestamp_format() {
        // Create a simple command that echoes a test string
        let mut cmd = Command::new("target/debug/qrun")
                .arg("echo")
                .arg("Test output")
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start qrun");

        // Get the output
        let output = cmd.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(output);
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap();

        // There should be exactly one line of output
        assert_eq!(lines.len(), 1, "Expected exactly one line of output");

        // The line should start with a timestamp in the format [timestamp]
        let line = &lines[0];
        let timestamp_regex = regex::Regex::new(r"^\[\d+\] Test output$").unwrap();
        assert!(
                timestamp_regex.is_match(line),
                "Output does not match expected format: {}",
                line
        );

        // Extract the timestamp and verify it's a valid Unix timestamp
        let timestamp_str = line
                .trim_start_matches('[')
                .split(']')
                .next()
                .expect("Failed to extract timestamp");

        let timestamp: u64 = timestamp_str
                .parse()
                .expect("Failed to parse timestamp as u64");

        // Get the current timestamp
        let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs();

        // The timestamp should be close to the current time (within 10 seconds)
        assert!(
                now.abs_diff(timestamp) < 10,
                "Timestamp {} is not close to current time {}",
                timestamp,
                now
        );
}

#[test]
fn test_timestamp_added_to_stdin() {
        // Create a command that will read from stdin and echo to stdout
        let mut cmd = Command::new("target/debug/qrun")
                .arg("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to start qrun");

        // Write to stdin
        let test_input = "Test input";
        {
                let mut stdin = cmd.stdin.take().expect("Failed to open stdin");
                stdin
                        .write_all(test_input.as_bytes())
                        .expect("Failed to write to stdin");
                // stdin is closed when it goes out of scope
        }

        // Get the output
        let output = cmd.stdout.take().expect("Failed to open stdout");
        let reader = BufReader::new(output);
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>().unwrap();

        // There should be exactly one line of output
        assert_eq!(lines.len(), 1, "Expected exactly one line of output");

        // The line should have timestamps in the format [timestamp] [timestamp] Test input
        // This is because qrun adds a timestamp to stdin and then cat outputs that line,
        // which qrun then adds another timestamp to
        let line = &lines[0];
        let timestamp_regex = regex::Regex::new(r"^\[\d+\] \[\d+\] Test input$").unwrap();
        assert!(
                timestamp_regex.is_match(line),
                "Output does not match expected format: {}",
                line
        );
}

#[test]
fn test_timestamp_added_to_stderr() {
        // Create a command that will output to stderr
        let output = Command::new("target/debug/qrun")
                .arg("sh")
                .arg("-c")
                .arg("echo 'Error message' >&2")
                .stderr(Stdio::piped())
                .output()
                .expect("Failed to start qrun");

        // Convert stderr to string
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = stderr.lines().collect();

        // There should be exactly one line of output
        assert_eq!(lines.len(), 1, "Expected exactly one line of stderr output");

        // The line should start with a timestamp in the format [timestamp]
        let line = lines[0];
        let timestamp_regex = regex::Regex::new(r"^\[\d+\] Error message$").unwrap();
        assert!(
                timestamp_regex.is_match(line),
                "Stderr output does not match expected format: {}",
                line
        );
}

#[test]
fn test_timestamp_added_to_multiline_output() {
        // Create a command that will output multiple lines
        let output = Command::new("target/debug/qrun")
                .arg("printf")
                .arg("Line 1\nLine 2\nLine 3\n")
                .stdout(Stdio::piped())
                .output()
                .expect("Failed to start qrun");

        // Convert stdout to string
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().collect();

        // There should be exactly three lines of output
        assert_eq!(lines.len(), 3, "Expected exactly three lines of output");

        // Each line should start with a timestamp in the format [timestamp]
        let timestamp_regex = regex::Regex::new(r"^\[\d+\] Line \d$").unwrap();

        for (i, line) in lines.iter().enumerate() {
                let expected_line_num = i + 1;
                assert!(
                        timestamp_regex.is_match(line),
                        "Line {} does not match expected format: {}",
                        expected_line_num,
                        line
                );

                // Verify the line number in the content matches the expected line number
                let content = line.split("] ").nth(1).unwrap();
                assert_eq!(
                        content,
                        format!("Line {}", expected_line_num),
                        "Line content does not match expected: {}",
                        content
                );
        }
}
