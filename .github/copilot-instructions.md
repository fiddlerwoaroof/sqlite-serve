# GitHub Copilot Instructions for sqlite-serve

## Project Overview

sqlite-serve is a dynamic NGINX module written in Rust that integrates SQLite databases with Handlebars templating. It enables data-driven web applications directly from NGINX configuration without requiring a separate application server.

## Environment Setup

### Build Environment

- **ALWAYS use `direnv exec "$PWD"` when invoking cargo and other tools** that need access to the nix/direnv environment
- The project uses Nix for dependency management
- Example: `direnv exec "$PWD" cargo build`
- Example: `direnv exec "$PWD" cargo test`

### Build Requirements

- Rust 2024 edition
- NGINX source must be available (set via `NGINX_SOURCE_DIR` or `NGINX_BUILD_DIR` environment variables)
  - Alternative: Use the `vendored` feature in ngx dependency to auto-download NGINX
- direnv/Nix environment for proper dependency resolution

### Avoid Common Pitfalls

- **Avoid using `head` and `tail`** when reading command output to prevent missing important information
- Always disable pagers in git commands: `git --no-pager status`, `git --no-pager diff`

## Design Principles

### 1. Parse, Don't Validate

- **Create newtype wrappers** for domain concepts (DatabasePath, SqlQuery, TemplatePath, etc.)
- **Validate at parse time, not at use time**
- Use Result types for parsing - if you have the type, it's valid
- **Make illegal states unrepresentable**

Example:
```rust
pub struct SqlQuery(String);

impl SqlQuery {
    pub fn parse(q: String) -> Result<Self, String> {
        if !q.trim().to_uppercase().starts_with("SELECT") {
            return Err("only SELECT queries allowed");
        }
        Ok(SqlQuery(q))
    }
}
```

### 2. Correctness by Construction

- **Use the type system to enforce invariants**
- Prefer types that can only represent valid states
- Example: SqlQuery can only be constructed from SELECT statements

**Domain Types and Their Invariants:**

| Type | Invariant | Enforced By |
|------|-----------|-------------|
| `SqlQuery` | Must be SELECT | `parse()` validation |
| `TemplatePath` | Must end in .hbs | `parse()` validation |
| `NginxVariable` | Must start with $ | `parse()` validation |
| `ParamName` | Must start with : or be empty | `parse()` validation |
| `DatabasePath` | Must not be empty | `parse()` validation |

### 3. Functional Core, Imperative Shell

- **Keep business logic pure and testable** (domain.rs)
- **Move I/O to adapter layer** (adapters.rs, query.rs, template.rs, variable.rs)
- **Use dependency injection via traits**
- Handler should be minimal orchestration only

