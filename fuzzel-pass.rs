use std::collections::VecDeque;
use std::io::{Error, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio, exit};
use std::{env, error};
use std::{fmt, str};

// Print newlines in Main function errors
struct MainError(String);

impl fmt::Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for MainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MainError {
    fn from(value: String) -> Self {
        MainError(value)
    }
}

#[derive(Debug)]
enum ParseFieldsError {
    MultilineError { password: String, field: Field },
}

impl fmt::Display for ParseFieldsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseFieldsError::MultilineError { password, field } => {
                write!(
                    f,
                    "Failed in password \"{}\": Expected a multiline field after \"{}\", but got: {}",
                    password, field.key, field.value
                )
            }
        }
    }
}

impl error::Error for ParseFieldsError {}

impl From<ParseFieldsError> for Error {
    fn from(value: ParseFieldsError) -> Self {
        Error::other(value)
    }
}

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
        Error::other(value)
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
        Error::other(value)
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
        Error::other(value)
    }
}

// A field of a password
#[derive(Debug)]
struct Field {
    key: String,
    value: String,
    is_multiline: bool,
}

struct Arguments {
    /// The password to show
    show_password: Option<String>,
    /// Type the selection instead of copying to the clipboard.
    type_selection: bool,
}

impl Arguments {
    fn new() -> Self {
        Self {
            show_password: None,
            type_selection: false,
        }
    }

    fn parse() -> Self {
        let mut arguments = Arguments::new();

        let mut args_iter = env::args();
        _ = args_iter.next(); // Program name
        for (idx, arg) in args_iter.enumerate() {
            match arg.as_str() {
                "-h" | "--help" => print_usage(),
                "-t" | "--type" => arguments.type_selection = true,
                value => {
                    if idx == 0 {
                        arguments.show_password = Some(value.to_string());
                    } else {
                        panic!("Unknown flag or value: \"{}\"!", value);
                    }
                }
            }
        }

        arguments
    }
}

fn print_usage() {
    eprintln!(
        "A utility to copy passwords from pass using fuzzel.

Usage: {} [password] [options]...

Positional Arguments:
     [password]
         A password to show directly, skipping the selection.

Options:
     -t,--type
         Type the selection instead of copying to the clipboard.
     -h,--help
         Show this help message.",
        env::args().next().unwrap_or("fuzzel-pass".to_string())
    );

    exit(0);
}

fn main() -> Result<(), MainError> {
    let args = Arguments::parse();

    let selected_password = if let Some(password) = args.show_password {
        password
    } else {
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

        // Set selected password using fuzzel
        fuzzel_select_value(&passwords).map_err(|e| format!("Failed selecting a value using fuzzel!: {}", e))?
    };

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
        ))?;
    };

    // Parse fields from "pass show <PWD>"
    let mut fields = parse_fields(field_list.to_string(), selected_password.clone())
        .map_err(|e| format!("Error while parsing the fields out of \"pass show\": {}", e))?;

    // Add the password in front
    let password = field_list.lines().next().ok_or_else(|| {
        format!(
            "Expected a password in the password file of \"{}\", but found nothing!",
            selected_password
        )
    })?;
    fields.push_front(Field {
        key: String::from("password"),
        value: password.to_string(),
        is_multiline: false,
    });

    // Select a field using fuzzel
    let field_keys = fields.iter().map(|field| field.key.clone()).collect::<Vec<String>>();
    let selected_field_key = fuzzel_select_value(&field_keys)
        .map_err(|e| format!("Error while selecting a password field using fuzzel!: {}", e))?;

    let selected_field = fields.iter().find(|field| field.key == selected_field_key);
    if selected_field.is_none() {
        Err("You somehow selected a non-existant field using fuzzel!".to_string())?;
    }

    // Copy selection to clipboard or type when that flag is passed
    if args.type_selection {
        if selected_field.unwrap().is_multiline {
            Err(format!(
                "Typing multiline fields using wtype is not recommended!\nPassword: {}\nField: {}",
                selected_password,
                selected_field.unwrap().key
            ))?;
        }

        type_field_value(&selected_field.unwrap().value)
            .map_err(|e| format!("Error while typing the selected fields value using wtype: {}", e))?;
    } else {
        copy_field_value(&selected_field.unwrap().value).map_err(|e| {
            format!(
                "Error while copying the selected fields value to the clipboard using wl-copy: {}",
                e
            )
        })?;
    }

    Ok(())
}

/// Parses the fields from "pass show <PWD>".
fn parse_fields(field_list: String, selected_password: String) -> Result<VecDeque<Field>, ParseFieldsError> {
    #[derive(PartialEq, Eq)]
    enum State {
        LookingForField,
        ReadingMultiline {
            key: String,
            marker: String,
            buffer: String,
        },
    }

    let mut result = VecDeque::new();
    let mut state = State::LookingForField;

    for raw_line in field_list.lines() {
        let line = raw_line.trim_end();

        match &mut state {
            State::LookingForField => {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.to_string();
                    let value = value.trim();

                    if value.is_empty() {
                        state = State::ReadingMultiline {
                            key,
                            marker: String::new(),
                            buffer: String::new(),
                        };
                    } else {
                        result.push_back(Field {
                            key,
                            value: value.to_string(),
                            is_multiline: false,
                        });
                    }
                }
            }
            State::ReadingMultiline { key, marker, buffer } => {
                let trimmed_line = line.trim();

                if marker.is_empty() {
                    if !trimmed_line.is_empty() {
                        *marker = trimmed_line.to_string();
                    }
                } else if trimmed_line == marker {
                    result.push_back(Field {
                        key: key.clone(),
                        value: buffer.trim_end().to_string(),
                        is_multiline: true,
                    });
                    state = State::LookingForField;
                } else {
                    buffer.push_str(line);
                    buffer.push('\n');
                }
            }
        }
    }

    if let State::ReadingMultiline { key, marker: _, buffer } = state {
        return Err(ParseFieldsError::MultilineError {
            password: selected_password,
            field: Field {
                key,
                value: buffer,
                is_multiline: true,
            },
        });
    }

    Ok(result)
}

/// Types the passed value wherever the cursor is using wtype.
fn type_field_value(value: &str) -> Result<(), TypeFieldError> {
    let wtype_status = Command::new("wtype")
        .arg(value)
        .status()
        .map_err(TypeFieldError::CommandFailed)?;

    if !wtype_status.success() {
        return Err(TypeFieldError::CommandFailed(Error::other(format!(
            "wtype failed with exit code: {}",
            wtype_status.code().unwrap_or(
                wtype_status
                    .stopped_signal()
                    .expect("If this fails I shoot myself in the foot!")
            )
        ))));
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
        return Err(CopyFieldError::CopyFailed(Error::other(format!(
            "wl-copy failed with exit code: {}",
            wl_copy_status.code().unwrap_or(
                wl_copy_status
                    .stopped_signal()
                    .expect("If this fails I shoot myself in the foot!")
            )
        ))));
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
        if let Some(p_or_p) = stack.iter().last()
            && !p_or_p.is_directory
        {
            let password = stack
                .iter()
                .map(|pwd| &pwd.value)
                .cloned()
                .collect::<Vec<String>>()
                .join("/");
            passwords.push(password);
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
        .replace("\u{1b}[00m", "")
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
