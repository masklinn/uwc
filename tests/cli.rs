/// Note that because std::process::Output::std{out,err} is just a Vec<u8> and
/// OsString::from_vec is unstable, these tests assume that stdout is valid UTF-8.
extern crate env_logger;
#[macro_use]
extern crate log;

use std::collections::{HashSet, VecDeque};
use std::process::Command;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::io::Read;

fn main_binary() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.arg("run");
    cmd.arg("-q");
    cmd
}

fn main_binary_with_args<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = main_binary();
    cmd.arg("--");
    cmd.args(args);
    cmd
}

/// Takes a String that should be the output of a run, discards the header, and
/// parses the rest of the output into their fields.
fn parse_lines<'a>(output: &'a str, has_header: bool) -> HashSet<(Vec<usize>, &'a str)> {
    let mut lines: VecDeque<&str> = output.lines().collect();

    // If there's a header, there should be at least 2 lines. If there is no
    // header, there should be at least one.
    let min_lines = if has_header { 2 } else { 1 };

    assert!(lines.len() >= min_lines, "bad output: {}", output);

    // discard the header if it has one
    if has_header {
        lines.pop_front();
    }

    let mut parsed = HashSet::new();

    for line in lines {
        let mut fields: Vec<&str> = line.split_whitespace().collect();
        let fname = fields.pop().unwrap();
        parsed.insert((
            fields
                .into_iter()
                .map(str::parse)
                .map(Result::unwrap)
                .collect(),
            fname,
        ));
    }

    parsed
}

/// Tests that the CLI run with no arguments prints the header with the default
/// counters and all 0s.
#[test]
fn test_no_args() {
    let out = main_binary().output().unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    let fields = parse_lines(&stdout, true);

    // lines  words  bytes  filename
    // 0      0      0      -
    let correct_fields: HashSet<_> = vec![(vec![0usize, 0, 0], "-")].into_iter().collect();
    assert_eq!(correct_fields, fields);

    // should be no stderr
    let stderr = out.stderr;
    assert_eq!(0, stderr.len());
}

/// Tests that the CLI run with no arguments prints the header with the default
/// counters and all 0s.
#[test]
fn test_no_args_no_elastic_tabs() {
    let out = main_binary_with_args(&["--no-elastic"]).output().unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    let correct_output = String::from(
        r#"lines	words	bytes	filename
0	0	0	-
"#,
    );

    assert_eq!(correct_output, stdout);

    // should be no stderr
    let stderr = out.stderr;
    assert_eq!(0, stderr.len());
}

// ----------------------------
//      FIXTURE TESTS
// ----------------------------

const FIXTURES_DIR: &str = "tests/fixtures";
const INPUT_FILE_NAME: &str = "input";
const OUTPUT_FILE_NAME: &str = "output";
const OPTS_FILE_NAME: &str = "opts";

/// Get the input files from the given directory.
fn get_input_files(base: &Path) -> Vec<PathBuf> {
    fs::read_dir(base)
        .unwrap()
        .map(Result::unwrap)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(INPUT_FILE_NAME)
        })
        .collect()
}

/// Soak up the given file into a String, unwrapping along the way.
fn soak_string(path: &Path) -> String {
    let mut file = File::open(path).expect(&format!("error on test entry: {:?}", path));
    let mut string = String::new();
    file.read_to_string(&mut string).unwrap();
    string
}

/// In the 'fixtures' directory, there is a set of fixed files that provide
/// a sample input file and an accompanying file that contains what the output
/// is expected to be. This test walks the directory and verfies each one.
///
/// The files are laid out like:
///
/// ```
/// tests/fixtures
/// └── hello
///     ├── input.* 🠜  These files contains the sample text to give to the binary as
///     │              input.
///     ├── opts    🠜  This file contains the options to pass to the binary, passed
///     │              after the binary name itself, but before the input file's
///     │              positional argument.
///     └── output  🠜  This file contains the expected output. The fields will
///                    be parsed, so whitespace formatting doesn't matter, only
///                    order.
/// ```
#[test]
fn test_fixtures() {
    let _ = env_logger::init();

    let fixtures_path = Path::new(FIXTURES_DIR);

    for entry in fs::read_dir(fixtures_path).unwrap() {
        let test_path = entry.unwrap().path();

        if !test_path.is_dir() {
            continue;
        }

        let opts = soak_string(&test_path.join(OPTS_FILE_NAME));
        let input_paths = get_input_files(&test_path);

        let mut args: Vec<OsString> = opts.split_whitespace().map(OsString::from).collect();
        args.extend(input_paths.into_iter().map(PathBuf::into_os_string));

        let expected_output = soak_string(&test_path.join(OUTPUT_FILE_NAME));
        let correct_fields = parse_lines(&expected_output, true);

        let mut cmd = main_binary_with_args(&args);
        debug!("Running command: {:?}", cmd);

        let out = cmd.output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let fields = parse_lines(&stdout, true);

        assert_eq!(correct_fields, fields);
    }
}

