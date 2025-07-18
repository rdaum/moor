# Lambda Functions Proposal for mooR

## Introduction

LambdaMOO did not have lambdas. As part of our mission to drag the future into the past, we will fix
this glitch.

This document proposes adding anonymous function (lambda) support to the MOO programming language as
implemented in mooR. The syntax leverages MOO's existing scatter assignment patterns to create a
uniquely MOO approach to lambda expressions.

## Syntax

Lambda functions use scatter-style parameter declarations with arrow syntax:

```moo
{parameters} => expression
{parameters} => begin ... end
```

### Basic Examples

```moo
// Simple expression lambdas
{x, y} => x + y;                    // Two required parameters
{x} => x * 2;                       // Single parameter
{} => "hello world";                // No parameters

// Statement lambdas
{x, y} => begin
    let sum = x + y;
    let product = x * y;
    return sum * product;
end
```

### Advanced Parameter Features

MOO's scatter assignment syntax naturally extends to lambda parameters:

```moo
// Optional parameters (using ?var syntax)
{x, ?y} => x + (y || 0);            // y defaults to 0 if not provided
{?name} => name || "Anonymous";     // Single optional parameter

// Rest parameters (using @var syntax)
{x, @rest} => x + length(rest);     // x gets first arg, rest gets remainder
{@all} => length(all);              // All args collected into list

// Mixed parameter types
{required, ?optional, @rest} => begin
    return [required -> required, optional -> optional, rest -> rest];
end
```

### Usage Examples

```moo
// Function assignment and calls
let add = {x, y} => x + y;
let result = add(5, 3);  // returns 8

// Higher-order functions (with future builtins)
let numbers = {1, 2, 3, 4, 5};
let doubled = map({x} => x * 2, numbers);
let evens = filter({x} => x % 2 == 0, numbers);

// List comprehensions with lambdas
let squares = {({x} => x * x)(n) for n in numbers};

// Flexible argument handling
let sum = {@nums} => begin
    let total = 0;
    for n in nums
        total = total + n;
    endfor
    return total;
end

sum(1, 2, 3, 4);  // returns 10
sum();            // returns 0
```

## AST and Opcode Changes

### AST Nodes (crates/compiler/src/ast.rs)

Add new `Expr` variant for lambda expressions:

```rust
Lambda {
    params: Vec<ScatterItem>,  // Reuse existing scatter parameter structure
    body: Box<Expr>,           // Lambda body (expression or statement block)
}
```

Modify existing `Call` expression to support both builtin and lambda calls:

```rust
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum CallTarget {
    Builtin(Symbol),        // Compile-time known builtin function
    Expr(Box<Expr>),        // Runtime expression that evaluates to callable
}

// Modified Call expression
Call {
    function: CallTarget,   // Changed from Symbol to CallTarget
    args: Vec<Arg>
}
```

The `ScatterItem` structure already supports:

- Required parameters (`ScatterKind::Required`)
- Optional parameters with defaults (`ScatterKind::Optional`)
- Rest parameters (`ScatterKind::Rest`)

**Call target resolution enables rich lambda calls:**

```moo
my_lambda(x, y)                    // Direct variable reference
obj.method_lambda(x, y)            // Property access
lambdas[index](x, y)               // Array/map indexing
get_transformer()(x, y)            // Function returning lambda
(use_fast ? fast_fn | slow_fn)(x)  // Conditional lambda selection
```

### New Opcodes (crates/common/src/program/opcode.rs)

Add two new opcodes:

```rust
/// Create lambda value from pre-compiled Program and parameter specification
/// The lambda Program is compiled at compile-time and embedded as a literal
MakeLambda {
    scatter_offset: Offset,    // Reference to parameter spec in scatter_tables
    program_literal: Offset,   // Reference to pre-compiled Program in literals table
},

/// Call a lambda value with arguments from stack
/// Expects stack: [lambda_value, args_list]
/// Uses existing scatter assignment for parameter binding
CallLambda,
```

### Program Structure Changes (crates/common/src/program/program.rs)