Architecture layers:
```
┌─────────────────────────────────────────┐
│          Imperative Shell               │
│  (lib.rs - NGINX FFI, actual I/O)       │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │      Functional Core                │ │
│  │  (domain.rs - pure logic)           │ │
│  │                                      │ │
│  │  • No I/O                            │ │
│  │  • No NGINX API calls                │ │
│  │  • 100% testable                     │ │
│  │  • Deterministic                     │ │
│  └────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

### 4. Ghost of Departed Proofs

- **Use proof tokens to carry compile-time guarantees**
- Functions accept tokens (validated types), not raw data
- Type system enforces the proofs

Example:
```rust
enum ParameterBinding {
    Positional { variable: NginxVariable },     // PROOF: is a valid variable
    Named { name: ParamName, variable: NginxVariable }, // PROOF: both valid
}
```

## Module Organization

### Directory Structure

```
src/
├── lib.rs (389 lines)          # NGINX module registration (imperative shell)
├── domain.rs (306 lines)       # Pure business logic (functional core)
├── types.rs (303 lines)        # Type-safe wrappers (parse, don't validate)
├── config.rs (133 lines)       # Configuration structures
├── query.rs (327 lines)        # SQL execution (I/O boundary)
├── template.rs (160 lines)     # Template loading (I/O boundary)
├── variable.rs (106 lines)     # Variable resolution (nginx API boundary)
└── main.rs (3 lines)           # Unused entry point
```

### Module Guidelines

- **Single responsibility per module**
- **Small, focused files** (< 300 lines preferred)
- **Tests co-located with code** (#[cfg(test)])
- **Clear separation**: types → domain → adapters → glue

### Layering Rules

**Layer 1: Types (types.rs)**
- Validated domain types
- Zero dependencies on other modules
- Pure validation logic

**Layer 2: Domain (domain.rs)**
- Pure business logic
- Depends only on types
- Dependency injection via traits
- 100% testable with mocks

**Layer 3: I/O Boundaries (query.rs, template.rs, variable.rs)**
- Implement domain traits
- Actual I/O operations
- SQLite access, file system, nginx API

**Layer 4: NGINX Integration (lib.rs)**
- Module registration
- Directive handlers
- Request handler (glue code)
- Calls domain layer with real implementations

## Error Handling

- **Use Result types, not exceptions**
- **Structured logging with context**
- **User-friendly error messages**
- **Fail fast on invalid configuration**

Example:
```rust
fn process_query(query: &str) -> Result<SqlQuery, String> {
    SqlQuery::parse(query.to_string())
        .map_err(|e| format!("Invalid query: {}", e))
}
```

## Code Quality

### General Rules

- **No dead code** - delete unused functions immediately
- **Comprehensive tests for all modules**
- **Prefer pure functions over stateful code**
- **Keep handlers minimal** - logic belongs in tested modules

### Testing Requirements

- **Test pure functions with simple inputs**
- **Use mock implementations for DI traits**
- **Aim for >90% coverage**
- **Tests should be fast and isolated**

Example mock:
```rust
struct MockQueryExecutor;
impl QueryExecutor for MockQueryExecutor {
    fn execute(...) -> Result<Vec<HashMap<String, Value>>, String> {
        Ok(vec![test_row()])
    }
}
```

### Test Organization

- All tests are independent
- Use temporary files (`/tmp/test_*`)
- Clean up after themselves
- No shared state
- Can run in parallel

## Development Workflow

### Building

```bash
# Check compilation
direnv exec "$PWD" cargo check

# Full build
direnv exec "$PWD" cargo build

# Release build
direnv exec "$PWD" cargo build --release
```

### Testing

```bash
# Run all tests
direnv exec "$PWD" cargo test

# Run specific test
direnv exec "$PWD" cargo test test_name

# Run with output
direnv exec "$PWD" cargo test -- --nocapture
```

### Running Examples

```bash
# Book catalog example (port 8080)
./start_book_catalog.sh

# Book detail with parameters (port 8081)
./start_book_detail.sh

# Named parameters example (port 8082)
./start_book_named_params.sh
```

### Debugging

Enable debug logging in nginx configuration:
```nginx
error_log logs/error.log debug;
```

Check logs:
```bash
tail -f logs/error.log | grep sqlite
```

## Dependencies

Core dependencies:
- **ngx** (0.5.0) - Rust bindings for NGINX
- **rusqlite** (0.37.0) - SQLite integration
- **handlebars** (6.3.2) - Template engine
- **serde** & **serde_json** - JSON serialization

## Security Considerations

- **SQL injection prevention**: Use prepared statements with parameterized queries
- **Read-only queries**: SqlQuery type enforces SELECT-only statements
- **UTF-8 validation**: All nginx variables are validated for UTF-8
- **Path traversal protection**: Paths are validated at parse time

## Common Patterns

### Creating a New Domain Type

```rust
pub struct MyType(String);

