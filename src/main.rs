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
            "Command {} {:?} failed: {}",
            command,
            args,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Finds all targets that depend on the given source file.
fn find_rdeps(package: String, source_file: &str, depth: usize) -> Result<Vec<String>, String> {
    let depth_str = match depth > 0 {
        true => format!(", {}", depth),
        false => "".to_string(),
    };
    let query = format!("kind(rule, rdeps({}:all, {}{}))", package, source_file, depth_str);
    let output = run_command("bazel", &["query", &query, "--output=label"])?;
    Ok(output.lines().map(String::from).collect())
}

/// Gets the dependent source files for a given target.
fn get_dependent_source_files(target: &str, depth: usize) -> Result<Vec<String>, String> {
    let query = format!(r#"kind("source file", deps({}, {}))"#, target, depth);
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

fn find_package(source_file: &str) -> Result<String, String> {
    let output = run_command("bazel", &["query", source_file, "--output=package"])?;
    Ok(output)
}

/// Computes the “distance” between two paths as the number of steps needed
/// to get from one to the other.
fn path_distance(a: &Path, b: &Path) -> usize {
    // Collect the components of each path.
    let a_components: Vec<_> = a.components().collect();
    let b_components: Vec<_> = b.components().collect();

    // Count how many components are common starting from the beginning.
    let common_components = a_components
        .iter()
        .zip(b_components.iter())
        .take_while(|(a_comp, b_comp)| a_comp == b_comp)
        .count();

    // The distance is the number of remaining components in both paths.
    (a_components.len() - common_components) + (b_components.len() - common_components)
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let source_file = &args.source_file;
    let source_file_path = Path::new(source_file);

    let line_limit = args.limit;
    let mut lines_printed = 0;

    let package = find_package(source_file)?;
    let main_targets = find_rdeps(package, source_file, 1)?;
    let main_target = main_targets
        .first()
        .ok_or(format!("no main target found for file {}", source_file))?;

    let mut target_files = get_dependent_source_files(&main_target, 1)?;
    target_files.sort_by_key(|file| path_distance(&source_file_path, &Path::new(file)));
    for file in target_files.clone() {
        lines_printed += print_file_with_content(&file, line_limit, lines_printed);
        if lines_printed >= line_limit {
            return Ok(()); // Stop if limit is reached
        }
    }

    let mut dep_files = get_dependent_source_files(main_target, 2)?;
    dep_files.sort_by_key(|file| path_distance(&source_file_path, &Path::new(file)));
    for file in dep_files {
        if target_files.contains(&file) {
            // Skip files that were already printed
            continue;
        }
        lines_printed += print_file_with_content(&file, line_limit, lines_printed);
        if lines_printed >= line_limit {
            return Ok(()); // Stop if limit is reached
        }
    }

    Ok(())
}
