use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::Hinter;
use rustyline::validate::{Validator, MatchingBracketValidator};
use rustyline::{CompletionType, Config, Editor};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

#[derive(Default)]
struct DbHelper {
    commands: Vec<String>,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
}

impl Completer for DbHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let line_parts: Vec<&str> = line[..pos].split_whitespace().collect();

        if line_parts.is_empty() {
            return Ok((0, self.commands.clone()));
        }

        let current = line_parts[0].to_uppercase();
        let matches: Vec<String> = self
            .commands
            .iter()
            .filter(|cmd| cmd.starts_with(&current))
            .cloned()
            .collect();

        Ok((0, matches))
    }
}

impl Highlighter for DbHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        self.highlighter.highlight(line, _pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, force_update: bool) -> bool {
        self.highlighter.highlight_char(line, pos, force_update)
    }
}

impl Hinter for DbHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        None
    }
}

impl Validator for DbHelper {
    fn validate(
        &self,
        ctx: &mut rustyline::validate::ValidationContext,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        self.validator.validate(ctx)
    }
}

impl rustyline::Helper for DbHelper {}

fn print_header() {
    println!(
        r#"
  ____        _        _
 |  _ \  __ _| |_ __ _| |__   __ _ ___  ___
 | | | |/ _` | __/ _` | '_ \ / _` / __|/ _ \
 | |_| | (_| | || (_| | |_) | (_| \__ \  __/
 |____/ \__,_|\__\__,_|_.__/ \__,_|___/\___|

        "#
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header();

    let mut stream = TcpStream::connect("127.0.0.1:5432")?;

    let helper = DbHelper {
        commands: vec![
            "GET".to_string(),
            "SET".to_string(),
            "UPDATE".to_string(),
            "DEL".to_string(),
            "ALL".to_string(),
            "STRLEN".to_string(),
            "STRCAT".to_string(),
            "SUBSTR".to_string(),
            "exit".to_string(),
            "help".to_string(),
        ],
        highlighter: MatchingBracketHighlighter::new(),
        validator: MatchingBracketValidator::new(),
    };

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(helper));

    println!("Connected to database. Type 'help' for commands, 'exit' to quit.");
    println!("Use TAB for command completion.");

    loop {
        let readline = match rl.readline("db> ") {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => {
                println!("Error: {}", err);
                break;
            }
        };

        if readline.trim().is_empty() {
            continue;
        }

        let _ = rl.add_history_entry(readline.as_str());

        match readline.trim() {
            "help" => {
                println!("Available commands:");
                println!("┌────────────────────────────┬──────────────────────────────────┐");
                println!("│ Command                    │ Description                      │");
                println!("├────────────────────────────┼──────────────────────────────────┤");
                println!("│ GET <key>                  │ Get value by key                 │");
                println!("│ SET <key> <value>          │ Set key-value pair               │");
                println!("│ UPDATE <key> <value>       │ Update key-value pair            │");
                println!("│ DEL <key>                  │ Delete key-value pair            │");
                println!("│ ALL                        │ Get all key-value pairs          │");
                println!("│ STRLEN <key>               │ Get length of value by key       │");
                println!("│ STRCAT <key> <value>       │ Concatenate value to key         │");
                println!("│ SUBSTR <key> <start> <len> │ Get substring of value by key    │");
                println!("│ exit                       │ Exit the client                  │");
                println!("│ help                       │ Show this help message           │");
                println!("└────────────────────────────┴──────────────────────────────────┘");
                continue;
            }
            "exit" => break,
            cmd => {
                stream.write_all(format!("{}\n", cmd).as_bytes())?;
                stream.flush()?;

                let mut reader = BufReader::new(&stream);
                let mut response = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line)?;
                    if line.trim() == "===END===" {
                        break;
                    }
                    response.push_str(&line);
                }
                print!("{}", response);
            }
        }
    }

    Ok(())
}
