use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Retrieve source code context for a given file using Bazel.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The source file to analyze.
    source_file: String,

    /// The maximum number of lines to print.
    #[arg(long, short, default_value_t = 2000)]
    limit: usize,

    /// The dependency depth.
    #[arg(long, short, default_value_t = 2)]
    depth: usize,

    /// Filter by the extension of the input file.
    #[arg(long, short, value_delimiter = ',')]
    include_file_types: Option<Vec<String>>,

    /// List of files to always include.
    #[arg(long, short, value_delimiter = ',')]
    always_include: Option<Vec<String>>,
}

/// Runs an external command and returns its stdout as a String.
fn run_command(command: &str, args: &[&str]) -> Result<(String, std::process::ExitStatus)> {
    let child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute command: {}", command))?;

    let output = child
        .wait_with_output()
        .with_context(|| format!("Failed to wait for command: {}", command))?;

    let stdout = String::from_utf8(output.stdout)
        .with_context(|| format!("Failed to decode stdout for command: {}", command))?;
    let stderr = String::from_utf8(output.stderr)
        .with_context(|| format!("Failed to decode stderr for command: {}", command))?;

    if !output.status.success() {
        eprintln!("Command stderr: {}", stderr); // Print stderr for debugging
    }
    Ok((stdout.trim().to_string(), output.status))
}

/// Parses the output from a bazel query --output=location command.
fn parse_bazel_output(output: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for line in output.lines() {
        // Bazel location output is typically: "filename:line:col: ..."
        if let Some(file_path) = line.split(':').next() {
            files.push(PathBuf::from(file_path));
        }
    }
    Ok(files)
}

/// Prints a header and the full content of a file, up to the line limit.
fn print_file_content(
    file_path: &Path,
    line_limit: usize,
    lines_printed: &mut usize,
) -> Result<()> {
    if *lines_printed >= line_limit {
        return Ok(()); // Limit reached
    }

    if !file_path.exists() {
        eprintln!("Warning: File {} does not exist.", file_path.display());
        return Ok(());
    }

    let file_lines = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?
        .lines()
        .count();
    let remaining_lines = line_limit - *lines_printed;

    if remaining_lines >= file_lines {
        println!("==> {} <==", file_path.display());
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
        print!("{}", content);
        *lines_printed += file_lines;
    } else {
        // Could add partial printing here if desired
    }
    Ok(())
}

/// Finds the Bazel package for a given file.
fn find_package(source_file: &str) -> Result<String> {
    let (output, status) = run_command("bazel", &["query", source_file, "--output=package"])?;
    if !status.success() {
        anyhow::bail!("Bazel query failed: {}", output);
    }
    Ok(output)
}

/// Gets the dependent source files for a given target using a single Bazel query.
fn get_dependent_source_files(
    package: &str,
    source_file: &str,
    depth: usize,
) -> Result<Vec<PathBuf>> {
    let query = format!(
        r#"kind("source file", deps(rdeps({}:all, {}, {}), {}))"#,
        package, source_file, depth, depth
    );
    let (output, status) = run_command("bazel", &["query", &query, "--output=location"])?;
    if !status.success() {
        anyhow::bail!("Bazel query failed: {}", output);
    }
    parse_bazel_output(&output)
}

/// Computes the “distance” between two paths.
fn path_distance(a: &Path, b: &Path) -> Result<usize> {
    let a = a.canonicalize()?;
    let b = b.canonicalize()?;

    let a_components: Vec<_> = a.components().collect();
    let b_components: Vec<_> = b.components().collect();

    let common_components = a_components
        .iter()
        .zip(b_components.iter())
        .take_while(|(a_comp, b_comp)| a_comp == b_comp)
        .count();

    Ok((a_components.len() - common_components) + (b_components.len() - common_components))
}

/// Extracts the file extension from a PathBuf, handling cases with no extension.
fn get_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|os_str| os_str.to_str())
        .map(String::from)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let source_file_path = PathBuf::from(&args.source_file);

    let mut lines_printed = 0;
    let mut printed_files = HashSet::new();

    // Always include files, print them first
    if let Some(always_include) = &args.always_include {
        for file_path_str in always_include {
            let file_path = PathBuf::from(file_path_str);
            if printed_files.contains(&file_path) {
                continue;
            }
            print_file_content(&file_path, args.limit, &mut lines_printed)?;
            printed_files.insert(file_path);
            if lines_printed >= args.limit {
                return Ok(());
            }
        }
    }

    let package = find_package(&args.source_file)?;
    let mut dep_files = get_dependent_source_files(&package, &args.source_file, args.depth)?;

    dep_files.sort_by_key(|file| path_distance(&source_file_path, file).unwrap_or(usize::MAX));

    let mut included_extensions = args
        .include_file_types
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    if let Some(source_file_ext) = get_extension(&source_file_path) {
        included_extensions.insert(source_file_ext.clone());
    }
    dep_files.retain(|dep_file| {
        if let Some(dep_file_ext) = get_extension(dep_file) {
            included_extensions.contains(&dep_file_ext)
        } else {
            false
        }
    });

    for file in dep_files {
        if printed_files.contains(&file) {
            continue;
        }
        print_file_content(&file, args.limit, &mut lines_printed)?;
        printed_files.insert(file);

        if lines_printed >= args.limit {
            break;
        }
    }

    Ok(())
}
