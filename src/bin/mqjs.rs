//! MQuickJS REPL
//!
//! Interactive JavaScript shell and script runner.
//!
//! Usage: mqjs [options] [file [args]]
//!   -h, --help         List options
//!   -e, --eval EXPR    Evaluate EXPR
//!   -i, --interactive  Go to interactive mode after running script
//!   -I, --include FILE Include an additional file before main script
//!   -d, --dump         Dump memory usage stats
//!   -c, --compile      Compile to bytecode (output to .qbc file)
//!   --memory-limit N   Limit memory usage to N bytes (supports k/K, m/M suffixes)

use mquickjs::Context;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

/// Command line options
struct Options {
    /// Script file to run
    script: Option<String>,
    /// Expression to evaluate (-e)
    eval_expr: Option<String>,
    /// Go to interactive mode after script (-i)
    interactive: bool,
    /// Files to include before main script (-I)
    includes: Vec<String>,
    /// Dump memory stats (-d)
    dump_stats: bool,
    /// Compile only mode (-c)
    compile_only: bool,
    /// Memory limit in bytes
    memory_limit: usize,
    /// Script arguments (passed to script)
    script_args: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            script: None,
            eval_expr: None,
            interactive: false,
            includes: Vec::new(),
            dump_stats: false,
            compile_only: false,
            memory_limit: 1024 * 1024, // 1MB default
            script_args: Vec::new(),
        }
    }
}

fn print_help() {
    println!("usage: mqjs [options] [file [args]]");
    println!("-h  --help         list options");
    println!("-e  --eval EXPR    evaluate EXPR");
    println!("-i  --interactive  go to interactive mode");
    println!("-I  --include file include an additional file");
    println!("-d  --dump         dump the memory usage stats");
    println!("-c  --compile      compile to bytecode (.qbc file)");
    println!("    --memory-limit n       limit the memory usage to 'n' bytes");
}

fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if s.ends_with('k') || s.ends_with('K') {
        (&s[..s.len() - 1], 1024)
    } else if s.ends_with('m') || s.ends_with('M') {
        (&s[..s.len() - 1], 1024 * 1024)
    } else {
        (s, 1)
    };

    num_str.parse::<usize>().ok().map(|n| n * multiplier)
}

fn parse_args() -> Result<Options, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = Options::default();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "-e" | "--eval" => {
                i += 1;
                if i >= args.len() {
                    return Err("--eval requires an argument".to_string());
                }
                opts.eval_expr = Some(args[i].clone());
            }
            "-i" | "--interactive" => {
                opts.interactive = true;
            }
            "-I" | "--include" => {
                i += 1;
                if i >= args.len() {
                    return Err("--include requires a filename".to_string());
                }
                opts.includes.push(args[i].clone());
            }
            "-d" | "--dump" => {
                opts.dump_stats = true;
            }
            "-c" | "--compile" => {
                opts.compile_only = true;
            }
            "--memory-limit" => {
                i += 1;
                if i >= args.len() {
                    return Err("--memory-limit requires a value".to_string());
                }
                opts.memory_limit = parse_size(&args[i])
                    .ok_or_else(|| format!("invalid memory limit: {}", args[i]))?;
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown option: {}", arg));
            }
            _ => {
                // First non-option is the script, rest are script args
                opts.script = Some(arg.clone());
                opts.script_args = args[i + 1..].to_vec();
                break;
            }
        }
        i += 1;
    }

    Ok(opts)
}

