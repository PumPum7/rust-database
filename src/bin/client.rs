use database::protocol::{Command, Response, connection::Connection};
use database::storage::Value;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::Hinter;
use rustyline::validate::{Validator, MatchingBracketValidator};
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
        let command = parse_command(input)?;
        self.conn.send_command(command)?;
        
        match self.conn.receive_response()? {
            Response::Ok => Ok("OK\n".into()),
            Response::Value(Some(value)) => Ok(format!("{:?}\n", value)),
            Response::Value(None) => Ok("NULL\n".into()),
            Response::Range(results) => {
                let mut output = String::new();
                for (key, value) in results {
                    output.push_str(&format!("{}: {:?}\n", key, value));
                }
                Ok(output)
            },
            Response::Error(err) => Ok(format!("ERROR: {}\n", err)),
            Response::Pong => Ok("PONG\n".into()),
            Response::Size(size) => Ok(format!("{}\n", size)),
        }
    }
}

fn parse_value(s: &str) -> Result<Value, Box<dyn std::error::Error>> {
    if s == "null" {
        Ok(Value::Null)
    } else if s == "true" {
        Ok(Value::Boolean(true))
    } else if s == "false" {
        Ok(Value::Boolean(false))
    } else if let Ok(i) = s.parse::<i64>() {
        Ok(Value::Integer(i))
    } else if let Ok(f) = s.parse::<f64>() {
        Ok(Value::Float(f))
    } else {
        Ok(Value::String(s.to_string()))
    }
}

fn parse_command(input: &str) -> Result<Command, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    match parts[0].to_uppercase().as_str() {
        "GET" => {
            if parts.len() != 2 {
                return Err("Usage: GET <key>".into());
            }
            Ok(Command::Get { key: parts[1].parse()? })
        },
        "SET" => {
            if parts.len() < 3 {
                return Err("Usage: SET <key> <value>".into());
            }
            let value = parts[2..].join(" ");
            Ok(Command::Set {
                key: parts[1].parse()?,
                value: parse_value(&value)?,
            })
        },
        "UPDATE" => {
            if parts.len() < 3 {
                return Err("Usage: SET <key> <value>".into());
            }
            let value = parts[2..].join(" ");
            Ok(Command::Update {
                key: parts[1].parse()?,
                value: parse_value(&value)?,
            })
        },
        "DEL" => {
            if parts.len() != 2 {
                return Err("Usage: DEL <key>".into());
            }
            Ok(Command::Delete { key: parts[1].parse()? })
        },
        "ALL" => Ok(Command::All),
        "STRLEN" => {
            if parts.len() != 2 {
                return Err("Usage: STRLEN <key>".into());
            }
            Ok(Command::Strlen { key: parts[1].parse()? })
        },
        "STRCAT" => {
            if parts.len() < 3 {
                return Err("Usage: STRCAT <key> <value>".into());
            }
            let value = parts[2..].join(" ");
            Ok(Command::Strcat {
                key: parts[1].parse()?,
                value: parse_value(&value)?,
            })
        },
        "SUBSTR" => {
            if parts.len() != 4 {
                return Err("Usage: SUBSTR <key> <start> <length>".into());
            }
            Ok(Command::Substr {
                key: parts[1].parse()?,
                start: parts[2].parse()?,
                length: parts[3].parse()?,
            })
        },
        "EXIT" => Ok(Command::Exit),
        _ => Err("Unknown command".into()),
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
                match client.execute_command(cmd) {
                    Ok(response) => print!("{}", response),
                    Err(e) => println!("Error: {}", e),
                }
            }
        }
    }

    Ok(())
}