No changes to `PrgInner` structure required! Lambda Programs are stored as literals in the existing
`literals: Vec<Var>` field:

```rust
pub struct PrgInner {
    /// All the literals referenced in this program.
    pub literals: Vec<Var>,  // Lambda Programs stored here as Var::mk_program()
    // ... existing fields unchanged ...
}
```

Lambda bodies are compiled into complete standalone Programs at compile-time and stored as literal
values, reusing existing infrastructure. This approach:

- **Avoids "Program during Program evaluation"** - all compilation happens at compile-time
- **Enables portability** - lambdas carry complete execution context
- **Reuses existing infrastructure** - no new storage mechanisms needed
- **Supports full language** - lambda Programs can contain forks, errors, comprehensions, etc.

### Codegen Changes (crates/compiler/src/codegen.rs)

The `generate_expr` function needs to handle both call targets and lambda creation:

```rust
Expr::Call { function, args } => {
    match function {
        CallTarget::Builtin(sym) => {
            // Existing builtin lookup logic (lines 500-525)
            match BUILTINS.find_builtin(*sym) {
                Some(id) => {
                    self.generate_arg_list(args)?;
                    self.emit(Op::FuncCall { id });
                }
                None => // Handle unknown builtin...
            }
        }
        CallTarget::Expr(expr) => {
            // New lambda call logic
            self.generate_expr(expr.as_ref())?;       // Evaluate callable expression
            self.generate_arg_list(args)?;            // Push args list
            self.emit(Op::CallLambda);                // Runtime dispatch
            self.pop_stack(1);                        // Pop callable, leave result
        }
    }
}

// New lambda creation - compiles lambda body into standalone Program
Expr::Lambda { params, body } => {
    // Compile lambda body into standalone Program (similar to fork vector pattern)
    let lambda_program = self.compile_lambda_body(params, body)?;

    // Store compiled Program as literal
    let program_literal = self.add_literal(Var::mk_program(lambda_program))?;
    let scatter_offset = self.compile_scatter_params(params)?;

    self.emit(Op::MakeLambda {
        scatter_offset,
        program_literal
    });
    self.push_stack(1);
}
```

**Lambda Body Compilation** (following fork vector pattern):

```rust
fn compile_lambda_body(&mut self, params: &[ScatterItem], body: &Expr) -> Result<Program> {
    // Stash current compilation state (like fork vectors)
    let stashed_ops = std::mem::take(&mut self.ops);
    let stashed_literals = std::mem::take(&mut self.literals);
    let stashed_var_names = self.var_names.clone();
    // ... stash other state ...

    // Create new compilation context for lambda
    self.reset_for_lambda_compilation();
    self.setup_lambda_parameters(params)?;

    // Compile lambda body in isolation
    self.generate_expr(body)?;
    self.emit(Op::Return);  // Implicit return of expression result

    // Build standalone Program from compiled state
    let lambda_program = Program::new(
        std::mem::take(&mut self.ops),
        std::mem::take(&mut self.literals),
        self.var_names.clone(),
        // ... all other compilation state ...
    );

    // Restore main compilation context
    self.ops = stashed_ops;
    self.literals = stashed_literals;
    self.var_names = stashed_var_names;
    // ... restore other state ...

    Ok(lambda_program)
}
```

### Parser Changes (crates/compiler/src/parse.rs)

Function call parsing determines the appropriate `CallTarget`:

```rust
fn parse_call(&mut self, name: Symbol, args: Vec<Arg>) -> Expr {
    if BUILTINS.find_builtin(name).is_some() {
        // Known builtin at compile time
        Expr::Call {
            function: CallTarget::Builtin(name),
            args
        }
    } else {
        // Unknown function - could be lambda variable
        let var_expr = self.create_variable_expr(name);
        Expr::Call {
            function: CallTarget::Expr(Box::new(var_expr)),
            args
        }
    }
}
```

This approach keeps builtin calls optimized at compile-time while enabling dynamic lambda dispatch.

### Decompile Support (crates/compiler/src/decompile.rs)

Add decompilation support for lambda opcodes:

