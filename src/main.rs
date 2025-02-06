use clap::Parser;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Retrieve source code context for a given file using Bazel.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The source file to analyze.
    source_file: String,

    /// The maximum number of lines to print.
    #[arg(long, short, default_value_t = usize::MAX)]
    limit: usize,
}

/// Runs an external command and returns its stdout as a String.
fn run_command(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", command, e))?;
    if !output.status.success() {
        return Err(format!(
            "Command {} failed: {}",
            command,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// For a given source file, runs:
///     bazel query '<source_file>' --output=package
/// and returns the Bazel package (trimmed).
fn get_bazel_package(source_file: &str) -> Result<String, String> {
    let output = run_command("bazel", &["query", source_file, "--output=package"])?;
    Ok(output.trim().to_string())
}

/// Runs a query to get the source files within the given Bazel package:
///
///   bazel query 'kind("source file", {pkg})' --output=location
fn get_source_files_in_package(bazel_pkg: &str) -> Result<Vec<String>, String> {
    let query = format!(r#"kind("source file", {pkg})"#, pkg = bazel_pkg);
    let output = run_command("bazel", &["query", &query, "--output=location"])?;
    parse_bazel_output(output)
}

/// Runs a dependency query using the given Bazel package:
///
///   bazel query 'kind("source file", deps({pkg}, 2))' --output=location
///
/// It then parses the output to extract the file paths.
fn get_dependent_source_files(bazel_pkg: &str) -> Result<Vec<String>, String> {
    let query = format!(r#"kind("source file", deps({pkg}, 2))"#, pkg = bazel_pkg);
    let output = run_command("bazel", &["query", &query, "--output=location"])?;
    parse_bazel_output(output)
}

/// Parses the output from a bazel query --output=location command.
fn parse_bazel_output(output: String) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for line in output.lines() {
        if line.starts_with('/') {
            if let Some(file_path) = line.split(':').next() {
                files.push(file_path.to_string());
            }
        }
    }
    Ok(files)
}

/// Counts the number of lines in a file.
fn count_lines(file_path: &str) -> Result<usize, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
    Ok(content.lines().count())
}

/// Prints a header and the full content of a file, or up to the line limit.
fn print_file_with_content(file_path: &str, line_limit: usize, lines_printed: usize) -> usize {
    if lines_printed >= line_limit {
        return 0; // Return 0 lines added
    }

    // Check if the file exists *before* trying to read it.
    if !Path::new(file_path).exists() {
        eprintln!("Warning: File {} does not exist.", file_path);
        return 0;
    }

    match count_lines(file_path) {
        Ok(file_lines) => {
            let remaining_lines = line_limit - lines_printed;
            if remaining_lines >= file_lines {
                // Print the entire file
                println!("==> {} <==", file_path);
                match fs::read_to_string(file_path) {
                    Ok(content) => {
                        print!("{}", content);
                        file_lines // Return number of lines printed
                    }
                    Err(e) => {
                        eprintln!("Error reading file {}: {}", file_path, e);
                        0 // Failed to read, so 0 lines printed.
                    }
                }
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("{}", e); // Print error from count_lines
            0
        }
    }
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let source_file = &args.source_file;
    let line_limit = args.limit;

    // Determine the Bazel package for the given source file.
    let bazel_pkg = get_bazel_package(source_file)?;

    // Get the source files directly in the package.
    let package_files = get_source_files_in_package(&bazel_pkg)?;

    // Get the dependent source files.
    let dependent_files = get_dependent_source_files(&bazel_pkg)?;

    let mut lines_printed = 0;

    // Print files in the main package first.
    for file in package_files.iter() {
        lines_printed += print_file_with_content(file, line_limit, lines_printed);
        if lines_printed >= line_limit {
            break; // Stop if limit is reached
        }
    }

    // Then print dependent files, skipping duplicates.
    for file in dependent_files {
        if !package_files.contains(&file) {
            lines_printed += print_file_with_content(&file, line_limit, lines_printed);
            if lines_printed >= line_limit {
                break; // Stop if limit is reached
            }
        }
    }

    Ok(())
}
