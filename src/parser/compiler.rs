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
}

/// Jump patch location
#[derive(Debug, Clone, Copy)]
struct JumpPatch {
    /// Offset in bytecode where the jump target needs to be patched
    offset: usize,
}

/// Compiler state
pub struct Compiler<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    previous_token: Token,
    bytecode: Vec<u8>,
    constants: Vec<Value>,
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
            locals: Vec::new(),
            max_locals: 0,
            scope_depth: 0,
            current_pos: SourcePos::default(),
            had_error: false,
            panic_mode: false,
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
            Ok(CompiledFunction {
                bytecode: self.bytecode,
                constants: self.constants,
                local_count: self.max_locals,
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
            Token::If => self.if_statement(),
            Token::While => self.while_statement(),
            Token::For => self.for_statement(),
            Token::Return => self.return_statement(),
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

        self.expect(Token::LParen)?;
        self.expression()?;
        self.expect(Token::RParen)?;

        let exit_jump = self.emit_jump(OpCode::IfFalse);

        self.statement()?;

        self.emit_loop(loop_start);
        self.patch_jump(exit_jump);

        Ok(())
    }

    /// Parse for statement
    fn for_statement(&mut self) -> Result<(), CompileError> {
        self.advance(); // consume 'for'
        self.expect(Token::LParen)?;

        self.begin_scope();

        // Initializer
        if self.match_token(&Token::Semicolon) {
            // No initializer
        } else if self.check(&Token::Var) {
            self.var_declaration()?;
        } else if self.check(&Token::Let) {
            self.let_declaration()?;
        } else {
            self.expression_statement()?;
        }

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

        self.end_scope();

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
                    // For now, store string as a constant (we'll need string interning later)
                    let idx = self.add_constant(Value::undefined()); // Placeholder - needs string support
                    self.emit_const(idx);
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

                    let idx = self.resolve_local(&name).ok_or_else(|| {
                        CompileError::SyntaxError(format!("Undefined variable '{}'", name))
                    })?;

                    // For compound assignment (+=, -=, etc.), get the current value first
                    if !matches!(op, Token::Eq) {
                        self.emit_get_local(idx);
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
                    self.emit_set_local(idx);
                } else if let Some(idx) = self.resolve_local(&name) {
                    self.emit_get_local(idx);
                } else {
                    // Global variable - emit as property access on global object
                    // For now, emit error for undefined variable
                    return Err(CompileError::SyntaxError(format!(
                        "Undefined variable '{}'",
                        name
                    )));
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

                // Array access: a[b]
                Token::LBracket => {
                    self.advance();
                    self.expression()?;
                    self.expect(Token::RBracket)?;
                    self.emit_op(OpCode::GetArrayEl);
                }

                // Member access: a.b
                Token::Dot => {
                    self.advance();
                    if let Token::Ident(name) = &self.current_token {
                        let name = name.clone();
                        self.advance();
                        // Emit GetField with property name as constant
                        let idx = self.add_constant(Value::undefined()); // Placeholder for string
                        self.emit_op(OpCode::GetField);
                        self.emit_u16(idx);
                        let _ = name; // TODO: use name for property lookup
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
pub struct CompiledFunction {
    /// Bytecode bytes
    pub bytecode: Vec<u8>,
    /// Constant pool
    pub constants: Vec<Value>,
    /// Number of local variables
    pub local_count: usize,
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