```rust
// In the decompile() match statement:
Op::MakeLambda { scatter_offset, program_literal } => {
    // Retrieve pre-compiled Program from literals
    let lambda_program = self.find_literal(&program_literal)?;
    let Variant::Program(program) = lambda_program.variant() else {
        return Err(MalformedProgram("expected Program literal for lambda".to_string()));
    };

    // Retrieve scatter specification
    let scatter_spec = self.program.scatter_table(scatter_offset).clone();

    // Decompile lambda body from standalone Program
    let lambda_body = decompile_lambda_program(program)?;

    // Convert scatter spec to parameter list
    let params = self.decompile_scatter_params(&scatter_spec)?;

    self.push_expr(Expr::Lambda {
        params,
        body: Box::new(lambda_body),
    });
}

Op::CallLambda => {
    let args = self.pop_expr()?;
    let lambda_expr = self.pop_expr()?;
    let Expr::List(args) = args else {
        return Err(MalformedProgram("expected list of args for lambda call".to_string()));
    };

    self.push_expr(Expr::Call {
        function: CallTarget::Expr(Box::new(lambda_expr)),
        args,
    });
}
```

**Lambda Program Decompilation:**

```rust
fn decompile_lambda_program(program: &Program) -> Result<Expr, DecompileError> {
    // Create separate decompiler for lambda's standalone Program
    let mut lambda_decompile = Decompile {
        program: program.clone(),
        fork_vector: None,
        position: 0,
        expr_stack: VecDeque::new(),
        statements: vec![],
        assigned_vars: HashSet::new(),
    };

    // Decompile lambda body
    let opcode_vector_len = lambda_decompile.opcode_vector().len();
    while lambda_decompile.position < opcode_vector_len {
        lambda_decompile.decompile()?;
    }

    // Lambda body should result in single expression or statement block
    match lambda_decompile.statements.len() {
        0 => {
            // Expression lambda - result should be on expression stack
            lambda_decompile.pop_expr()
        }
        _ => {
            // Statement lambda - wrap statements in begin/end block
            Ok(Expr::Block(lambda_decompile.statements))
        }
    }
}
```

### Unparse Support (crates/compiler/src/unparse.rs)

Add unparsing support for lambda expressions:

```rust
// In unparse_expr() match statement:
Expr::Lambda { params, body } => {
    let mut buffer = String::new();
    buffer.push('{');

    // Unparse parameter list using scatter syntax
    let len = params.len();
    for (i, param) in params.iter().enumerate() {
        match param.kind {
            ScatterKind::Required => {},
            ScatterKind::Optional => buffer.push('?'),
            ScatterKind::Rest => buffer.push('@'),
        }

        let name = self.unparse_variable(&param.id);
        buffer.push_str(&name.as_arc_string());

        if let Some(expr) = &param.expr {
            buffer.push_str(" = ");
            buffer.push_str(self.unparse_expr(expr)?.as_str());
        }

        if i + 1 < len {
            buffer.push_str(", ");
        }
    }

    buffer.push_str("} => ");

    // Unparse lambda body
    match body.as_ref() {
        Expr::Block(statements) => {
            // Multi-statement lambda
            buffer.push_str("begin\n");
            let stmt_lines = self.unparse_stmts(statements, INDENT_LEVEL)?;
            for line in stmt_lines {
                buffer.push_str("  ");
                buffer.push_str(&line);
                buffer.push('\n');
            }
            buffer.push_str("end");
        }
        _ => {
            // Expression lambda
            buffer.push_str(self.unparse_expr(body)?.as_str());
        }
    }

    Ok(buffer)
}

// Update Call expression unparsing to handle CallTarget
Expr::Call { function, args } => {
    let mut buffer = String::new();

    match function {
        CallTarget::Builtin(sym) => {
            buffer.push_str(&sym.as_arc_string());
        }
        CallTarget::Expr(expr) => {
            buffer.push_str(self.unparse_expr(expr)?.as_str());
        }
    }

    buffer.push('(');
    buffer.push_str(self.unparse_args(args)?.as_str());
    buffer.push(')');
    Ok(buffer)
}
```