fn main() {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Use -h for help.");
            std::process::exit(1);
        }
    };

    // Compile-only mode
    if opts.compile_only {
        if let Some(ref script) = opts.script {
            if let Err(e) = compile_to_bytecode(script) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        } else {
            eprintln!("Error: -c requires a script file");
            std::process::exit(1);
        }
        return;
    }

    let mut ctx = Context::new(opts.memory_limit);

    // Run include files first
    for include in &opts.includes {
        if let Err(e) = run_file(&mut ctx, include) {
            eprintln!("Error in include file {}: {}", include, e);
            std::process::exit(1);
        }
    }

    // Run -e expression
    if let Some(ref expr) = opts.eval_expr {
        match ctx.eval(expr) {
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

    // Run script file (check if it's a .qbc bytecode file)
    if let Some(ref script) = opts.script {
        if script.ends_with(".qbc") {
            if let Err(e) = run_bytecode_file(&mut ctx, script) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        } else if let Err(e) = run_file(&mut ctx, script) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    // Go to interactive mode if requested or no script/expr provided
    if opts.interactive || (opts.script.is_none() && opts.eval_expr.is_none()) {
        run_repl(&mut ctx);
    }

    // Dump memory stats if requested
    if opts.dump_stats {
        dump_memory_stats(&ctx);
    }
}

fn run_file(ctx: &mut Context, filename: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(filename)
        .map_err(|e| format!("Error reading {}: {}", filename, e))?;

    match ctx.eval(&source) {
        Ok(result) => {
            if !result.is_undefined() {
                println!("{}", result);
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn run_repl(ctx: &mut Context) {
    println!("MQuickJS - Rust Edition");
    println!("Type JavaScript code to evaluate, Ctrl+D to exit.\n");

    let mut rl = match DefaultEditor::new() {
        Ok(rl) => rl,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            return;
        }
    };

    // Try to load history from file
    let history_file = dirs_history_file();
    if let Some(ref path) = history_file {
        let _ = rl.load_history(path);
    }

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(line);

                match ctx.eval(line) {
                    Ok(result) => {
                        println!("{}", result);
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C - just show new prompt
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D - exit
                println!();
                break;
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_file {
        let _ = rl.save_history(path);
    }
}

/// Get the history file path (~/.mqjs_history)
fn dirs_history_file() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|mut p| {
        p.push(".mqjs_history");
        p
    })
}

/// Placeholder for dirs::home_dir if dirs crate is not available
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

fn dump_memory_stats(ctx: &Context) {
    let stats = ctx.memory_stats();
    println!("\nMemory usage:");
    println!("  Heap size:       {} bytes", stats.heap_size);
    println!("  Used:            {} bytes", stats.used);
    println!("  Runtime strings: {}", stats.runtime_strings);
    println!("  Arrays:          {}", stats.arrays);
    println!("  Objects:         {}", stats.objects);
    println!("  Closures:        {}", stats.closures);
    println!("  Error objects:   {}", stats.error_objects);
    println!("  RegExp objects:  {}", stats.regex_objects);
    println!("  TypedArrays:     {}", stats.typed_arrays);
}

/// Bytecode file magic bytes
const BYTECODE_MAGIC: &[u8] = b"MQJS";
/// Bytecode file version
const BYTECODE_VERSION: u8 = 1;

/// Compile a JavaScript file to bytecode and save to .qbc file
fn compile_to_bytecode(script_path: &str) -> Result<(), String> {
    // Read source file
    let source = std::fs::read_to_string(script_path)
        .map_err(|e| format!("Error reading {}: {}", script_path, e))?;

    // Compile
    let ctx = Context::new(1024 * 1024);
    let bytecode = ctx.compile(&source)
        .map_err(|e| format!("Compile error: {}", e))?;

    // Serialize
    let mut output = Vec::new();
    output.extend_from_slice(BYTECODE_MAGIC);
    output.push(BYTECODE_VERSION);
    let serialized = bytecode.serialize();
    output.extend_from_slice(&serialized);

    // Write to .qbc file
    let output_path = if script_path.ends_with(".js") {
        script_path.replace(".js", ".qbc")
    } else {
        format!("{}.qbc", script_path)
    };

    std::fs::write(&output_path, &output)
        .map_err(|e| format!("Error writing {}: {}", output_path, e))?;

    println!("Compiled {} -> {} ({} bytes)", script_path, output_path, output.len());
    Ok(())
}

/// Load and execute a bytecode file
fn run_bytecode_file(ctx: &mut Context, filename: &str) -> Result<(), String> {
    use mquickjs::FunctionBytecode;

    // Read bytecode file
    let data = std::fs::read(filename)
        .map_err(|e| format!("Error reading {}: {}", filename, e))?;

    // Verify magic and version
    if data.len() < 5 {
        return Err("Invalid bytecode file: too short".to_string());
    }
    if &data[0..4] != BYTECODE_MAGIC {
        return Err("Invalid bytecode file: bad magic".to_string());
    }
    if data[4] != BYTECODE_VERSION {
        return Err(format!(
            "Unsupported bytecode version: {} (expected {})",
            data[4], BYTECODE_VERSION
        ));
    }

    // Deserialize
    let (bytecode, _) = FunctionBytecode::deserialize(&data[5..])
        .map_err(|e| format!("Error loading bytecode: {}", e))?;

    // Execute
    match ctx.execute(&bytecode) {
        Ok(result) => {
            if !result.is_undefined() {
                println!("{}", result);
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}
