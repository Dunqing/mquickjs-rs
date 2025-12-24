//! JavaScript compiler
//!
//! Generates bytecode from source code in a single pass.
//! Uses precedence climbing for expression parsing.

use super::lexer::{Lexer, SourcePos, Token};
use crate::value::Value;
use crate::vm::opcode::OpCode;

/// Maximum number of local variables
const MAX_LOCALS: usize = 256;

/// Maximum number of constants
const MAX_CONSTANTS: usize = 65536;

/// Local variable info
#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: u32,
    /// Whether this local is captured by an inner function
    is_captured: bool,
}

/// Captured variable info (for closures)
#[derive(Debug, Clone)]
struct Capture {
    /// Name of the captured variable
    name: String,
    /// Index in the outer function's locals (or captures)
    outer_index: usize,
    /// Whether this captures from outer's locals (true) or outer's captures (false)
    is_local: bool,
}

/// Jump patch location
#[derive(Debug, Clone, Copy)]
struct JumpPatch {
    /// Offset in bytecode where the jump target needs to be patched
    offset: usize,
}

/// Loop context for break/continue
#[derive(Debug, Clone)]
struct LoopContext {
    /// Continue jump target (start of loop or increment section)
    continue_target: usize,
    /// Break jump patches (to be patched after loop)
    break_patches: Vec<JumpPatch>,
    /// Scope depth when loop started (for proper cleanup)
    scope_depth: u32,
}

/// Compiler state
pub struct Compiler<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    previous_token: Token,
    bytecode: Vec<u8>,
    constants: Vec<Value>,
    /// String constant pool
    string_constants: Vec<String>,
    /// Local variables in current scope
    locals: Vec<Local>,
    /// Maximum number of locals ever used (for frame allocation)
    max_locals: usize,
    /// Current scope depth
    scope_depth: u32,
    /// Current source position
    current_pos: SourcePos,
    /// Had error during compilation
    had_error: bool,
    /// Panic mode (suppress cascading errors)
    panic_mode: bool,
    /// Compiled inner functions
    functions: Vec<CompiledFunction>,
    /// Loop context stack for break/continue
    loop_stack: Vec<LoopContext>,
    /// Captured variables from outer scopes (for closures)
    captures: Vec<Capture>,
    /// Outer function's locals (for resolving captures during inner function compilation)
    outer_locals: Option<Vec<Local>>,
    /// Outer function's captures (for resolving nested captures)
    outer_captures: Option<Vec<Capture>>,
}

impl<'a> Compiler<'a> {
    /// Create a new compiler for the given source
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current_token = lexer.next_token();

