# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**sqlite-serve** is a dynamic NGINX module written in Rust that integrates SQLite databases with Handlebars templating. It serves data-driven web applications directly from NGINX without requiring a separate application server.

## Build & Test Commands

### Build Environment Setup

**CRITICAL**: Always use `direnv exec "$PWD"` prefix when running cargo commands:

```bash
# Build
direnv exec "$PWD" cargo build
direnv exec "$PWD" cargo build --release

# Test
direnv exec "$PWD" cargo test
direnv exec "$PWD" cargo test test_name
direnv exec "$PWD" cargo test -- --nocapture

# Check
direnv exec "$PWD" cargo check
```

### Running the Module

```bash
# Start example server
./start.sh

# Manual NGINX control
./ngx_src/nginx-1.28.0/objs/nginx -c conf/sqlite_serve.conf -p .
./ngx_src/nginx-1.28.0/objs/nginx -s stop -c conf/sqlite_serve.conf -p .
```

### Debugging

Enable debug logging in nginx configuration:
```nginx
error_log logs/error.log debug;
```

Then check logs:
```bash
tail -f logs/error.log | grep sqlite
```

## Architecture Overview

This codebase follows **functional programming principles** with a strict layered architecture:

### Layered Design (Functional Core, Imperative Shell)

```
┌────────────────────────────────────────────┐
│  Layer 4: NGINX Integration (lib.rs)      │  ← Imperative Shell
│  - Module registration                     │
│  - FFI glue code                           │
├────────────────────────────────────────────┤
│  Layer 3: I/O Boundaries                   │  ← Imperative Shell
│  - query.rs: SQLite operations             │
│  - template.rs: File system operations     │
│  - variable.rs: NGINX API calls            │
├────────────────────────────────────────────┤
│  Layer 2: Domain Logic (domain.rs)        │  ← Functional Core
│  - Pure business logic                     │
│  - Dependency injection via traits         │
│  - 100% testable with mocks                │
│  - Zero I/O operations                     │
├────────────────────────────────────────────┤
│  Layer 1: Types (types.rs)                │  ← Foundation
│  - Validated domain types                  │
│  - Parse-time validation                   │
│  - Zero dependencies                       │
└────────────────────────────────────────────┘
```

### Module Structure (1,727 total lines)

- **lib.rs** (389 lines): NGINX module registration and FFI handlers
- **domain.rs** (306 lines): Pure business logic with dependency injection
- **types.rs** (303 lines): Type-safe wrappers with validation
- **config.rs** (133 lines): Configuration data structures
- **query.rs** (327 lines): SQL execution (I/O boundary)
- **template.rs** (160 lines): Template loading and rendering (I/O boundary)
- **variable.rs** (106 lines): NGINX variable resolution (API boundary)
- **handler_types.rs**: Request processing orchestration
- **nginx_helpers.rs**: NGINX API utilities
- **parsing.rs**: Configuration parsing utilities
- **logging.rs**: Structured logging
- **content_type.rs**: Content type handling
- **adapters.rs**: Adapter implementations

### Design Principles

#### 1. Parse, Don't Validate

All validation happens at **parse time**, not use time. Types carry proof of validity:

```rust
// Bad: validate returns bool, data still invalid
fn is_valid_query(q: &str) -> bool { ... }

// Good: parse returns validated type
pub struct SqlQuery(String);
impl SqlQuery {
    pub fn parse(q: String) -> Result<Self, String> { ... }
}
```

**Domain Types with Invariants:**

| Type | Invariant | Enforced By |
|------|-----------|-------------|
| `SqlQuery` | Must be SELECT only | `parse()` validation |
| `TemplatePath` | Must end in .hbs | `parse()` validation |
| `NginxVariable` | Must start with $ | `parse()` validation |
| `ParamName` | Must start with : or be empty | `parse()` validation |
| `DatabasePath` | Must not be empty | `parse()` validation |

#### 2. Correctness by Construction

The type system makes invalid states **unrepresentable**. If you have a `SqlQuery`, it's guaranteed to be a valid SELECT statement.

#### 3. Dependency Injection via Traits

Core domain logic accepts trait-based dependencies, enabling:
- Testing without NGINX runtime
- Mocking I/O operations
- Deterministic test behavior

```rust
// Domain layer accepts abstractions
trait QueryExecutor {
    fn execute(...) -> Result<Vec<HashMap<String, Value>>, String>;
}

// Test with mocks
struct MockQueryExecutor;
impl QueryExecutor for MockQueryExecutor { ... }

// Production with real implementation
struct RealQueryExecutor;
impl QueryExecutor for RealQueryExecutor { ... }
```

#### 4. Ghost of Departed Proofs

Types carry compile-time guarantees about data provenance:

```rust
enum ParameterBinding {
    Positional { variable: NginxVariable },  // PROOF: validated variable
    Named { name: ParamName, variable: NginxVariable },  // PROOF: both valid
}
```

## Development Guidelines

### Module Organization

- **Single responsibility** per module
- Files should be **< 300 lines** (preferred)
- Tests co-located with code using `#[cfg(test)]`
- Clear separation: `types → domain → adapters → glue`

### Layering Rules

**MUST FOLLOW**:
1. **Layer 1 (types.rs)**: No dependencies on other modules
2. **Layer 2 (domain.rs)**: Depends only on types; uses trait-based DI
3. **Layer 3 (I/O)**: Implements domain traits; performs actual I/O
4. **Layer 4 (lib.rs)**: Glue code that wires everything together

**DO NOT**:
- Add I/O operations to domain.rs
- Call NGINX APIs from domain.rs
- Use concrete implementations in domain.rs (use traits)

### Code Quality

- **No dead code**: Delete unused functions immediately
- **Prefer pure functions**: Stateless, deterministic, testable
- **Keep handlers minimal**: Logic belongs in tested modules
- **Comprehensive tests**: Aim for >90% coverage

### Error Handling

- Use `Result` types, never exceptions
- Provide structured logging with context
- User-friendly error messages
- Fail fast on invalid configuration

### Testing Strategy

- **Pure functions**: Test with simple mock inputs
- **I/O operations**: Use temporary files (`/tmp/test_*`)
- **Independence**: Tests must be isolated and parallelizable
- **Mocking**: Use trait implementations for testing domain logic

## Key Configuration Directives

### `sqlite_db`
Set the SQLite database file path.
- **Context**: `location`
- **Syntax**: `sqlite_db path;`

### `sqlite_query`
Define the SQL SELECT query (only SELECT allowed).
- **Context**: `location`
- **Syntax**: `sqlite_query "SELECT ...";`

### `sqlite_template`
Specify the Handlebars template file.
- **Context**: `location`
- **Syntax**: `sqlite_template filename.hbs;`

### `sqlite_param`
Add query parameters (can be used multiple times).
- **Context**: `location`
- **Positional**: `sqlite_param $variable;`
- **Named** (recommended): `sqlite_param :name $variable;`

### `sqlite_global_templates`
Set global template directory.
- **Context**: `http`
- **Syntax**: `sqlite_global_templates directory;`

## Template Resolution

Templates resolve as: `{document_root}{uri}/{template_name}`

Example:
- `root "public"`
- `location /books`
- `sqlite_template "list.hbs"`
- → Resolves to: `public/books/list.hbs`

## Common Development Patterns

### Adding a New Domain Type

1. Add to **types.rs**:
```rust
pub struct MyType(String);
impl MyType {
    pub fn parse(input: String) -> Result<Self, String> {
        // Validation logic
        Ok(MyType(input))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_valid() { ... }
}
```

### Adding a New Trait for DI

1. Define trait in **domain.rs**:
```rust
pub trait MyTrait {
    fn do_something(&self, input: &str) -> Result<String, String>;
}
```

2. Implement in appropriate I/O module (query.rs, template.rs, etc.):
```rust
pub struct RealImplementation;
impl MyTrait for RealImplementation {
    fn do_something(&self, input: &str) -> Result<String, String> {
        // Real I/O implementation
    }
}
```

3. Mock for tests:
```rust
#[cfg(test)]
struct MockImplementation;
impl MyTrait for MockImplementation {
    fn do_something(&self, _: &str) -> Result<String, String> {
        Ok("mocked".to_string())
    }
}
```

## Build Troubleshooting

### nginx-sys Build Fails

**Problem**: "feature is disabled" or missing NGINX source

**Solution**: The project uses direnv/Nix environment. Ensure you:
1. Have direnv installed and run `direnv allow` in project directory
2. Use `direnv exec "$PWD"` prefix for all cargo commands
3. Alternatively, set `NGINX_SOURCE_DIR` or `NGINX_BUILD_DIR` environment variable

### macOS Linking Issues

The `build.rs` file handles macOS-specific linking with `-undefined dynamic_lookup`. This is required for dynamic library loading in NGINX.

## Security

- **SQL injection prevention**: Uses prepared statements exclusively
- **Read-only queries**: `SqlQuery` type enforces SELECT-only at parse time
- **UTF-8 validation**: All nginx variables validated
- **Path traversal protection**: Paths validated at parse time

## Performance

- **Zero-cost abstractions**: Newtype pattern compiles away
- **Parse-time validation**: No runtime overhead for type safety
- **Prepared statements**: Queries prepared once, reused
- **Template caching**: Templates loaded once and cached
