//! MQuickJS REPL
//!
//! Interactive JavaScript shell and script runner.

use mquickjs::Context;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        // Run a script file
        run_file(&args[1]);
    } else {
        // Interactive REPL
        run_repl();
    }
}

fn run_file(filename: &str) {
    let source = match std::fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", filename, e);
            std::process::exit(1);
        }
    };

    let mut ctx = Context::new(64 * 1024); // 64KB default

    match ctx.eval(&source) {
        Ok(result) => {
            if !result.is_undefined() {
                println!("{}", result);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_repl() {
    println!("MQuickJS - Rust Edition");
    println!("Type JavaScript code to evaluate, Ctrl+D to exit.\n");

    let mut ctx = Context::new(1024 * 1024); // 1MB for REPL
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF
                println!();
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                match ctx.eval(line) {
                    Ok(result) => {
                        println!("{}", result);
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
}