impl MyType {
    pub fn parse(input: String) -> Result<Self, String> {
        // Validation logic
        if input.is_empty() {
            return Err("cannot be empty".to_string());
        }
        Ok(MyType(input))
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_valid() {
        assert!(MyType::parse("valid".to_string()).is_ok());
    }
    
    #[test]
    fn test_parse_invalid() {
        assert!(MyType::parse("".to_string()).is_err());
    }
}
```

### Adding a New Trait for Dependency Injection

```rust
// In domain.rs
pub trait MyTrait {
    fn do_something(&self, input: &str) -> Result<String, String>;
}

// In adapters.rs or query.rs
pub struct RealImplementation;

impl MyTrait for RealImplementation {
    fn do_something(&self, input: &str) -> Result<String, String> {
        // Real implementation with I/O
        Ok(format!("processed: {}", input))
    }
}

// In tests
#[cfg(test)]
mod tests {
    struct MockImplementation;
    
    impl MyTrait for MockImplementation {
        fn do_something(&self, _input: &str) -> Result<String, String> {
            Ok("mocked".to_string())
        }
    }
}
```

## NGINX Configuration Examples

### Simple Query
```nginx
location = /books {
    sqlite_db "catalog.db";
    sqlite_query "SELECT * FROM books ORDER BY title";
    sqlite_template "list.hbs";
}
```

### Named Parameters (Recommended)
```nginx
location = /book {
    sqlite_db "catalog.db";
    sqlite_query "SELECT * FROM books WHERE id = :book_id";
    sqlite_param :book_id $arg_id;
    sqlite_template "detail.hbs";
}
```

### Positional Parameters
```nginx
location = /search {
    sqlite_db "catalog.db";
    sqlite_query "SELECT * FROM books WHERE year >= ? AND year <= ?";
    sqlite_param $arg_min;  # First ?
    sqlite_param $arg_max;  # Second ?
    sqlite_template "list.hbs";
}
```

## Troubleshooting

### Build Issues

**Problem**: `nginx-sys` build fails with "feature is disabled"
**Solution**: 
- Ensure `NGINX_SOURCE_DIR` or `NGINX_BUILD_DIR` is set, or use the nix/direnv environment
- Alternative: Temporarily enable the `vendored` feature in the ngx dependency in Cargo.toml:
  ```toml
  ngx = { version = "0.5.0", features = ["vendored"] }
  ```
  This will download and build NGINX automatically during compilation.

**Problem**: `direnv: command not found`
**Solution**: Install direnv and nix, then run `direnv allow` in the project directory

### Runtime Issues

**Problem**: Module not loading
**Solution**: Check that the module path in nginx.conf is correct and the .so file exists

**Problem**: Template not found
**Solution**: Verify template path resolution: `{document_root}{uri}/{template_name}`

**Problem**: SQL errors
**Solution**: Check logs with `tail -f logs/error.log | grep sqlite` and verify query syntax

## Performance Considerations

- **Type safety has zero runtime cost**: All validation happens at parse time or compile time
- **Newtype pattern compiles away**: No memory overhead
- **Prepared statements**: Queries are prepared once and reused
- **Template caching**: Templates are loaded once and cached

## Contributing Guidelines

When adding new features:

1. **Start with types**: Define validated types in types.rs
2. **Add pure logic**: Implement business logic in domain.rs with trait-based DI
3. **Implement I/O**: Add concrete implementations in query.rs, template.rs, etc.
4. **Wire it up**: Connect everything in lib.rs
5. **Write tests**: Add unit tests for each layer
6. **Document**: Update README.md and configuration examples

Always maintain the layered architecture and follow the design principles above.

## Additional Resources

- **NGINX Module Development**: https://nginx.org/en/docs/dev/development_guide.html
- **ngx Rust Crate**: https://crates.io/crates/ngx
- **Handlebars Rust**: https://crates.io/crates/handlebars
- **Rusqlite**: https://crates.io/crates/rusqlite
- **Architecture Documentation**: See ARCHITECTURE.md for detailed design patterns
