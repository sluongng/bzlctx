use std::env;
use std::fs;
use std::process::Command;

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

/// Runs a dependency query using the given Bazel package:
///
///   bazel query 'kind("source file", deps({pkg}, 2) + {pkg})' --output=location
///
/// It then parses the output to extract the file paths (the portion before the first colon).
fn get_related_source_files(bazel_pkg: &str) -> Result<Vec<String>, String> {
    // Build the query string by substituting the package value.
    let query = format!(
        r#"kind("source file", deps({pkg}, 2))"#,
        pkg = bazel_pkg
    );
    let output = run_command("bazel", &["query", &query, "--output=location"])?;
    let mut files = Vec::new();
    for line in output.lines() {
        // Bazel may output INFO lines; filter for lines starting with '/'
        if line.starts_with('/') {
            // Each line is of the form:
            //   /full/path/to/file:line:column: <other info>
            // We split on ':' and take the first field (the file path).
            if let Some(file_path) = line.split(':').next() {
                files.push(file_path.to_string());
            }
        }
    }
    Ok(files)
}

/// Prints a header and the full content of a file.
fn print_file_with_content(file_path: &str) -> Result<(), String> {
    println!("==> {} <==", file_path);
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
    println!("{}", content);
    Ok(())
}

fn main() -> Result<(), String> {
    // We support only a single source file.
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: cargo run -- <source_file>".to_string());
    }
    let source_file = &args[0];

    // Determine the Bazel package for the given source file.
    let bazel_pkg = get_bazel_package(source_file)?;

    // Build and run the dependency query using the determined package.
    let related_files = get_related_source_files(&bazel_pkg)?;

    // For each related file, print its name and contents.
    for file in related_files {
        if fs::metadata(&file).is_ok() {
            print_file_with_content(&file)?;
        } else {
            eprintln!("Warning: File {} does not exist.", file);
        }
    }
    Ok(())
}
