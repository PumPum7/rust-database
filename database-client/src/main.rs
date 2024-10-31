use database::protocol::error::ProtocolError;
use database::protocol::{connection::Connection, response::Response};
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::Hinter;
use rustyline::validate::{MatchingBracketValidator, Validator};
use rustyline::{CompletionType, Config, Editor};
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

struct Client {
    conn: Connection,
}

impl Client {
    fn new(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self {
            conn: Connection::new(stream),
        })
    }

    fn execute_command(&mut self, input: &str) -> Result<String, Box<dyn std::error::Error>> {
        if input.trim().is_empty() {
            return Ok(String::new());
        }

        match self.conn.send_raw_command(input) {
            Ok(_) => {}
            Err(ProtocolError::ConnectionClosed) => {
                return Err("Connection closed by server".into());
            }
            Err(e) => return Err(Box::new(e)),
        }

        match self.conn.receive_response() {
            Ok(response) => match response {
                Response::Ok => Ok("OK\n".into()),
                Response::Value(Some(value)) => Ok(format!("{:?}\n", value)),
                Response::Value(None) => Ok("NULL\n".into()),
                Response::Range(results) => {
                    let mut output = String::new();
                    for (key, value) in results {
                        output.push_str(&format!("{}: {:?}\n", key, value));
                    }
                    Ok(output)
                }
                Response::Error(err) => Ok(format!("ERROR: {}\n", err)),
                Response::Pong => Ok("PONG\n".into()),
                Response::Size(size) => Ok(format!("{}\n", size)),
            },
            Err(ProtocolError::ConnectionClosed) => Err("Connection closed by server".into()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

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

fn print_help() -> String {
    r#"Available commands:
┌────────────────────────────┬──────────────────────────────────┐
│ Command                    │ Description                      │
├────────────────────────────┼──────────────────────────────────┤
│ GET <key>                  │ Get value by key                 │
│ SET <key> EXPR(<expr>)     │ Set key to expression result     │
│ UPDATE <key> EXPR(<expr>)  │ Update key with expression       │
│ DEL <key>                  │ Delete key-value pair            │
│ ALL                        │ Get all key-value pairs          │
│ STRLEN <key>               │ Get length of value by key       │
│ STRCAT <key> <value>       │ Concatenate value to key         │
│ SUBSTR <key> <start> <len> │ Get substring of value by key    │
│ EXPR(<expression>)         │ Calculate expression             │
│ Expression Examples:       │                                  │
│ EXPR(GET 1 + GET 2)        │ Calculate sum of values          │
│ SET 3 EXPR(GET 1 * 2)      │ Set using expression             │
│ EXPR(GET 1 + 3.14)         │ Mix direct values and GET        │
│ exit                       │ Exit the client                  │
│ help                       │ Show this help message           │
└────────────────────────────┴──────────────────────────────────┘"#
        .to_string()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header();

    let mut client = Client::new("127.0.0.1:5432")?;

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
            "EXPR".to_string(),
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
                println!("{}", print_help())
            }
            "exit" => break,
            cmd => match client.execute_command(cmd) {
                Ok(response) => print!("{}", response),
                Err(e) => println!("Error: {}", e),
            },
        }
    }

    Ok(())
}