**Lambda Expression Precedence:**

```rust
// In Expr::precedence() method:
Expr::Lambda { .. } => 1,  // Highest precedence like other primary expressions
```

This ensures lambda expressions can be properly decompiled back to source code and unparsed with
correct syntax, completing the round-trip compilation cycle.

## Changes in var/ and common/

### New Variant Type (crates/var/src/variant.rs)

Add lambda variant to the `Variant` enum:

```rust
#[derive(Clone, Encode, Decode)]
pub enum Variant {
    // ... existing variants ...
    Lambda(Box<Lambda>),
}

#[derive(Clone, Encode, Decode)]
pub struct Lambda {
    /// Parameter specification (reuses scatter assignment structure)
    pub params: ScatterArgs,
    /// The lambda body as standalone executable program
    /// Compiled at compile-time into a complete, self-contained Program
    pub body: Program,
    /// Captured variable environment from lambda creation site
    pub captured_env: Vec<Vec<Var>>,
}
```

### Var Implementation (crates/var/src/var.rs)

Add lambda support methods:

```rust
impl Var {
    pub fn mk_lambda(params: ScatterArgs, body: Program, captured_env: Vec<Vec<Var>>) -> Self {
        Var(Variant::Lambda(Box::new(Lambda { params, body, captured_env })))
    }

    pub fn as_lambda(&self) -> Option<&Lambda> {
        match self.variant() {
            Variant::Lambda(l) => Some(l.as_ref()),
            _ => None,
        }
    }
}
```

Update type checking:

- Add `TYPE_LAMBDA` to `VarType` enum
- Add lambda case to `type_code()` method
- Add lambda support to `Hash`, `Ord`, `PartialEq` implementations

### JSON Serialization (crates/web-host/src/host/mod.rs)

Lambda values cannot be serialized to JSON since they contain executable code and program state. The
web-host boundary must explicitly handle this:

```rust
pub fn var_as_json(v: &Var) -> Result<serde_json::Value, JsonSerializationError> {
    match v.variant() {
        // ... existing cases ...
        Variant::Lambda(_) => {
            Err(JsonSerializationError::UnsupportedType(
                "Lambda functions cannot be serialized to JSON".to_string()
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum JsonSerializationError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),
    // ... other error cases ...
}
```

**Error Handling Strategy:**

When lambda values reach the JSON serialization boundary (e.g., in verb return values, object
properties, or narrative events), the web-host should:

1. **Detect lambda values** during serialization
2. **Return a structured error** to the web client indicating the type cannot be serialized
3. **Provide meaningful error messages** to help users understand the limitation
4. **Maintain connection stability** - don't crash the session over serialization errors

## VM Changes

### Lambda Creation

When executing `MakeLambda` opcode:

1. Retrieve scatter specification from `scatter_tables[scatter_offset]`
2. Retrieve pre-compiled Program from `literals[program_literal]`
3. Capture current variable environment (`Vec<Vec<Var>>`) from execution context
4. Create `Lambda` value containing parameters, Program, and captured environment
5. Push lambda value onto stack

### Lambda Invocation

When executing `CallLambda` opcode:

1. Pop lambda value and arguments list from stack
2. Create new execution frame for lambda's standalone Program:
   - Push new `MooStackFrame` for lambda's Program
   - Restore captured variable environment as base context
   - Set up fresh parameter binding scope on top of captured environment
3. Execute parameter binding using lambda's scatter specification:
   - Reuses existing scatter assignment logic
   - Binds arguments to lambda's parameter environment
   - Required parameters get corresponding arguments
   - Optional parameters get arguments or their defaults
   - Rest parameters collect remaining arguments into a list
   - Error if insufficient arguments for required parameters
4. Execute lambda's Program in isolated execution context:
   - Complete standalone Program execution
   - Access to captured variables, lambda parameters, and all language features
   - Supports forks, errors, comprehensions, control flow, etc.
   - VM executes lambda Program exactly like any other Program
5. Return lambda result value and clean up execution frame

### Scope Management (crates/kernel/src/vm/moo_frame.rs)

Lambda execution requires environment restoration from captured state:

```rust
pub enum ScopeType {
    // ... existing types ...
    Lambda,  // Restores captured environment with parameter overlay
}
```

Lambda scopes restore captured closure state:

- Restore captured variable environment (`Vec<Vec<Var>>`) from lambda creation site
- Create fresh parameter binding scope on top of captured environment
- Lambda parameters overlay on captured environment (can shadow captured variables)
- Access to both captured variables and lambda parameters
- Clean up automatically when lambda returns

**Environment capture at lambda creation:**

- Capture current variable environment (`Vec<Vec<Var>>`) when `MakeLambda` executes
- Store captured environment in Lambda value alongside parameter spec and body
- Captured environment includes all accessible scopes at creation time

### Error Handling

Lambda calls can produce standard MOO errors:

- `E_TYPE` if called value is not a lambda
- `E_ARGS` if insufficient arguments for required parameters
- `E_MAXREC` if lambda recursion exceeds limits
- Any errors from lambda body execution

## Implementation Notes

### Variable Capture and Closures

Lambda values capture their lexical environment at creation time, enabling true closure behavior:

- **Lexical capture** - can access variables from definition site
- **Environment restoration** - captured environment is restored during lambda execution
- **Accessible references:**
  - Lambda's own parameters
  - Captured variables from outer scopes
  - Literals embedded in the lambda program
  - Object references and properties (e.g., `#123`, `player`)
  - Builtin function calls

**How variable capture works:**

Lambda values contain a snapshot of the variable environment (`Vec<Vec<Var>>`) from their creation
site. When the lambda executes, this captured environment is restored as the base context, with
lambda parameters overlaid on top.

**Example showing closure behavior:**

```moo
let x = 10;
let make_lambda = begin
    let y = 20;
    return {z} => x + y + z;  // Captures x and y from outer scopes
end

let my_lambda = make_lambda();
let result = my_lambda(5);     // Returns 35 (10 + 20 + 5)

// Lambda can access all levels of captured scope:
let outer = 1;
let middle_fn = begin
    let middle = 2;
    return begin
        let inner = 3;
        return {param} => outer + middle + inner + param;  // Captures all three
    end
end
```

This enables powerful functional programming patterns while maintaining MOO's execution semantics.

### Compile-Time Program Creation Architecture

The key architectural insight is **avoiding "Program during Program evaluation"** by doing all
Program compilation at compile-time:

**Compile-Time:**

- Lambda bodies are compiled into complete standalone Programs
- Each lambda gets its own compilation context (following fork vector pattern)
- Compiled Programs are stored as literals in the parent Program
- All cross-references (jumps, scatters, etc.) resolved at compile-time

**Runtime:**

- `MakeLambda` extracts pre-compiled Program from literals + captures environment
- `CallLambda` executes standalone Program with restored environment
- No Program construction during execution - everything pre-compiled

**Benefits:**

- **Clean separation**: Compilation complexity stays at compile-time
- **Full language support**: Lambda Programs support all MOO features
- **Portable**: Lambdas carry complete execution context
- **Efficient**: No runtime compilation overhead
- **Reuses infrastructure**: Leverages existing Program compilation and execution

This mirrors how fork vectors work but creates complete Programs instead of just opcode vectors,
enabling lambdas to be truly first-class portable functions.

### Reusing Existing Infrastructure

The proposal leverages existing MOO infrastructure:

- `ScatterArgs` for parameter handling (already supports optional/rest parameters)
- Existing `Scatter` opcode for all parameter binding logic
- Existing program storage patterns (similar to fork vectors)
- Existing error handling patterns
- List-based argument passing (consistent with verb calls)

**New compilation requirements:**

- Lambda compilation must analyze variable references for capture requirements
- Environment capture at `MakeLambda` execution time
- Lambda body compilation with access to both parameters and captured variables
- Proper scope resolution during lambda execution

### No Currying Support

Lambda values are **not curryable** - they must be called with a complete argument list:

```moo
let add = {x, y} => x + y;
let partial = add(5);     // ERROR: E_ARGS - insufficient arguments
let result = add(5, 3);   // OK: returns 8
```

