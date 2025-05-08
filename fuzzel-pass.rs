use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio, exit};
use std::{env, error};
use std::{fmt, str};

#[derive(Debug)]
enum FuzzelSelectError {
    SpawnFailed(Error),
    PipeFailed(Error),
    OutputFailed(Error),
    UserCancelled,
    Utf8Error(std::string::FromUtf8Error),
}

impl fmt::Display for FuzzelSelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FuzzelSelectError::SpawnFailed(e) => {
                write!(f, "Failed to spawn fuzzel! Maybe fuzzel is not installed?: {}", e)
            }
            FuzzelSelectError::PipeFailed(e) => write!(f, "Failed to pipe values into fuzzel!: {}", e),
            FuzzelSelectError::OutputFailed(e) => write!(f, "Failed get a selection from fuzzel!: {}", e),
            FuzzelSelectError::Utf8Error(e) => write!(f, "Fuzzel output is not valid UTF-8!: {}", e),
            FuzzelSelectError::UserCancelled => write!(f, "User cancelled the fuzzel selection!"),
        }
    }
}

impl error::Error for FuzzelSelectError {}

impl From<FuzzelSelectError> for Error {
    fn from(value: FuzzelSelectError) -> Self {
        Error::new(ErrorKind::Other, value)
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
enum CopyFieldError {
    SpawnFailed(Error),
    PipeFailed(Error),
    CopyFailed(Error),
}

impl fmt::Display for CopyFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopyFieldError::SpawnFailed(e) => write!(
                f,
                "Failed to spawn wl-copy! Maybe wl-clipboard is not installed?: {}",
                e
            ),
            CopyFieldError::PipeFailed(e) => write!(f, "Failed to pipe the selected fields value into wl-copy!: {}", e),
            CopyFieldError::CopyFailed(e) => write!(f, "Failed to copy to clipboard using wl-copy!: {}", e),
        }
    }
}

impl error::Error for CopyFieldError {}

impl From<CopyFieldError> for Error {
    fn from(value: CopyFieldError) -> Self {
        Error::new(ErrorKind::Other, value)
    }
}

#[derive(Debug)]
enum TypeFieldError {
    CommandFailed(Error),
}

impl fmt::Display for TypeFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeFieldError::CommandFailed(e) => write!(f, "Failed to run wtype! Maybe wtype is not installed?: {}", e),
        }
    }
}

impl error::Error for TypeFieldError {}

impl From<TypeFieldError> for Error {
    fn from(value: TypeFieldError) -> Self {
        Error::new(ErrorKind::Other, value)
    }
}

struct Arguments {
    /// Type the selection instead of copying to the clipboard.
    type_selection: bool,
}

impl Arguments {
    fn new() -> Self {
        Self { type_selection: false }
    }

    fn parse() -> Self {
        let mut arguments = Arguments::new();

        let mut args_iter = env::args();
        _ = args_iter.next(); // Program name
        for arg in args_iter {
            match arg.as_str() {
                "-h" | "--help" => print_usage(),
                "-t" | "--type" => arguments.type_selection = true,
                _ => panic!("Unknown flag or value: \"{}\"!", arg.as_str()),
            }
        }

        arguments
    }
}

fn print_usage() {
    eprintln!(
        "A utility to copy passwords from pass using fuzzel.

Usage: {} [options]...

Options:
     -t,--type
         Type the selection instead of copying to the clipboard.
     -h,--help
         Show this help message.",
        env::args().next().unwrap_or("fuzzel-pass".to_string())
    );

    exit(0);
}

