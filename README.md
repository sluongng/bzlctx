(Disclaimer: 99% of this project was written by an LLM under human supervision.)

# bzlctx

`bzlctx` is a command-line tool that leverages Bazel's dependency graph to retrieve the source code of files related to a given input file. This can be particularly useful for providing context to Large Language Models (LLMs) when working with codebases managed by Bazel. By understanding the dependencies of a specific file, an LLM can gain a broader understanding of the code's purpose and functionality.

## How it Works

Given a source file, `bzlctx` performs the following steps:

1.  **Determine the Bazel Package:** It uses `bazel query` to find the Bazel package that the input file belongs to.
2.  **Query Dependencies:** It constructs a Bazel query `kind("source file", deps({pkg}, 2))` to find all source files within a dependency depth of 2 from the determined package. The output format is set to `location`.
3.  **Extract File Paths:** The output of the query (which includes file paths, line numbers, and column numbers) is parsed to extract only the file paths.
4.  **Print File Contents:**  For each related file found, `bzlctx` prints a header indicating the file name (`==> filename <==`) followed by the complete content of the file.  It checks if files exists and prints a warning if they do not.

## Installation

You can install `bzlctx` directly from crates.io using Cargo:

```bash
cargo install bzlctx
```
Or directly from the GitHub repository:
```bash
cargo install --git https://github.com/sluongng/bzlctx
```

## Usage

```bash
cargo run -- <source_file>
```

Replace `<source_file>` with the path to the source file you want to analyze.  The path should be relative to your Bazel workspace root, or an absolute path.

**Example:**

If you have a file `src/main.rs` in your Bazel workspace, you would run:

```bash
cargo run -- src/main.rs
```

The output will be a series of file names and their contents, representing the source files related to `src/main.rs` according to Bazel's dependency graph.

## Use Case: LLM Context Building

The primary use case for `bzlctx` is to provide context to LLMs for code-related tasks. For example, if you are using an LLM to generate documentation, debug, or refactor a specific
file, you can use `bzlctx` to gather the relevant surrounding code.  This allows the LLM to operate with a more complete understanding of the codebase, leading to more accurate and
relevant results.  Instead of just providing the single source file to the LLM, you can provide the output of `bzlctx`, giving the LLM a much richer context.

Here is an example of using `bzlctx` to provide context to [llm](https://github.com/simonw/llm/) using a Gemini-Thinking model:

```bash
~/work/bazelbuild/bazelisk> bzlctx core/repositories.go | llm -m gemini-t 'rewrite the Repo interfaces in rust'
\`\`\`rust
use std::error::Error as StdError;

// Define a placeholder for the Error type.
// You should replace this with your actual error type in your Rust project.
pub type Error = Box<dyn StdError>;

// Define a placeholder for the Config struct.
// You should replace this with your actual Config struct in your Rust project.
pub struct Config;

// FilterOpts represents options relevant to filtering Bazel versions.
pub struct FilterOpts<'a> {
    pub max_results: i32,
    pub track: i32,
    pub filter: Option<Box<dyn Fn(&str) -> bool + 'a>>, // Option to make filter optional
}

// LTSRepo represents a repository that stores LTS Bazel releases and their candidates.
pub trait LtsRepo {
    // GetLTSVersions returns a list of all available LTS release (candidates) that match the given filter options.
    // Warning: Filters only work reliably if the versions are processed in descending order!
    fn get_lts_versions(
        &self,
        bazelisk_home: &str,
        opts: &FilterOpts
    ) -> Result<Vec<String>, Error>;

    // DownloadLTS downloads the given Bazel version into the specified location and returns the absolute path.
    fn download_lts(
        &self,
        version: &str,
        dest_dir: &str,
        dest_file: &str,
        config: &Config
    ) -> Result<String, Error>;
}

// ForkRepo represents a repository that stores a fork of Bazel (releases).
pub trait ForkRepo {
...
```

## Limitations

*   **Single Source File:**  `bzlctx` currently only supports a single source file as input.
*   **Dependency Depth:** The dependency depth is currently hardcoded to 2.  Future versions might allow configuring this depth.
*   **Bazel Dependency:** `bzlctx` relies on the `bazel` command-line tool being available in your system's PATH.
*   **Output format:**  The current version outputs all sources, which might be a lot for a very interconnected codebase. It does not support filtering or summarizing yet.

## Contributing

Contributions are welcome!  Please feel free to open issues or submit pull requests on the [GitHub repository](https://github.com/sluongng/bzlctx).