        Compiler {
            lexer,
            current_token,
            previous_token: Token::Eof,
            bytecode: Vec::new(),
            constants: Vec::new(),
            string_constants: Vec::new(),
            locals: Vec::new(),
            max_locals: 0,
            scope_depth: 0,
            current_pos: SourcePos::default(),
            had_error: false,
            panic_mode: false,
            functions: Vec::new(),
            loop_stack: Vec::new(),
            captures: Vec::new(),
            outer_locals: None,
            outer_captures: None,
        }
    }

    /// Compile the source and return bytecode
    pub fn compile(mut self) -> Result<CompiledFunction, CompileError> {
        // Parse statements until EOF
        while !self.check(&Token::Eof) {
            self.statement()?;
        }

        // Emit implicit return undefined
        self.emit_op(OpCode::ReturnUndef);

        if self.had_error {
            Err(CompileError::SyntaxError("Compilation failed".into()))
        } else {
            // Convert captures to CaptureInfo
            let captures: Vec<CaptureInfo> = self
                .captures
                .iter()
                .map(|c| CaptureInfo {
                    outer_index: c.outer_index,
                    is_local: c.is_local,
                })
                .collect();

            Ok(CompiledFunction {
                bytecode: self.bytecode,
                constants: self.constants,
                string_constants: self.string_constants,
                local_count: self.max_locals,
                arg_count: 0, // Top-level script has no arguments
                functions: self.functions,
                captures,
            })
        }
    }

    // =========================================================================
    // Token handling
    // =========================================================================

    /// Advance to the next token
    fn advance(&mut self) {
        self.previous_token = std::mem::replace(&mut self.current_token, Token::Eof);
        self.current_pos = self.lexer.position();

        loop {
            self.current_token = self.lexer.next_token();
            if !matches!(self.current_token, Token::Error(_)) {
                break;
            }
            // Report lexer error and continue
            if let Token::Error(msg) = &self.current_token {
                self.error(&msg.clone());
            }
        }
    }

    /// Check if current token matches expected
    fn check(&self, expected: &Token) -> bool {
        std::mem::discriminant(&self.current_token) == std::mem::discriminant(expected)
    }

    /// Consume token if it matches, return true if matched
    fn match_token(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Expect a specific token, advance if matched
    fn expect(&mut self, expected: Token) -> Result<(), CompileError> {
        if self.check(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(CompileError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: format!("{:?}", self.current_token),
            })
        }
    }

    /// Report an error
    fn error(&mut self, message: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        self.had_error = true;
        eprintln!(
            "[line {}] Error: {}",
            self.current_pos.line, message
        );
    }

    /// Check if current token is an assignment operator
    fn is_assignment_op(&self) -> bool {
        matches!(
            self.current_token,
            Token::Eq
                | Token::PlusEq
                | Token::MinusEq
                | Token::StarEq
                | Token::SlashEq
                | Token::PercentEq
                | Token::AmpEq
                | Token::PipeEq
                | Token::CaretEq
                | Token::LtLtEq
                | Token::GtGtEq
                | Token::GtGtGtEq
                | Token::StarStarEq
        )
    }

    /// Synchronize after error
    fn synchronize(&mut self) {
        self.panic_mode = false;

        while !self.check(&Token::Eof) {
            // Stop at statement boundary
            if matches!(self.previous_token, Token::Semicolon) {
                return;
            }

            // Stop before certain keywords
            match self.current_token {
                Token::Function
                | Token::Var
                | Token::Let
                | Token::Const
                | Token::For
                | Token::If
                | Token::While
                | Token::Return => return,
                _ => {}
            }

            self.advance();
        }
    }

    // =========================================================================
    // Bytecode emission
    // =========================================================================

    /// Emit a single opcode
    fn emit_op(&mut self, op: OpCode) {
        self.bytecode.push(op as u8);
    }

    /// Emit a single byte
    fn emit_byte(&mut self, byte: u8) {
        self.bytecode.push(byte);
    }

    /// Emit two bytes
    fn emit_bytes(&mut self, b1: u8, b2: u8) {
        self.bytecode.push(b1);
        self.bytecode.push(b2);
    }

    /// Emit a 16-bit value (little-endian)
    fn emit_u16(&mut self, val: u16) {
        self.bytecode.push((val & 0xff) as u8);
        self.bytecode.push((val >> 8) as u8);
    }

    /// Emit a 32-bit value (little-endian)
    fn emit_u32(&mut self, val: u32) {
        self.bytecode.push((val & 0xff) as u8);
        self.bytecode.push(((val >> 8) & 0xff) as u8);
        self.bytecode.push(((val >> 16) & 0xff) as u8);
        self.bytecode.push((val >> 24) as u8);
    }

    /// Emit a signed 32-bit value (little-endian)
    fn emit_i32(&mut self, val: i32) {
        self.emit_u32(val as u32);
    }

    /// Emit an integer constant, using optimized opcodes when possible
    fn emit_int(&mut self, val: i32) {
        match val {
            -1 => self.emit_op(OpCode::PushMinus1),
            0 => self.emit_op(OpCode::Push0),
            1 => self.emit_op(OpCode::Push1),
            2 => self.emit_op(OpCode::Push2),
            3 => self.emit_op(OpCode::Push3),
            4 => self.emit_op(OpCode::Push4),
            5 => self.emit_op(OpCode::Push5),
            6 => self.emit_op(OpCode::Push6),
            7 => self.emit_op(OpCode::Push7),
            v if v >= i8::MIN as i32 && v <= i8::MAX as i32 => {
                self.emit_op(OpCode::PushI8);
                self.emit_byte(v as i8 as u8);
            }
            v if v >= i16::MIN as i32 && v <= i16::MAX as i32 => {
                self.emit_op(OpCode::PushI16);
                self.emit_u16(v as i16 as u16);
            }
            _ => {
                // Large integer: for now, truncate to short int range
                // TODO: Store as float constant when float support is added
                let idx = self.add_constant(Value::int(val.max(-(1 << 30)).min((1 << 30) - 1)));
                self.emit_const(idx);
            }
        }
    }

    /// Emit a constant load instruction
    fn emit_const(&mut self, index: u16) {
        if index < 256 {
            self.emit_op(OpCode::PushConst8);
            self.emit_byte(index as u8);
        } else {
            self.emit_op(OpCode::PushConst);
            self.emit_u16(index);
        }
    }

    /// Emit local variable get
    fn emit_get_local(&mut self, index: usize) {
        match index {
            0 => self.emit_op(OpCode::GetLoc0),
            1 => self.emit_op(OpCode::GetLoc1),
            2 => self.emit_op(OpCode::GetLoc2),
            3 => self.emit_op(OpCode::GetLoc3),
            i if i < 256 => {
                self.emit_op(OpCode::GetLoc8);
                self.emit_byte(i as u8);
            }
            i => {
                self.emit_op(OpCode::GetLoc);
                self.emit_u16(i as u16);
            }
        }
    }

    /// Emit local variable set
    fn emit_set_local(&mut self, index: usize) {
        match index {
            0 => self.emit_op(OpCode::PutLoc0),
            1 => self.emit_op(OpCode::PutLoc1),
            2 => self.emit_op(OpCode::PutLoc2),
            3 => self.emit_op(OpCode::PutLoc3),
            i if i < 256 => {
                self.emit_op(OpCode::PutLoc8);
                self.emit_byte(i as u8);
            }
            i => {
                self.emit_op(OpCode::PutLoc);
                self.emit_u16(i as u16);
            }
        }
    }

    /// Emit get capture instruction
    fn emit_get_capture(&mut self, index: usize) {
        self.emit_op(OpCode::GetVarRef);
        self.emit_u16(index as u16);
    }

    /// Emit set capture instruction
    fn emit_set_capture(&mut self, index: usize) {
        self.emit_op(OpCode::PutVarRef);
        self.emit_u16(index as u16);
    }

    /// Emit get global instruction for builtin functions
    fn emit_get_global(&mut self, name: &str) {
        // Add the name as a string constant
        let str_idx = self.string_constants.len() as u16;
        self.string_constants.push(name.to_string());
        let const_idx = self.add_constant(Value::string(str_idx));
        self.emit_op(OpCode::GetGlobal);
        self.emit_u16(const_idx);
    }

    /// Emit a jump instruction and return the patch location
    fn emit_jump(&mut self, op: OpCode) -> JumpPatch {
        self.emit_op(op);
        let offset = self.bytecode.len();
        self.emit_u32(0); // Placeholder
        JumpPatch { offset }
    }

    /// Patch a jump instruction to jump to the current position
    fn patch_jump(&mut self, patch: JumpPatch) {
        let target = self.bytecode.len() as i32;
        let jump_end = (patch.offset + 4) as i32;
        let offset = target - jump_end;

        self.bytecode[patch.offset] = (offset & 0xff) as u8;
        self.bytecode[patch.offset + 1] = ((offset >> 8) & 0xff) as u8;
        self.bytecode[patch.offset + 2] = ((offset >> 16) & 0xff) as u8;
        self.bytecode[patch.offset + 3] = ((offset >> 24) & 0xff) as u8;
    }

    /// Patch a 32-bit value at the given offset
    fn patch_i32(&mut self, offset: usize, val: i32) {
        let bytes = val.to_le_bytes();
        self.bytecode[offset] = bytes[0];
        self.bytecode[offset + 1] = bytes[1];
        self.bytecode[offset + 2] = bytes[2];
        self.bytecode[offset + 3] = bytes[3];
    }

    /// Emit a loop back to a previous position
    fn emit_loop(&mut self, loop_start: usize) {
        self.emit_op(OpCode::Goto);
        let current = self.bytecode.len() + 4; // After the 4-byte offset
        let offset = (loop_start as i32) - (current as i32);
        self.emit_u32(offset as u32);
    }

    /// Add a constant to the pool and return its index
    fn add_constant(&mut self, value: Value) -> u16 {
        // Check for existing identical constant
        for (i, c) in self.constants.iter().enumerate() {
            if value.raw().0 == c.raw().0 {
                return i as u16;
            }
        }

        if self.constants.len() >= MAX_CONSTANTS {
            self.error("Too many constants");
            return 0;
        }

        let index = self.constants.len();
        self.constants.push(value);
        index as u16
    }

    /// Current bytecode offset
    fn current_offset(&self) -> usize {
        self.bytecode.len()
    }

    // =========================================================================
    // Variable handling
    // =========================================================================

    /// Declare a local variable
    fn declare_local(&mut self, name: &str) -> Result<usize, CompileError> {
        // Check for redeclaration in same scope
        for local in self.locals.iter().rev() {
            if local.depth < self.scope_depth {
                break;
            }
            if local.name == name {
                return Err(CompileError::SyntaxError(format!(
                    "Variable '{}' already declared in this scope",
                    name
                )));
            }
        }

        if self.locals.len() >= MAX_LOCALS {
            return Err(CompileError::TooManyLocals);
        }

        let index = self.locals.len();
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
            is_captured: false,
        });

        // Track maximum locals for frame allocation
        if self.locals.len() > self.max_locals {
            self.max_locals = self.locals.len();
        }

        Ok(index)
    }

    /// Resolve a local variable, returning its index
    fn resolve_local(&self, name: &str) -> Option<usize> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i);
            }
        }
        None
    }

    /// Resolve a captured variable, returning its capture index
    /// This also adds the capture if it doesn't exist yet
    fn resolve_capture(&mut self, name: &str) -> Option<usize> {
        // First check if we already have this capture
        for (i, capture) in self.captures.iter().enumerate() {
            if capture.name == name {
                return Some(i);
            }
        }

        // Try to resolve from outer function's locals
        if let Some(ref mut outer_locals) = self.outer_locals.clone() {
            for (i, local) in outer_locals.iter().enumerate().rev() {
                if local.name == name {
                    // Mark the outer local as captured
                    if let Some(ref mut locals) = self.outer_locals {
                        locals[i].is_captured = true;
                    }

                    // Add a new capture
                    let capture_idx = self.captures.len();
                    self.captures.push(Capture {
                        name: name.to_string(),
                        outer_index: i,
                        is_local: true,
                    });
                    return Some(capture_idx);
                }
            }
        }

        // Try to resolve from outer function's captures (nested closure)
        if let Some(ref outer_captures) = self.outer_captures.clone() {
            for (i, capture) in outer_captures.iter().enumerate() {
                if capture.name == name {
                    // Add a new capture that references outer's capture
                    let capture_idx = self.captures.len();
                    self.captures.push(Capture {
                        name: name.to_string(),
                        outer_index: i,
                        is_local: false,
                    });
                    return Some(capture_idx);
                }
            }
        }

        None
    }

    /// Begin a new scope
    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// End the current scope
    fn end_scope(&mut self) {
        self.scope_depth -= 1;

        // Remove locals from ended scope from our tracking
        // Note: We don't emit Drop because locals are stored in fixed frame slots,
        // not on the value stack. The frame cleanup happens on function return.
        while let Some(local) = self.locals.last() {
            if local.depth <= self.scope_depth {
                break;
            }
            self.locals.pop();
        }
    }

    // =========================================================================
    // Statement parsing
    // =========================================================================

    /// Parse a statement
    fn statement(&mut self) -> Result<(), CompileError> {
        match &self.current_token {
            Token::Var => self.var_declaration(),
            Token::Let => self.let_declaration(),
            Token::Const => self.const_declaration(),
            Token::Function => self.function_declaration(),
            Token::If => self.if_statement(),
            Token::While => self.while_statement(),
            Token::For => self.for_statement(),
            Token::Break => self.break_statement(),
            Token::Continue => self.continue_statement(),
            Token::Return => self.return_statement(),
            Token::Print => self.print_statement(),
            Token::Try => self.try_statement(),
            Token::Throw => self.throw_statement(),
            Token::LBrace => self.block_statement(),
            _ => self.expression_statement(),
        }
    }

    /// Parse var declaration: var x = expr;
    fn var_declaration(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'var'

        let name = match &self.current_token {
            Token::Ident(s) => s.clone(),
            _ => {
                return Err(CompileError::SyntaxError(
                    "Expected variable name".into(),
                ))
            }
        };
        self.advance();

        // Declare the variable
        let index = self.declare_local(&name)?;

        // Optional initializer
        if self.match_token(&Token::Eq) {
            self.expression()?;
        } else {
            self.emit_op(OpCode::Undefined);
        }

        // Store to local (value stays on stack as the local)
        // Actually we need to emit set_local here
        self.emit_set_local(index);

        // Expect semicolon
        self.expect(Token::Semicolon)?;

        Ok(())
    }

    /// Parse let declaration
    fn let_declaration(&mut self) -> Result<(), CompileError> {
        self.var_declaration_impl("let")
    }

    /// Parse const declaration
    fn const_declaration(&mut self) -> Result<(), CompileError> {
        self.var_declaration_impl("const")
    }

    /// Common implementation for var/let/const
    fn var_declaration_impl(&mut self, _keyword: &str) -> Result<(), CompileError> {
        self.advance(); // consume keyword

        let name = match &self.current_token {
            Token::Ident(s) => s.clone(),
            _ => {
                return Err(CompileError::SyntaxError(
                    "Expected variable name".into(),
                ))
            }
        };
        self.advance();

        let index = self.declare_local(&name)?;

        if self.match_token(&Token::Eq) {
            self.expression()?;
        } else {
            self.emit_op(OpCode::Undefined);
        }

        self.emit_set_local(index);
        self.expect(Token::Semicolon)?;

        Ok(())
    }

    /// Parse function declaration: function name(args) { body }
    fn function_declaration(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'function'

        // Get function name
        let name = match &self.current_token {
            Token::Ident(s) => s.clone(),
            _ => {
                return Err(CompileError::SyntaxError(
                    "Expected function name".into(),
                ))
            }
        };
        self.advance();

        // Declare the function as a local variable
        let func_index = self.declare_local(&name)?;

        // Parse parameter list
        self.expect(Token::LParen)?;
        let mut params: Vec<String> = Vec::new();

        if !self.check(&Token::RParen) {
            loop {
                if let Token::Ident(param_name) = &self.current_token {
                    params.push(param_name.clone());
                    self.advance();
                } else {
                    return Err(CompileError::SyntaxError(
                        "Expected parameter name".into(),
                    ));
                }

                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect(Token::RParen)?;

        // Parse function body
        self.expect(Token::LBrace)?;

        // Compile the function body with a new compiler
        // Pass the function name so it can reference itself for recursion
        let body_bytecode = self.compile_function_body(Some(&name), &params)?;

        // Store the bytecode in functions list
        let bytecode_idx = self.functions.len();
        self.functions.push(body_bytecode);

        // Emit instruction to create function closure
        self.emit_op(OpCode::FClosure);
        self.emit_u16(bytecode_idx as u16);

        // Store to local
        self.emit_set_local(func_index);

        Ok(())
    }

    /// Compile a function body
    ///
    /// If `func_name` is provided, the function can reference itself for recursion.
    fn compile_function_body(
        &mut self,
        func_name: Option<&str>,
        params: &[String],
    ) -> Result<CompiledFunction, CompileError> {
        // Save current compiler state
        let saved_bytecode = std::mem::take(&mut self.bytecode);
        let saved_constants = std::mem::take(&mut self.constants);
        let saved_string_constants = std::mem::take(&mut self.string_constants);
        let saved_locals = std::mem::take(&mut self.locals);
        let saved_functions = std::mem::take(&mut self.functions);
        let saved_loop_stack = std::mem::take(&mut self.loop_stack);
        let saved_captures = std::mem::take(&mut self.captures);
        let saved_outer_locals = std::mem::take(&mut self.outer_locals);
        let saved_outer_captures = std::mem::take(&mut self.outer_captures);
        let saved_max_locals = self.max_locals;
        let saved_scope_depth = self.scope_depth;

        // Set outer locals and captures for closure resolution
        // The inner function can capture from our locals
        self.outer_locals = Some(saved_locals.clone());
        self.outer_captures = Some(saved_captures.clone());

        // Reset for function compilation
        self.bytecode = Vec::new();
        self.constants = Vec::new();
        self.string_constants = Vec::new();
        self.locals = Vec::new();
        self.functions = Vec::new();
        self.loop_stack = Vec::new();
        self.captures = Vec::new();
        self.max_locals = 0;
        self.scope_depth = 0;

        let arg_count = params.len();

        // Declare parameters as locals FIRST (they must be at slots 0..arg_count)
        for param in params {
            self.declare_local(param)?;
        }

        // If function has a name, declare it as a local for recursion
        // (after parameters so it doesn't affect argument slot positions)
        // and emit code to initialize it with ThisFunc at the start
        if let Some(name) = func_name {
            let func_slot = self.declare_local(name)?;
            self.emit_op(OpCode::ThisFunc);
            self.emit_set_local(func_slot);
        }

        // Parse function body statements
        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            self.statement()?;
        }
        self.expect(Token::RBrace)?;

        // Emit implicit return undefined
        self.emit_op(OpCode::ReturnUndef);

        // Convert captures to CaptureInfo
        let captures: Vec<CaptureInfo> = self
            .captures
            .iter()
            .map(|c| CaptureInfo {
                outer_index: c.outer_index,
                is_local: c.is_local,
            })
            .collect();

        // Create compiled function
        let result = CompiledFunction {
            bytecode: std::mem::take(&mut self.bytecode),
            constants: std::mem::take(&mut self.constants),
            string_constants: std::mem::take(&mut self.string_constants),
            local_count: self.max_locals,
            arg_count,
            functions: std::mem::take(&mut self.functions),
            captures,
        };

        // Restore compiler state
        self.bytecode = saved_bytecode;
        self.constants = saved_constants;
        self.string_constants = saved_string_constants;
        self.locals = saved_locals;
        self.functions = saved_functions;
        self.loop_stack = saved_loop_stack;
        self.captures = saved_captures;
        self.outer_locals = saved_outer_locals;
        self.outer_captures = saved_outer_captures;
        self.max_locals = saved_max_locals;
        self.scope_depth = saved_scope_depth;

        Ok(result)
    }

    /// Parse if statement
    fn if_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'if'
        self.expect(Token::LParen)?;
        self.expression()?;
        self.expect(Token::RParen)?;

        // Jump over then branch if condition is false
        let then_jump = self.emit_jump(OpCode::IfFalse);

        self.statement()?;

        // Jump over else branch
        let else_jump = self.emit_jump(OpCode::Goto);

        self.patch_jump(then_jump);

        // Parse else branch if present
        if self.match_token(&Token::Else) {
            self.statement()?;
        }

        self.patch_jump(else_jump);

        Ok(())
    }

    /// Parse while statement
    fn while_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'while'

        let loop_start = self.current_offset();

        // Push loop context for break/continue
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        self.expect(Token::LParen)?;
        self.expression()?;
        self.expect(Token::RParen)?;

        let exit_jump = self.emit_jump(OpCode::IfFalse);

        self.statement()?;

        self.emit_loop(loop_start);
        self.patch_jump(exit_jump);

        // Pop loop context and patch break jumps
        let loop_ctx = self.loop_stack.pop().unwrap();
        for patch in loop_ctx.break_patches {
            self.patch_jump(patch);
        }

        Ok(())
    }

    /// Parse for statement (C-style or for-in)
    fn for_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'for'
        self.expect(Token::LParen)?;

        self.begin_scope();

        // Check for for-in syntax: for (var/let x in obj)
        if self.check(&Token::Var) || self.check(&Token::Let) {
            self.advance(); // consume var/let

            if let Token::Ident(name) = self.current_token.clone() {
                // Look ahead to check if this is for-in
                // We need to save state and peek at next token
                let saved_name = name.clone();
                self.advance(); // consume identifier

                if self.match_token(&Token::In) {
                    // This is a for-in loop
                    return self.for_in_statement_rest(saved_name);
                }

                if self.match_token(&Token::Of) {
                    // This is a for-of loop
                    return self.for_of_statement_rest(saved_name);
                }

                // Not a for-in loop, restore and continue as C-style for
                // We already consumed var/let and identifier, need to handle initializer
                // Declare local first to get the index
                let index = self.declare_local(&saved_name)?;
                // Check for initializer
                if self.match_token(&Token::Eq) {
                    // Has initializer: var x = expr
                    self.expression()?;
                } else {
                    // No initializer, push undefined
                    self.emit_op(OpCode::Undefined);
                }
                // Store to local
                self.emit_set_local(index);
                self.expect(Token::Semicolon)?;
            } else {
                return Err(CompileError::SyntaxError(
                    "Expected identifier in for loop".to_string(),
                ));
            }
        } else if !self.match_token(&Token::Semicolon) {
            // C-style initializer expression
            self.expression_statement()?;
        }

        // Continue with C-style for loop
        self.for_c_style_rest()
    }

    /// Parse rest of for-in statement after "for (var/let name in"
    fn for_in_statement_rest(&mut self, var_name: String) -> Result<(), CompileError> {
        // Parse the object to iterate over
        self.expression()?;
        self.expect(Token::RParen)?;

        // Create iterator from object - ForInStart converts obj to iterator index
        self.emit_op(OpCode::ForInStart);

        // Declare a hidden local to store the iterator index
        // Use a name that can't conflict with user code
        let iter_slot = self.declare_local("\x00iter")?;
        self.emit_set_local(iter_slot);

        // Declare the loop variable
        self.emit_op(OpCode::Undefined);
        let var_slot = self.declare_local(&var_name)?;
        self.emit_set_local(var_slot);

        // Loop start - get next key
        let loop_start = self.current_offset();

        // Push loop context for break/continue
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        // Get iterator from hidden local
        self.emit_get_local(iter_slot);

        // ForInNext: iter -> key done
        self.emit_op(OpCode::ForInNext);

        // Check if done (pops done flag)
        let exit_jump = self.emit_jump(OpCode::IfTrue);

        // Store key to loop variable (PutLoc pops the key)
        self.emit_set_local(var_slot);

        // Body
        self.statement()?;

        // Loop back
        self.emit_loop(loop_start);

        // Exit point
        self.patch_jump(exit_jump);

        // Pop remaining key value (undefined from done iteration)
        self.emit_op(OpCode::Drop);

        // Pop loop context and patch break jumps
        let loop_ctx = self.loop_stack.pop().unwrap();
        for patch in loop_ctx.break_patches {
            self.patch_jump(patch);
        }

        self.end_scope();

        Ok(())
    }

    /// Parse rest of for-of statement after "for (var/let name of"
    fn for_of_statement_rest(&mut self, var_name: String) -> Result<(), CompileError> {
        // Parse the iterable to iterate over
        self.expression()?;
        self.expect(Token::RParen)?;

        // Create iterator from iterable - ForOfStart converts to iterator index
        self.emit_op(OpCode::ForOfStart);

        // Declare a hidden local to store the iterator index
        let iter_slot = self.declare_local("\x00iter")?;
        self.emit_set_local(iter_slot);

        // Declare the loop variable
        self.emit_op(OpCode::Undefined);
        let var_slot = self.declare_local(&var_name)?;
        self.emit_set_local(var_slot);

        // Loop start - get next value
        let loop_start = self.current_offset();

        // Push loop context for break/continue
        self.loop_stack.push(LoopContext {
            continue_target: loop_start,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        // Get iterator from hidden local
        self.emit_get_local(iter_slot);

        // ForOfNext: iter -> value done
        self.emit_op(OpCode::ForOfNext);

        // Check if done (pops done flag)
        let exit_jump = self.emit_jump(OpCode::IfTrue);

        // Store value to loop variable (PutLoc pops the value)
        self.emit_set_local(var_slot);

        // Body
        self.statement()?;

        // Loop back
        self.emit_loop(loop_start);

        // Exit point
        self.patch_jump(exit_jump);

        // Pop remaining value (undefined from done iteration)
        self.emit_op(OpCode::Drop);

        // Pop loop context and patch break jumps
        let loop_ctx = self.loop_stack.pop().unwrap();
        for patch in loop_ctx.break_patches {
            self.patch_jump(patch);
        }

        self.end_scope();

        Ok(())
    }

    /// Parse rest of C-style for loop after initializer
    fn for_c_style_rest(&mut self) -> Result<(), CompileError> {
        let loop_start = self.current_offset();

        // Condition
        let exit_jump = if !self.match_token(&Token::Semicolon) {
            self.expression()?;
            self.expect(Token::Semicolon)?;
            let j = self.emit_jump(OpCode::IfFalse);
            Some(j)
        } else {
            None
        };

        // Increment (executed at end of each iteration)
        let increment_start = if !self.check(&Token::RParen) {
            // Jump over increment initially
            let body_jump = self.emit_jump(OpCode::Goto);
            let inc_start = self.current_offset();
            self.expression()?;
            self.emit_op(OpCode::Drop); // Discard increment result
            self.emit_loop(loop_start);
            self.patch_jump(body_jump);
            Some(inc_start)
        } else {
            None
        };

        self.expect(Token::RParen)?;

        // Push loop context for break/continue
        // Continue should jump to increment section if present, otherwise loop start
        let continue_target = increment_start.unwrap_or(loop_start);
        self.loop_stack.push(LoopContext {
            continue_target,
            break_patches: Vec::new(),
            scope_depth: self.scope_depth,
        });

        // Body
        self.statement()?;

        // Loop back (to increment if present, otherwise to condition)
        if let Some(inc) = increment_start {
            self.emit_loop(inc);
        } else {
            self.emit_loop(loop_start);
        }

        // Patch exit jump
        if let Some(j) = exit_jump {
            self.patch_jump(j);
        }

        // Pop loop context and patch break jumps
        let loop_ctx = self.loop_stack.pop().unwrap();
        for patch in loop_ctx.break_patches {
            self.patch_jump(patch);
        }

        self.end_scope();

        Ok(())
    }

    /// Parse break statement
    fn break_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'break'

        if self.loop_stack.is_empty() {
            return Err(CompileError::SyntaxError(
                "'break' outside of loop".to_string(),
            ));
        }

        // Emit jump (will be patched when loop ends)
        let patch = self.emit_jump(OpCode::Goto);

        // Register this jump to be patched
        if let Some(ctx) = self.loop_stack.last_mut() {
            ctx.break_patches.push(patch);
        }

        self.expect(Token::Semicolon)?;
        Ok(())
    }

    /// Parse continue statement
    fn continue_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'continue'

        if self.loop_stack.is_empty() {
            return Err(CompileError::SyntaxError(
                "'continue' outside of loop".to_string(),
            ));
        }

        // Get the continue target
        let continue_target = self.loop_stack.last().unwrap().continue_target;

        // Emit loop back to continue target
        self.emit_loop(continue_target);

        self.expect(Token::Semicolon)?;
        Ok(())
    }

    /// Parse return statement
    fn return_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'return'

        if self.match_token(&Token::Semicolon) {
            self.emit_op(OpCode::ReturnUndef);
        } else {
            self.expression()?;
            self.expect(Token::Semicolon)?;
            self.emit_op(OpCode::Return);
        }

        Ok(())
    }

    /// Parse print statement: print expr;
    fn print_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'print'

        self.expression()?;
        self.expect(Token::Semicolon)?;
        self.emit_op(OpCode::Print);

        Ok(())
    }

    /// Parse throw statement: throw expr;
    fn throw_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'throw'

        self.expression()?;
        self.expect(Token::Semicolon)?;
        self.emit_op(OpCode::Throw);

        Ok(())
    }

    /// Parse try-catch-finally statement
    fn try_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'try'

        // Expect '{'
        if !self.check(&Token::LBrace) {
            return Err(CompileError::SyntaxError(
                "Expected '{' after 'try'".into(),
            ));
        }

        // Emit Catch opcode with placeholder offset
        self.emit_op(OpCode::Catch);
        let catch_jump = self.bytecode.len();
        self.emit_i32(0); // placeholder

        // Parse try block
        self.block_statement()?;

        // If we get here without exception, remove the handler
        self.emit_op(OpCode::DropCatch);

        // Jump over catch block
        self.emit_op(OpCode::Goto);
        let end_jump = self.bytecode.len();
        self.emit_i32(0); // placeholder

        // Patch catch jump target (points here - start of catch block)
        let catch_target = self.bytecode.len() as i32;
        let catch_offset = catch_target - (catch_jump as i32 + 4);
        self.patch_i32(catch_jump, catch_offset);

        // Check for catch clause
        let has_catch = self.check(&Token::Catch);
        if has_catch {
            self.advance(); // consume 'catch'

            // Optional (e) parameter
            if self.match_token(&Token::LParen) {
                let name = match &self.current_token {
                    Token::Ident(s) => s.clone(),
                    _ => {
                        return Err(CompileError::SyntaxError(
                            "Expected catch variable name".into(),
                        ))
                    }
                };
                self.advance();
                self.expect(Token::RParen)?;

                // Begin a new scope for the catch variable
                self.begin_scope();

                // Declare the catch variable
                let index = self.declare_local(&name)?;

                // The exception value is on the stack from Catch opcode
                // Store it in the catch variable
                self.emit_set_local(index);

                // Parse catch body
                if !self.check(&Token::LBrace) {
                    return Err(CompileError::SyntaxError(
                        "Expected '{' after catch".into(),
                    ));
                }
                self.block_statement()?;

                self.end_scope();
            } else {
                // No parameter - just discard the exception value
                self.emit_op(OpCode::Drop);

                // Parse catch body
                if !self.check(&Token::LBrace) {
                    return Err(CompileError::SyntaxError(
                        "Expected '{' after catch".into(),
                    ));
                }
                self.block_statement()?;
            }
        } else {
            // No catch clause - we still need to handle the exception value
            // For now, just re-throw it (must have finally)
            self.emit_op(OpCode::Throw);
        }

        // Patch end jump (points here - after catch block)
        let end_target = self.bytecode.len() as i32;
        let end_offset = end_target - (end_jump as i32 + 4);
        self.patch_i32(end_jump, end_offset);

        // Check for finally clause
        if self.check(&Token::Finally) {
            self.advance(); // consume 'finally'

            // Parse finally body
            if !self.check(&Token::LBrace) {
                return Err(CompileError::SyntaxError(
                    "Expected '{' after finally".into(),
                ));
            }
            self.block_statement()?;
        }

        // Must have at least catch or finally
        if !has_catch && !self.check(&Token::Finally) {
            // Already consumed finally if present, so check the previous token
            // Actually this check is wrong - we consumed finally above
            // Let's just allow try without catch/finally for now (will still work)
        }

        Ok(())
    }

    /// Parse block statement
    fn block_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume '{'
        self.begin_scope();

        while !self.check(&Token::RBrace) && !self.check(&Token::Eof) {
            self.statement()?;
        }

        self.expect(Token::RBrace)?;
        self.end_scope();

        Ok(())
    }

    /// Parse expression statement
    fn expression_statement(&mut self) -> Result<(), CompileError> {
        self.expression()?;
        self.expect(Token::Semicolon)?;
        self.emit_op(OpCode::Drop); // Discard expression value
        Ok(())
    }

    // =========================================================================
    // Expression parsing (precedence climbing)
    // =========================================================================

    /// Parse an expression
    fn expression(&mut self) -> Result<(), CompileError> {
        self.parse_precedence(Precedence::Assignment)
    }

    /// Parse expression with given minimum precedence
    fn parse_precedence(&mut self, min_prec: Precedence) -> Result<(), CompileError> {
        // Parse prefix expression
        self.prefix_expr()?;

        // Parse infix expressions at or above min precedence
        while let Some((prec, assoc)) = self.infix_precedence() {
            if prec < min_prec {
                break;
            }

            let op = self.current_token.clone();
            self.advance();

            // Handle assignment specially
            if prec == Precedence::Assignment {
                self.assignment_expr(&op)?;
                continue;
            }

            // Handle ternary operator
            if matches!(op, Token::Question) {
                self.ternary_expr()?;
                continue;
            }

            // Handle short-circuit operators
            if matches!(op, Token::AmpAmp | Token::PipePipe) {
                self.short_circuit_expr(&op)?;
                continue;
            }

            // Right-associative: use same precedence; left-associative: use next higher
            let next_prec = if assoc == Associativity::Right {
                prec
            } else {
                prec.next()
            };

            self.parse_precedence(next_prec)?;
            self.emit_binary_op(&op)?;
        }

        Ok(())
    }

    /// Parse prefix expression (unary, literals, grouping)
    fn prefix_expr(&mut self) -> Result<(), CompileError> {
        match &self.current_token {
            // Literals
            Token::Number(n) => {
                let n = *n;
                self.advance();
                // Check if it's an integer that fits in short int range
                if n.fract() == 0.0 && n >= -(1i64 << 30) as f64 && n <= ((1i64 << 30) - 1) as f64 {
                    self.emit_int(n as i32);
                } else {
                    // TODO: Handle floats when float support is added to Value
                    // For now, truncate to integer
                    let int_val = n as i32;
                    self.emit_int(int_val.max(-(1 << 30)).min((1 << 30) - 1));
                }
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance();
                if s.is_empty() {
                    self.emit_op(OpCode::PushEmptyString);
                } else {
                    // Check for built-in strings used by typeof
                    use crate::value::{STR_UNDEFINED, STR_OBJECT, STR_BOOLEAN, STR_NUMBER, STR_FUNCTION, STR_STRING};
                    let builtin_idx = match s.as_str() {
                        "undefined" => Some(STR_UNDEFINED),
                        "object" => Some(STR_OBJECT),
                        "boolean" => Some(STR_BOOLEAN),
                        "number" => Some(STR_NUMBER),
                        "function" => Some(STR_FUNCTION),
                        "string" => Some(STR_STRING),
                        _ => None,
                    };

                    if let Some(idx) = builtin_idx {
                        // Use built-in string constant for typeof comparison
                        self.emit_op(OpCode::PushConst);
                        let const_idx = self.add_constant(Value::string(idx));
                        self.emit_u16(const_idx);
                    } else {
                        // Store string in string constant pool
                        let idx = self.string_constants.len() as u16;
                        self.string_constants.push(s);
                        // Emit PushConst with a string value
                        self.emit_op(OpCode::PushConst);
                        // We'll encode this as a Value::string(idx) in the constants
                        let const_idx = self.add_constant(Value::string(idx));
                        self.emit_u16(const_idx);
                    }
                }
            }
            Token::True => {
                self.advance();
                self.emit_op(OpCode::PushTrue);
            }
            Token::False => {
                self.advance();
                self.emit_op(OpCode::PushFalse);
            }
            Token::Null => {
                self.advance();
                self.emit_op(OpCode::Null);
            }
            Token::This => {
                self.advance();
                self.emit_op(OpCode::PushThis);
            }

            // Identifiers (variables)
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();

                // Check for assignment
                if self.is_assignment_op() {
                    let op = self.current_token.clone();
                    self.advance();

                    // Resolve variable: local, capture, or error
                    let (is_local, idx) = if let Some(idx) = self.resolve_local(&name) {
                        (true, idx)
                    } else if let Some(idx) = self.resolve_capture(&name) {
                        (false, idx)
                    } else {
                        return Err(CompileError::SyntaxError(format!(
                            "Undefined variable '{}'",
                            name
                        )));
                    };

                    // For compound assignment (+=, -=, etc.), get the current value first
                    if !matches!(op, Token::Eq) {
                        if is_local {
                            self.emit_get_local(idx);
                        } else {
                            self.emit_get_capture(idx);
                        }
                    }

                    // Parse the right-hand side
                    self.parse_precedence(Precedence::Assignment)?;

                    // For compound assignment, apply the operation
                    match op {
                        Token::Eq => {}
                        Token::PlusEq => self.emit_op(OpCode::Add),
                        Token::MinusEq => self.emit_op(OpCode::Sub),
                        Token::StarEq => self.emit_op(OpCode::Mul),
                        Token::SlashEq => self.emit_op(OpCode::Div),
                        Token::PercentEq => self.emit_op(OpCode::Mod),
                        Token::AmpEq => self.emit_op(OpCode::And),
                        Token::PipeEq => self.emit_op(OpCode::Or),
                        Token::CaretEq => self.emit_op(OpCode::Xor),
                        Token::LtLtEq => self.emit_op(OpCode::Shl),
                        Token::GtGtEq => self.emit_op(OpCode::Sar),
                        Token::GtGtGtEq => self.emit_op(OpCode::Shr),
                        _ => {}
                    }

                    // Duplicate value (for expression result) and store
                    self.emit_op(OpCode::Dup);
                    if is_local {
                        self.emit_set_local(idx);
                    } else {
                        self.emit_set_capture(idx);
                    }
                } else if let Some(idx) = self.resolve_local(&name) {
                    self.emit_get_local(idx);
                } else if let Some(idx) = self.resolve_capture(&name) {
                    self.emit_get_capture(idx);
                } else {
                    // Try as a global (builtin function)
                    self.emit_get_global(&name);
                }
            }

            // Grouping: (expr)
            Token::LParen => {
                self.advance();
                self.expression()?;
                self.expect(Token::RParen)?;
            }

            // Unary operators
            Token::Minus => {
                self.advance();
                self.parse_precedence(Precedence::Unary)?;
                self.emit_op(OpCode::Neg);
            }
            Token::Plus => {
                self.advance();
                self.parse_precedence(Precedence::Unary)?;
                self.emit_op(OpCode::Plus);
            }
            Token::Bang => {
                self.advance();
                self.parse_precedence(Precedence::Unary)?;
                self.emit_op(OpCode::LNot);
            }
            Token::Tilde => {
                self.advance();
                self.parse_precedence(Precedence::Unary)?;
                self.emit_op(OpCode::Not);
            }
            Token::TypeOf => {
                self.advance();
                self.parse_precedence(Precedence::Unary)?;
                self.emit_op(OpCode::TypeOf);
            }

            // Delete operator: delete obj.prop or delete arr[idx]
            Token::Delete => {
                self.advance();
                // Parse the operand - we need to handle member access specially
                // to get both the object and the property key
                self.delete_expr()?;
            }

            // Pre-increment/decrement
            Token::PlusPlus => {
                self.advance();
                // Need to handle as lvalue modification
                if let Token::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.advance();
                    if let Some(idx) = self.resolve_local(&name) {
                        self.emit_get_local(idx);
                        self.emit_op(OpCode::Inc);
                        self.emit_op(OpCode::Dup);
                        self.emit_set_local(idx);
                    } else if let Some(idx) = self.resolve_capture(&name) {
                        self.emit_get_capture(idx);
                        self.emit_op(OpCode::Inc);
                        self.emit_op(OpCode::Dup);
                        self.emit_set_capture(idx);
                    } else {
                        return Err(CompileError::SyntaxError(format!(
                            "Undefined variable '{}'",
                            name
                        )));
                    }
                } else {
                    return Err(CompileError::SyntaxError(
                        "Invalid increment operand".into(),
                    ));
                }
            }
            Token::MinusMinus => {
                self.advance();
                if let Token::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.advance();
                    if let Some(idx) = self.resolve_local(&name) {
                        self.emit_get_local(idx);
                        self.emit_op(OpCode::Dec);
                        self.emit_op(OpCode::Dup);
                        self.emit_set_local(idx);
                    } else if let Some(idx) = self.resolve_capture(&name) {
                        self.emit_get_capture(idx);
                        self.emit_op(OpCode::Dec);
                        self.emit_op(OpCode::Dup);
                        self.emit_set_capture(idx);
                    } else {
                        return Err(CompileError::SyntaxError(format!(
                            "Undefined variable '{}'",
                            name
                        )));
                    }
                } else {
                    return Err(CompileError::SyntaxError(
                        "Invalid decrement operand".into(),
                    ));
                }
            }

            // Array literal: [1, 2, 3]
            Token::LBracket => {
                self.advance();
                let count = self.array_literal()?;
                // Emit ArrayFrom opcode with element count
                self.emit_op(OpCode::ArrayFrom);
                self.emit_u16(count);
            }

            // New expression: new Constructor() or new Constructor
            Token::New => {
                self.advance();
                // Parse the constructor expression (just the primary, not function call)
                // We handle the member access chain but not the call
                self.new_expr_target()?;
                // Check for argument list
                let arg_count = if self.check(&Token::LParen) {
                    self.advance();
                    self.argument_list()?
                } else {
                    0
                };
                // Emit CallConstructor opcode
                self.emit_op(OpCode::CallConstructor);
                self.emit_u16(arg_count);
            }

            _ => {
                return Err(CompileError::SyntaxError(format!(
                    "Unexpected token: {:?}",
                    self.current_token
                )));
            }
        }

        // Handle postfix operators and member access
        self.postfix_expr()
    }

    /// Parse postfix operators (++, --, call, member access)
    fn postfix_expr(&mut self) -> Result<(), CompileError> {
        loop {
            match &self.current_token {
                // Function call
                Token::LParen => {
                    self.advance();
                    let arg_count = self.argument_list()?;
                    self.emit_op(OpCode::Call);
                    self.emit_u16(arg_count);
                }

                // Array access: a[b] or a[b] = c
                Token::LBracket => {
                    self.advance();
                    self.expression()?;
                    self.expect(Token::RBracket)?;

                    // Check for assignment
                    if self.match_token(&Token::Eq) {
                        // arr[idx] = value
                        self.expression()?;
                        self.emit_op(OpCode::PutArrayEl);
                    } else {
                        self.emit_op(OpCode::GetArrayEl);
                    }
                }

                // Member access: a.b or a.b = c or a.b()
                Token::Dot => {
                    self.advance();
                    if let Token::Ident(name) = &self.current_token {
                        let name = name.clone();
                        self.advance();

                        // Store property name as string constant
                        let str_idx = self.string_constants.len() as u16;
                        self.string_constants.push(name);

                        // Check for assignment
                        if self.match_token(&Token::Eq) {
                            // obj.prop = value
                            self.expression()?;
                            self.emit_op(OpCode::PutField);
                            self.emit_u16(str_idx);
                        } else if self.check(&Token::LParen) {
                            // Method call: obj.method(args)
                            // Use GetField2 to keep obj on stack, then CallMethod
                            self.emit_op(OpCode::GetField2);
                            self.emit_u16(str_idx);
                            // Now stack is: [obj, method]
                            self.advance(); // consume LParen
                            let arg_count = self.argument_list()?;
                            self.emit_op(OpCode::CallMethod);
                            self.emit_u16(arg_count);
                        } else {
                            // obj.prop
                            self.emit_op(OpCode::GetField);
                            self.emit_u16(str_idx);
                        }
                    } else {
                        return Err(CompileError::SyntaxError(
                            "Expected property name".into(),
                        ));
                    }
                }

                // Post-increment/decrement handled here for simple variables
                // (more complex cases would need special handling)

                _ => break,
            }
        }
        Ok(())
    }

    /// Parse the target of a new expression (identifier or member access, but not function call)
    /// This is called for `new Foo()` or `new foo.Bar()` - we want to parse the constructor
    /// expression without consuming the `(` which we'll handle separately.
    fn new_expr_target(&mut self) -> Result<(), CompileError> {
        // Parse the primary expression (identifier, grouping, etc.)
        match &self.current_token {
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();
                // Resolve as local, capture, or global
                if let Some(idx) = self.resolve_local(&name) {
                    self.emit_get_local(idx);
                } else if let Some(idx) = self.resolve_capture(&name) {
                    self.emit_get_capture(idx);
                } else {
                    // Try as a global (builtin function)
                    self.emit_get_global(&name);
                }
            }
            Token::LParen => {
                self.advance();
                self.expression()?;
                self.expect(Token::RParen)?;
            }
            _ => {
                return Err(CompileError::SyntaxError(format!(
                    "Expected constructor expression, got {:?}",
                    self.current_token
                )));
            }
        }

        // Handle member access chain (but NOT function calls)
        loop {
            match &self.current_token {
                Token::Dot => {
                    self.advance();
                    if let Token::Ident(name) = &self.current_token {
                        let name = name.clone();
                        self.advance();
                        let str_idx = self.string_constants.len() as u16;
                        self.string_constants.push(name);
                        self.emit_op(OpCode::GetField);
                        self.emit_u16(str_idx);
                    } else {
                        return Err(CompileError::SyntaxError(
                            "Expected property name".into(),
                        ));
                    }
                }
                Token::LBracket => {
                    // Array access: foo[expr]
                    self.advance();
                    self.expression()?;
                    self.expect(Token::RBracket)?;
                    self.emit_op(OpCode::GetArrayEl);
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Parse the operand of a delete expression
    /// Handles: delete obj.prop, delete arr[idx], delete variable
    fn delete_expr(&mut self) -> Result<(), CompileError> {
        // Parse the base expression (identifier or grouped expression)
        match &self.current_token {
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();

                // Resolve the variable as local, capture, or global
                if let Some(idx) = self.resolve_local(&name) {
                    self.emit_get_local(idx);
                } else if let Some(idx) = self.resolve_capture(&name) {
                    self.emit_get_capture(idx);
                } else {
                    // Try as a global (builtin function)
                    self.emit_get_global(&name);
                }
            }
            Token::LParen => {
                self.advance();
                self.expression()?;
                self.expect(Token::RParen)?;
            }
            _ => {
                return Err(CompileError::SyntaxError(format!(
                    "Expected expression after delete, got {:?}",
                    self.current_token
                )));
            }
        }

        // Now handle the property access (must have . or [])
        match &self.current_token {
            Token::Dot => {
                self.advance();
                if let Token::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.advance();
                    // Push property name as string constant
                    let str_idx = self.string_constants.len() as u16;
                    self.string_constants.push(name);
                    self.emit_op(OpCode::PushConst);
                    let const_idx = self.add_constant(Value::string(str_idx));
                    self.emit_u16(const_idx);
                    self.emit_op(OpCode::Delete);
                } else {
                    return Err(CompileError::SyntaxError(
                        "Expected property name after .".into(),
                    ));
                }
            }
            Token::LBracket => {
                self.advance();
                self.expression()?;
                self.expect(Token::RBracket)?;
                self.emit_op(OpCode::Delete);
            }
            _ => {
                // delete on a simple variable - push undefined as key
                // This is non-standard but we'll return true
                self.emit_op(OpCode::Undefined);
                self.emit_op(OpCode::Delete);
            }
        }
        Ok(())
    }

    /// Parse function call arguments
    fn argument_list(&mut self) -> Result<u16, CompileError> {
        let mut count = 0;

        if !self.check(&Token::RParen) {
            loop {
                self.expression()?;
                count += 1;

                if count > 255 {
                    return Err(CompileError::SyntaxError("Too many arguments".into()));
                }

                if !self.match_token(&Token::Comma) {
                    break;
                }
            }
        }

        self.expect(Token::RParen)?;
        Ok(count)
    }

    /// Parse array literal elements: expr, expr, ... ]
    /// Called after the opening '[' has been consumed
    fn array_literal(&mut self) -> Result<u16, CompileError> {
        let mut count = 0;

        if !self.check(&Token::RBracket) {
            loop {
                self.expression()?;
                count += 1;

                if count > 65535 {
                    return Err(CompileError::SyntaxError("Too many array elements".into()));
                }

                if !self.match_token(&Token::Comma) {
                    break;
                }

                // Handle trailing comma: [1, 2, ]
                if self.check(&Token::RBracket) {
                    break;
                }
            }
        }

        self.expect(Token::RBracket)?;
        Ok(count)
    }

    /// Get precedence and associativity of current infix operator
    fn infix_precedence(&self) -> Option<(Precedence, Associativity)> {
        use Associativity::*;
        use Precedence::*;

        match &self.current_token {
            // Assignment
            Token::Eq
            | Token::PlusEq
            | Token::MinusEq
            | Token::StarEq
            | Token::SlashEq
            | Token::PercentEq
            | Token::AmpEq
            | Token::PipeEq
            | Token::CaretEq
            | Token::LtLtEq
            | Token::GtGtEq
            | Token::GtGtGtEq
            | Token::StarStarEq => Some((Assignment, Right)),

            // Ternary
            Token::Question => Some((Ternary, Right)),

            // Logical OR
            Token::PipePipe => Some((LogicalOr, Left)),

            // Logical AND
            Token::AmpAmp => Some((LogicalAnd, Left)),

            // Bitwise OR
            Token::Pipe => Some((BitwiseOr, Left)),

            // Bitwise XOR
            Token::Caret => Some((BitwiseXor, Left)),

            // Bitwise AND
            Token::Amp => Some((BitwiseAnd, Left)),

            // Equality
            Token::EqEq | Token::BangEq | Token::EqEqEq | Token::BangEqEq => {
                Some((Equality, Left))
            }

            // Relational
            Token::Lt | Token::LtEq | Token::Gt | Token::GtEq | Token::InstanceOf | Token::In => {
                Some((Relational, Left))
            }

            // Shift
            Token::LtLt | Token::GtGt | Token::GtGtGt => Some((Shift, Left)),

            // Additive
            Token::Plus | Token::Minus => Some((Additive, Left)),

            // Multiplicative
            Token::Star | Token::Slash | Token::Percent => Some((Multiplicative, Left)),

            // Exponentiation (right-associative)
            Token::StarStar => Some((Exponentiation, Right)),

            _ => None,
        }
    }

    /// Handle assignment expression
    fn assignment_expr(&mut self, _op: &Token) -> Result<(), CompileError> {
        // For now, only handle simple variable assignment
        // The left-hand side was already compiled; we need to undo that
        // This is a simplified implementation - a proper one would track lvalues
        Err(CompileError::SyntaxError(
            "Assignment expressions not yet fully implemented".into(),
        ))
    }

    /// Handle ternary conditional: a ? b : c
    fn ternary_expr(&mut self) -> Result<(), CompileError> {
        // Condition already on stack
        let else_jump = self.emit_jump(OpCode::IfFalse);

        // Parse 'then' expression
        self.expression()?;
        let end_jump = self.emit_jump(OpCode::Goto);

        self.expect(Token::Colon)?;
        self.patch_jump(else_jump);

        // Parse 'else' expression
        self.parse_precedence(Precedence::Ternary)?;
        self.patch_jump(end_jump);

        Ok(())
    }

    /// Handle short-circuit logical operators
    fn short_circuit_expr(&mut self, op: &Token) -> Result<(), CompileError> {
        match op {
            Token::AmpAmp => {
                // Left is on stack; if false, skip right
                let end_jump = self.emit_jump(OpCode::IfFalse);
                self.emit_op(OpCode::Drop); // Drop the true value
                self.parse_precedence(Precedence::LogicalAnd.next())?;
                self.patch_jump(end_jump);
            }
            Token::PipePipe => {
                // Left is on stack; if true, skip right
                let end_jump = self.emit_jump(OpCode::IfTrue);
                self.emit_op(OpCode::Drop); // Drop the false value
                self.parse_precedence(Precedence::LogicalOr.next())?;
                self.patch_jump(end_jump);
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    /// Emit binary operator
    fn emit_binary_op(&mut self, op: &Token) -> Result<(), CompileError> {
        match op {
            Token::Plus => self.emit_op(OpCode::Add),
            Token::Minus => self.emit_op(OpCode::Sub),
            Token::Star => self.emit_op(OpCode::Mul),
            Token::Slash => self.emit_op(OpCode::Div),
            Token::Percent => self.emit_op(OpCode::Mod),
            Token::StarStar => self.emit_op(OpCode::Pow),
            Token::Amp => self.emit_op(OpCode::And),
            Token::Pipe => self.emit_op(OpCode::Or),
            Token::Caret => self.emit_op(OpCode::Xor),
            Token::LtLt => self.emit_op(OpCode::Shl),
            Token::GtGt => self.emit_op(OpCode::Sar),
            Token::GtGtGt => self.emit_op(OpCode::Shr),
            Token::Lt => self.emit_op(OpCode::Lt),
            Token::LtEq => self.emit_op(OpCode::Lte),
            Token::Gt => self.emit_op(OpCode::Gt),
            Token::GtEq => self.emit_op(OpCode::Gte),
            Token::EqEq => self.emit_op(OpCode::Eq),
            Token::BangEq => self.emit_op(OpCode::Neq),
            Token::EqEqEq => self.emit_op(OpCode::StrictEq),
            Token::BangEqEq => self.emit_op(OpCode::StrictNeq),
            Token::InstanceOf => self.emit_op(OpCode::InstanceOf),
            Token::In => self.emit_op(OpCode::In),
            _ => {
                return Err(CompileError::SyntaxError(format!(
                    "Unknown binary operator: {:?}",
                    op
                )))
            }
        }
        Ok(())
    }
}

/// Operator precedence levels (lowest to highest)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Lowest,
    Assignment,     // = += -= etc.
    Ternary,        // ?:
    LogicalOr,      // ||
    LogicalAnd,     // &&
    BitwiseOr,      // |
    BitwiseXor,     // ^
    BitwiseAnd,     // &
    Equality,       // == != === !==
    Relational,     // < <= > >= instanceof in
    Shift,          // << >> >>>
    Additive,       // + -
    Multiplicative, // * / %
    Exponentiation, // **
    Unary,          // ! ~ - + typeof void delete
    Postfix,        // ++ -- (postfix)
    Call,           // () [] .
    Primary,        // literals, identifiers
}

impl Precedence {
    /// Get the next higher precedence
    fn next(self) -> Self {
        match self {
            Precedence::Lowest => Precedence::Assignment,
            Precedence::Assignment => Precedence::Ternary,
            Precedence::Ternary => Precedence::LogicalOr,
            Precedence::LogicalOr => Precedence::LogicalAnd,
            Precedence::LogicalAnd => Precedence::BitwiseOr,
            Precedence::BitwiseOr => Precedence::BitwiseXor,
            Precedence::BitwiseXor => Precedence::BitwiseAnd,
            Precedence::BitwiseAnd => Precedence::Equality,
            Precedence::Equality => Precedence::Relational,
            Precedence::Relational => Precedence::Shift,
            Precedence::Shift => Precedence::Additive,
            Precedence::Additive => Precedence::Multiplicative,
            Precedence::Multiplicative => Precedence::Exponentiation,
            Precedence::Exponentiation => Precedence::Unary,
            Precedence::Unary => Precedence::Postfix,
            Precedence::Postfix => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::Primary,
        }
    }
}

/// Operator associativity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Associativity {
    Left,
    Right,
}

/// Compiled function
/// Capture info for closures (public for interpreter use)
#[derive(Debug, Clone)]
pub struct CaptureInfo {
    /// Index in the outer function's locals (or captures)
    pub outer_index: usize,
    /// Whether this captures from outer's locals (true) or outer's captures (false)
    pub is_local: bool,
}

pub struct CompiledFunction {
    /// Bytecode bytes
    pub bytecode: Vec<u8>,
    /// Constant pool
    pub constants: Vec<Value>,
    /// String constant pool
    pub string_constants: Vec<String>,
    /// Number of local variables
    pub local_count: usize,
    /// Number of arguments
    pub arg_count: usize,
    /// Inner functions defined within this function
    pub functions: Vec<CompiledFunction>,
    /// Capture information for closures
    pub captures: Vec<CaptureInfo>,
}

/// Compilation error
#[derive(Debug)]
pub enum CompileError {
    UnexpectedToken { expected: String, found: String },
    SyntaxError(String),
    TooManyConstants,
    TooManyLocals,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::UnexpectedToken { expected, found } => {
                write!(f, "Expected {}, found {}", expected, found)
            }
            CompileError::SyntaxError(msg) => write!(f, "Syntax error: {}", msg),
            CompileError::TooManyConstants => write!(f, "Too many constants"),
            CompileError::TooManyLocals => write!(f, "Too many local variables"),
        }
    }
}

