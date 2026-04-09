use sqlengine::tokenizer::Tokenizer;
use sqlengine::parser::Parser;
use sqlengine::engine::Database;
use sqlengine::storage;
use std::io::{self, Write, BufRead};

const DEFAULT_DB_FILE: &str = "database.json";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let db_path = args.get(1).map(|s| s.as_str()).unwrap_or(DEFAULT_DB_FILE);

    // Load existing database
    let mut db = match storage::load_from_file(db_path) {
        Ok(db) => {
            let table_count = db.tables.len();
            if table_count > 0 {
                println!("Loaded database from '{}' ({} table(s))", db_path, table_count);
            }
            db
        }
        Err(e) => {
            eprintln!("Warning: Could not load '{}': {}", db_path, e);
            Database::new()
        }
    };

    println!("SQLEngine v0.1.0 — In-memory SQL database");
    println!("Type SQL statements, or use these commands:");
    println!("  .tables    — list all tables");
    println!("  .schema    — show table schemas");
    println!("  .save      — save database to file");
    println!("  .quit      — exit (auto-saves)");
    println!();

    let stdin = io::stdin();
    let mut input_buf = String::new();

    loop {
        // Print prompt
        if input_buf.is_empty() {
            print!("sql> ");
        } else {
            print!("  -> ");
        }
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
            Ok(_) => {}
        }

        let trimmed = line.trim();

        // Handle dot-commands
        if input_buf.is_empty() {
            match trimmed {
                ".quit" | ".exit" | "\\q" => {
                    save_db(&db, db_path);
                    println!("Goodbye.");
                    return;
                }
                ".tables" => {
                    if db.tables.is_empty() {
                        println!("No tables.");
                    } else {
                        let mut names: Vec<&String> = db.tables.keys().collect();
                        names.sort();
                        for name in names {
                            println!("  {}", name);
                        }
                    }
                    continue;
                }
                ".schema" => {
                    if db.tables.is_empty() {
                        println!("No tables.");
                    } else {
                        let mut names: Vec<&String> = db.tables.keys().collect();
                        names.sort();
                        for name in names {
                            let table = &db.tables[name];
                            let cols: Vec<String> = table.schema.columns.iter().map(|c| {
                                let pk = if c.primary_key { " PRIMARY KEY" } else { "" };
                                let dt = match c.data_type {
                                    sqlengine::ast::DataType::Int => "INT",
                                    sqlengine::ast::DataType::Float => "FLOAT",
                                    sqlengine::ast::DataType::Text => "TEXT",
                                    sqlengine::ast::DataType::Boolean => "BOOLEAN",
                                };
                                format!("{} {}{}", c.name, dt, pk)
                            }).collect();
                            println!("CREATE TABLE {} ({});", name, cols.join(", "));
                        }
                    }
                    continue;
                }
                ".save" => {
                    save_db(&db, db_path);
                    continue;
                }
                "" => continue,
                _ => {}
            }
        }

        input_buf.push_str(trimmed);
        input_buf.push(' ');

        // Check if the statement is complete (ends with ; or is a dot-command)
        let full = input_buf.trim();
        if !full.ends_with(';') && !full.is_empty() {
            // Multi-line input: keep reading
            continue;
        }

        let sql = input_buf.trim().to_string();
        input_buf.clear();

        if sql.is_empty() {
            continue;
        }

        // Tokenize
        let mut tokenizer = Tokenizer::new(&sql);
        let tokens = match tokenizer.tokenize() {
            Ok(tokens) => tokens,
            Err(e) => {
                eprintln!("Tokenizer error: {}", e);
                continue;
            }
        };

        // Parse
        let mut parser = Parser::new(tokens);
        let stmt = match parser.parse() {
            Ok(stmt) => stmt,
            Err(e) => {
                eprintln!("Parse error: {}", e);
                continue;
            }
        };

        // Execute
        match db.execute(stmt) {
            Ok(result) => {
                println!("{}", result);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    // Auto-save on exit
    save_db(&db, db_path);
}

fn save_db(db: &Database, path: &str) {
    if db.tables.is_empty() {
        return;
    }
    match storage::save_to_file(db, path) {
        Ok(_) => println!("Database saved to '{}'", path),
        Err(e) => eprintln!("Error saving database: {}", e),
    }
}
