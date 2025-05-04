use std::collections::VecDeque;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};
use std::str;

struct Arguments {
    /// Type the selection instead of copying to the clipboard.
    typ: bool,
}

impl Arguments {
    fn new() -> Self {
        Self { typ: false }
    }

    fn parse() -> Self {
        let mut arguments = Arguments::new();

        let mut args_iter = env::args();
        _ = args_iter.next(); // Program name
        for arg in args_iter {
            match arg.as_str() {
                "-h" | "--help" => print_usage(),
                "-t" | "--type" => {
                    arguments.typ = true;
                    println!("Typing is not yet implemented!");
                }
                _ => panic!("Unknown flag or value: \"{}\"!", arg.as_str()),
            }
        }

        arguments
    }
}

fn print_usage() {
    println!(
        "A utility to copy passwords from pass using fuzzel.

Usage: {} [options]...

Options:
     -t,--type
         Type the selection instead of copying to the clipboard.
     -h,--help
         Show this help message.",
        env::args()
            .next()
            .unwrap_or_else(|| "fuzzel-pass".to_string())
    );
}

fn main() {
    // TODO: implement typing
    let _args = Arguments::parse();

    // Get all passwords from "pass list"
    let pass_list = Command::new("pass")
        .arg("list")
        .output()
        .expect("Failed to list passwords using \"pass list\"!");

    // Convert the "pass list" passwords to a &str
    let password_list = if pass_list.status.success() {
        str::from_utf8(&pass_list.stdout).expect("Output of \"pass list\" is not valid UTF-8!")
    } else {
        let stderr =
            str::from_utf8(&pass_list.stderr).expect("Output of \"pass list\" is not valid UTF-8!");
        panic!("Failed to list passwords using \"pass list\":\n{}", stderr)
    };

    // Parse the passwords with their shit format into a vector
    let passwords = parse_passwords(password_list);

    // Spawn fuzzel to select a password
    let mut fuzzel_dmenu = Command::new("fuzzel")
        .arg("--dmenu")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect(
            "Failed to spawn the \"fuzzel --dmenu\" command. Maybe \"fuzzel\" is not installed?",
        );

    // Pipe the passwords list into fuzzel
    if let Some(stdin) = &mut fuzzel_dmenu.stdin {
        stdin
            .write_all(passwords.join("\n").as_bytes())
            .expect("Failed to pipe passwords into \"fuzzel --dmenu\"!");
    }

    // Get the selection from fuzzel
    let selection = fuzzel_dmenu
        .wait_with_output()
        .expect("Failed to get a password selection from \"fuzzel --dmenu\"!");
    let selection = String::from_utf8(selection.stdout)
        .expect("The chosen password from \"fuzzel --dmenu\" is not valid UTF-8!");
    // Remove previously added newline
    let selection = selection.trim();

    dbg!(selection);
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