fn main() -> Result<(), String> {
    let args = Arguments::parse();

    // Get all passwords from "pass list"
    let pass_list = Command::new("pass")
        .arg("list")
        .output()
        .map_err(|e| format!("Failed to list password using \"pass list\"!: {}", e))?;

    // Convert the "pass list" passwords to a &str
    let password_list = if pass_list.status.success() {
        str::from_utf8(&pass_list.stdout).map_err(|e| format!("Output of \"pass list\" is not valid UTF-8!: {}", e))
    } else {
        let stderr = str::from_utf8(&pass_list.stderr)
            .map_err(|e| format!("The error output of \"pass list\" is not valid UTF-8!: {}", e))?;
        Err(format!("Failed to list passwords using \"pass list\": {}", stderr))
    };

    // Parse the passwords with their shit format into a vector
    let passwords = parse_passwords(password_list?);

    // Select password using fuzzel
    let selected_password =
        fuzzel_select_value(&passwords).map_err(|e| format!("Failed selecting a value using fuzzel!: {}", e))?;

    // Get the extra fields in the password file
    let pass_show = Command::new("pass")
        .arg("show")
        .arg(&selected_password)
        .output()
        .map_err(|e| {
            format!(
                "Failed to show the password contents using \"pass show {}\"!: {}",
                selected_password, e
            )
        })?;

    // Convert "pass show" output to a &str
    let field_list = if pass_show.status.success() {
        str::from_utf8(&pass_show.stdout).map_err(|e| {
            format!(
                "The contents of the password: \"{}\" are not valid UTF-8!: {}",
                selected_password, e
            )
        })?
    } else {
        let stderr = str::from_utf8(&pass_show.stderr)
            .map_err(|e| format!("The error output of \"pass show\" is not valid UTF-8!: {}", e))?;

        return Err(format!(
            "Failed to show the contents of the password using \"pass show {}\": {}",
            selected_password, stderr
        ));
    };

    // Parse fields from "pass show <PWD>"
    let mut fields = field_list
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.split_once(':').map(|(k, v)| (k, v.trim())).ok_or_else(|| {
                format!(
                    "Expected a key value pair split by ':' in the password file of \"{}\", but found: {}",
                    selected_password, line
                )
            })
        })
        // (&str, &str) => (Key, Value)
        .collect::<Result<VecDeque<(&str, &str)>, String>>()?;

    // Add the password in front
    let password = field_list.lines().next().ok_or_else(|| {
        format!(
            "Expected a password in the password file of \"{}\", but found nothing!",
            selected_password
        )
    })?;
    fields.push_front(("password", password));

    // Select a field using fuzzel
    let field_keys = fields.iter().map(|field| field.0.to_string()).collect::<Vec<String>>();
    let selected_field_key = fuzzel_select_value(&field_keys)
        .map_err(|e| format!("Error while selecting a password field using fuzzel!: {}", e))?;

    let selected_field = fields.iter().find(|field| field.0 == selected_field_key);
    if selected_field.is_none() {
        return Err("You somehow selected a non-existant field using fuzzel!".to_string());
    }

    // Copy selection to clipboard or type when that flag is passed
    if args.type_selection {
        type_field_value(selected_field.unwrap().1)
            .map_err(|e| format!("Error while typing the selected fields value using wl-copy: {}", e))?;
    } else {
        copy_field_value(selected_field.unwrap().1).map_err(|e| {
            format!(
                "Error while copying the selected fields value to the clipboard using wl-copy: {}",
                e
            )
        })?;
    }

    Ok(())
}

/// Types the passed value wherever the cursor is using wtype.
fn type_field_value(value: &str) -> Result<(), TypeFieldError> {
    let wtype_status = Command::new("wtype")
        .arg(value)
        .status()
        .map_err(TypeFieldError::CommandFailed)?;

    if !wtype_status.success() {
        return Err(TypeFieldError::CommandFailed(Error::new(
            ErrorKind::Other,
            format!(
                "wtype failed with exit code: {}",
                wtype_status.code().unwrap_or(
                    wtype_status
                        .stopped_signal()
                        .expect("If this fails I shoot myself in the foot!")
                )
            ),
        )));
    }

    Ok(())
}