**Why we rejected automatic partial application:**

Automatic currying creates fundamental ambiguity with MOO's scatter assignment parameters:

```moo
// Case 1: When should execution happen vs. continued currying?
let func = {x, ?y = 10} => x + y;
let result = func(5);  // Execute now (5 + 10) or curry waiting for y?

// Case 2: Rest parameters make the decision impossible
let func = {x, @rest} => x + length(rest);
func(1)        // Complete? (x=1, rest=[])
func(1, 2)     // Complete? (x=1, rest=[2])
func(1, 2, 3)  // Complete? (x=1, rest=[2,3])
// Every call could be either "done" or "partial" - no way to decide!

// Case 3: Mixed parameters compound the problem
let func = {a, ?b = 5, @rest} => a + b + length(rest);
// ANY number of arguments could be "complete"
```

The ambiguity is **inherent and unsolvable**:

- Can't reliably determine user intent from argument count alone
- Different behavior for different parameter patterns creates confusion
- Error messages become cryptic ("why can't I curry this lambda?")
- Debugging becomes difficult (value vs. partial function?)

**Design principle**: MOO emphasizes **predictable, explicit semantics**. Automatic currying
introduces magic that conflicts with this philosophy.

**Alternative**: Users can easily implement explicit currying when needed:

```moo
// Manual currying helpers
let curry2 = {f} => {x} => {y} => f(x, y);
let curry3 = {f} => {x} => {y} => {z} => f(x, y, z);

// Use when desired
let add = {x, y} => x + y;
let curried_add = curry2(add);
let add_five = curried_add(5);
let result = add_five(3);  // Returns 8

// Or build currying directly into lambda design
let multiply_by = {n} => {x} => x * n;
let double = multiply_by(2);
let result = double(5);    // Returns 10
```

This approach:

- **Eliminates ambiguity**: `lambda(args)` always executes
- **Maintains predictability**: Clear distinction between values and functions
- **Preserves flexibility**: Manual currying is more powerful than automatic
- **Aligns with MOO**: Explicit rather than magical behavior

### Future Extensions

This foundation could support future enhancements:

- Destructuring parameters: `{{x, y}, z} => x + y + z`
- Type annotations: `{x: INT, y: FLOAT} => x + y`

## Architectural Refactoring: Moving Program to var/

To support lambda values containing `Program` objects, we should move the program-related code from
`crates/common/src/program/` into `crates/var/`. This eliminates circular dependencies and creates a
cleaner architecture.

### Files to Move

Move these files from `crates/common/src/program/` to `crates/var/src/program/`:

- `builtins.rs` - Builtin function definitions
- `labels.rs` - Jump labels and offsets
- `names.rs` - Variable name management
- `opcode.rs` - VM opcodes
- `program.rs` - Program structure and serialization

### Benefits of This Move

1. **Enables Lambda Values**: `Variant::Lambda` can directly contain `Program` without circular
   dependencies
2. **Cleaner Architecture**: Program types logically belong with other value types
3. **Simplified Dependencies**: Reduces cross-crate complexity
4. **Future Flexibility**: Easier to add other code-containing value types

### Migration Strategy

1. **Phase 1**: Move program files to var/, update var/ to compile
2. **Phase 2**: Add re-exports in common/ for backward compatibility
3. **Phase 3**: Update all imports in other crates incrementally
4. **Phase 4**: Remove re-exports once all imports are updated
5. **Phase 5**: Implement lambda support using integrated program types

This refactoring is a prerequisite for lambda implementation but provides architectural benefits
beyond just lambda support.

## Migration Path

Lambda support can be added incrementally:

1. **Phase 0**: Move `Program` from common/ to var/ (architectural prerequisite)
2. **Phase 1**: Basic expression lambdas with required parameters
3. **Phase 2**: Statement lambdas with `begin`/`end` blocks
4. **Phase 3**: Optional and rest parameters
5. **Phase 4**: Integration with higher-order builtins (`map`, `filter`, etc.)

This provides a smooth adoption path while maintaining backward compatibility.