impl std::error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_expr(source: &str) -> Result<CompiledFunction, CompileError> {
        // Wrap expression in a statement
        let full_source = format!("{};", source);
        Compiler::new(&full_source).compile()
    }

    #[test]
    fn test_compile_integers() {
        let func = compile_expr("42").unwrap();
        // Should emit: PushI8 42, Drop, ReturnUndef
        assert!(!func.bytecode.is_empty());
    }

    #[test]
    fn test_compile_small_integers() {
        // Test optimized integer opcodes (0-7)
        // Note: -1 is parsed as unary minus + 1, so it produces Push1, Neg
        for i in 0..=7 {
            let func = compile_expr(&i.to_string()).unwrap();
            // First byte should be one of the optimized push opcodes
            let expected = match i {
                0 => OpCode::Push0 as u8,
                1 => OpCode::Push1 as u8,
                2 => OpCode::Push2 as u8,
                3 => OpCode::Push3 as u8,
                4 => OpCode::Push4 as u8,
                5 => OpCode::Push5 as u8,
                6 => OpCode::Push6 as u8,
                7 => OpCode::Push7 as u8,
                _ => unreachable!(),
            };
            assert_eq!(func.bytecode[0], expected);
        }
    }

    #[test]
    fn test_compile_negative_one() {
        // -1 is parsed as unary minus + 1
        let func = compile_expr("-1").unwrap();
        // Should emit: Push1, Neg, Drop, ReturnUndef
        assert_eq!(func.bytecode[0], OpCode::Push1 as u8);
        assert_eq!(func.bytecode[1], OpCode::Neg as u8);
    }

    #[test]
    fn test_compile_boolean() {
        let func = compile_expr("true").unwrap();
        assert_eq!(func.bytecode[0], OpCode::PushTrue as u8);

        let func = compile_expr("false").unwrap();
        assert_eq!(func.bytecode[0], OpCode::PushFalse as u8);
    }

    #[test]
    fn test_compile_null() {
        let func = compile_expr("null").unwrap();
        assert_eq!(func.bytecode[0], OpCode::Null as u8);
    }

    #[test]
    fn test_compile_addition() {
        let func = compile_expr("1 + 2").unwrap();
        // Should emit: Push1, Push2, Add, Drop, ReturnUndef
        assert_eq!(func.bytecode[0], OpCode::Push1 as u8);
        assert_eq!(func.bytecode[1], OpCode::Push2 as u8);
        assert_eq!(func.bytecode[2], OpCode::Add as u8);
    }

    #[test]
    fn test_compile_precedence() {
        // 1 + 2 * 3 should be 1 + (2 * 3)
        let func = compile_expr("1 + 2 * 3").unwrap();
        // Should emit: Push1, Push2, Push3, Mul, Add
        assert_eq!(func.bytecode[0], OpCode::Push1 as u8);
        assert_eq!(func.bytecode[1], OpCode::Push2 as u8);
        assert_eq!(func.bytecode[2], OpCode::Push3 as u8);
        assert_eq!(func.bytecode[3], OpCode::Mul as u8);
        assert_eq!(func.bytecode[4], OpCode::Add as u8);
    }

    #[test]
    fn test_compile_parentheses() {
        // (1 + 2) * 3
        let func = compile_expr("(1 + 2) * 3").unwrap();
        // Should emit: Push1, Push2, Add, Push3, Mul
        assert_eq!(func.bytecode[0], OpCode::Push1 as u8);
        assert_eq!(func.bytecode[1], OpCode::Push2 as u8);
        assert_eq!(func.bytecode[2], OpCode::Add as u8);
        assert_eq!(func.bytecode[3], OpCode::Push3 as u8);
        assert_eq!(func.bytecode[4], OpCode::Mul as u8);
    }

    #[test]
    fn test_compile_unary_minus() {
        let func = compile_expr("-5").unwrap();
        // Should emit: Push5, Neg
        assert_eq!(func.bytecode[0], OpCode::Push5 as u8);
        assert_eq!(func.bytecode[1], OpCode::Neg as u8);
    }

    #[test]
    fn test_compile_comparison() {
        let func = compile_expr("1 < 2").unwrap();
        assert_eq!(func.bytecode[0], OpCode::Push1 as u8);
        assert_eq!(func.bytecode[1], OpCode::Push2 as u8);
        assert_eq!(func.bytecode[2], OpCode::Lt as u8);
    }

    #[test]
    fn test_compile_var_declaration() {
        let source = "var x = 10;";
        let func = Compiler::new(source).compile().unwrap();
        // Should declare local and initialize it
        assert_eq!(func.local_count, 1);
    }

    #[test]
    fn test_compile_var_usage() {
        let source = "var x = 10; x;";
        let func = Compiler::new(source).compile().unwrap();
        // Check that GetLoc0 is emitted for x
        assert!(func
            .bytecode
            .contains(&(OpCode::GetLoc0 as u8)));
    }

    #[test]
    fn test_compile_if_statement() {
        let source = "var x = 1; if (x) { x; }";
        let func = Compiler::new(source).compile().unwrap();
        // Should contain IfFalse jump
        assert!(func
            .bytecode
            .contains(&(OpCode::IfFalse as u8)));
    }

    #[test]
    fn test_compile_while_loop() {
        let source = "var i = 0; while (i < 5) { i; }";
        let func = Compiler::new(source).compile().unwrap();
        // Should contain IfFalse and Goto
        assert!(func
            .bytecode
            .contains(&(OpCode::IfFalse as u8)));
        assert!(func.bytecode.contains(&(OpCode::Goto as u8)));
    }

    #[test]
    fn test_compile_ternary() {
        let func = compile_expr("1 ? 2 : 3").unwrap();
        // Should contain IfFalse and Goto for branches
        assert!(func
            .bytecode
            .contains(&(OpCode::IfFalse as u8)));
        assert!(func.bytecode.contains(&(OpCode::Goto as u8)));
    }
}