/// Copies the passed value to the clipboard using wl-copy.
fn copy_field_value(value: &str) -> Result<(), CopyFieldError> {
    let mut wl_copy = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(CopyFieldError::SpawnFailed)?;

    // Pipe the selected fields value into wl-copy
    if let Some(stdin) = &mut wl_copy.stdin {
        stdin.write_all(value.as_bytes()).map_err(CopyFieldError::PipeFailed)?;
    }

    // Check wl-copy status
    let wl_copy_status = wl_copy.wait().map_err(CopyFieldError::CopyFailed)?;
    if !wl_copy_status.success() {
        return Err(CopyFieldError::CopyFailed(Error::new(
            ErrorKind::Other,
            format!(
                "wl-copy failed with exit code: {}",
                wl_copy_status.code().unwrap_or(
                    wl_copy_status
                        .stopped_signal()
                        .expect("If this fails I shoot myself in the foot!")
                )
            ),
        )));
    }

    Ok(())
}

/// Select and return a value from the given list of values using fuzzel.
fn fuzzel_select_value(values: &[String]) -> Result<String, FuzzelSelectError> {
    // Spawn fuzzel to select a value
    let mut fuzzel_dmenu = Command::new("fuzzel")
        .arg("--dmenu")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(FuzzelSelectError::SpawnFailed)?;

    // Pipe the passwords list into fuzzel
    if let Some(stdin) = &mut fuzzel_dmenu.stdin {
        stdin
            .write_all(values.join("\n").as_bytes())
            .map_err(FuzzelSelectError::PipeFailed)?;
    }

    // Get the selected values from fuzzel
    let selection = fuzzel_dmenu
        .wait_with_output()
        .map_err(FuzzelSelectError::OutputFailed)?;
    if !selection.status.success() {
        return Err(FuzzelSelectError::UserCancelled);
    }
    let selection = String::from_utf8(selection.stdout).map_err(FuzzelSelectError::Utf8Error)?;

    // Remove previously added newline
    Ok(selection.trim().to_string())
}

/// Parse the passwords list and get the password paths.
fn parse_passwords(passwords_list: &str) -> Vec<String> {
    let mut passwords = Vec::new();
    let mut stack = VecDeque::new();

    struct PasswordOrDirectory {
        value: String,
        is_directory: bool,
    }

    // Skip the first line
    let lines = passwords_list.lines().skip(1);

    for line in lines {
        let path_or_pwd = strip_line(line);
        let indent_level = get_line_indent(line);

        // Adjust the stack based on indentation level
        while stack.len() > indent_level {
            stack.pop_back();
        }

        // Add current path part to the stack
        stack.push_back(PasswordOrDirectory {
            value: path_or_pwd,
            is_directory: is_line_directory(line),
        });

        // Join the stack into the full path if it is not a directory
        if let Some(p_or_p) = stack.iter().last() {
            if !p_or_p.is_directory {
                let password = stack
                    .iter()
                    .map(|pwd| &pwd.value)
                    .cloned()
                    .collect::<Vec<String>>()
                    .join("/");
                passwords.push(password);
            }
        }
    }

    passwords
}

/// Calculates the password list lines indentation.
fn get_line_indent(line: &str) -> usize {
    // Count the leading spaces or fancy line characters for indentation
    let prefix: String = strip_ansi_line(line)
        .chars()
        .take_while(|&c| " ├└─│".contains(c))
        .collect();

    prefix.chars().filter(|&c| c == ' ' || c == '│').count() / 4
}

/// Check if a password list line is a directory.
fn is_line_directory(line: &str) -> bool {
    line.contains("\u{1b}[01;34m") && line.contains("\u{1b}[0m")
}

/// Strip out the ANSI codes and any non-breaking spaces from a password list line.
fn strip_ansi_line(line: &str) -> String {
    line.replace("\u{1b}[01;34m", "")
        .replace("\u{1b}[0m", "")
        .replace("\u{a0}", " ")
}

/// Remove unwanted characters in a password list line.
fn strip_line(line: &str) -> String {
    let no_ansi = strip_ansi_line(line);

    // Remove leading spaces and those things: └ ├ ─ │
    no_ansi
        .trim_start_matches(|c: char| c.is_whitespace() || "└├─│".contains(c))
        .to_string()
}
