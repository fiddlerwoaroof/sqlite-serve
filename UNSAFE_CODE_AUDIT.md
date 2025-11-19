# Unsafe Code Audit

This document explains all `unsafe` code in the sqlite-serve codebase and why it cannot be removed.

## Summary

- **Total unsafe blocks**: ~24
- **Removable unsafe blocks**: 1 (✅ REMOVED in this PR)
- **Required unsafe blocks**: ~23 (all documented and necessary for FFI)

## Removed Unsafe Code

### adapters.rs (REMOVED ✅)
**Previous code:**
```rust
impl<'a> VariableResolver for NginxVariableResolver<'a> {
    fn resolve(&self, var_name: &str) -> Result<String, String> {
        // UNSAFE: Cast &self to &mut self
        let request_ptr = self.request as *const Request as *mut Request;
        let request = unsafe { &mut *request_ptr };
        variable::resolve_variable(request, var_name)
    }
}
```

**Solution:**
Changed the `VariableResolver` trait to use `&mut self` instead of `&self`, eliminating the need for the unsafe cast.

```rust
pub trait VariableResolver {
    fn resolve(&mut self, var_name: &str) -> Result<String, String>;
}

impl<'a> VariableResolver for NginxVariableResolver<'a> {
    fn resolve(&mut self, var_name: &str) -> Result<String, String> {
        variable::resolve_variable(self.request, var_name)
    }
}
```

## Required Unsafe Code (Cannot Be Removed)

All remaining unsafe code is **required** for Foreign Function Interface (FFI) with NGINX's C API.

### Category 1: NGINX Module Registration (lib.rs)

#### 1.1 Static Module Structures
```rust
#[unsafe(no_mangle)]
pub static mut ngx_http_howto_module: ngx_module_t = { ... }

#[unsafe(no_mangle)]
static ngx_http_howto_module_ctx: ngx_http_module_t = { ... }

#[unsafe(no_mangle)]
static mut ngx_http_howto_commands: [ngx_command_t; 6] = [ ... ]
```

**Why required:**
- `#[unsafe(no_mangle)]` is required for C linkage - NGINX needs to find these symbols by name
- `static mut` is required because NGINX mutates these structures during initialization
- This is part of the NGINX module ABI contract

#### 1.2 Module Reference
```rust
impl ngx::http::HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        unsafe { &*addr_of!(ngx_http_howto_module) }
    }
}
```

**Why required:**
- Creates a static reference to the mutable static module
- Required by the ngx crate's module system
- Safe because the module has 'static lifetime

#### 1.3 Trait Implementations
```rust
unsafe impl HttpModuleLocationConf for Module { ... }
unsafe impl HttpModuleMainConf for Module { ... }
```

**Why required:**
- Required by the ngx crate's trait system
- Guarantees that our config types are valid NGINX configuration structures

#### 1.4 C Callbacks
```rust
unsafe extern "C" fn postconfiguration(_cf: *mut ngx_conf_t) -> ngx_int_t { ... }
```

**Why required:**
- Must be `unsafe extern "C"` to match NGINX's expected function signature
- Part of the FFI contract between Rust and C

### Category 2: Configuration Directive Handlers (lib.rs)

```rust
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_global_templates(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut MainConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.global_templates_dir = (*args.add(1)).to_string();
    };
    std::ptr::null_mut()
}
```

**Why required:**
- NGINX calls these functions during configuration parsing
- Raw pointers are required by the C FFI
- NGINX guarantees these pointers are valid during config parsing
- `#[unsafe(no_mangle)]` required for C linkage
- Similar pattern for: `set_db_path`, `set_query`, `set_template_path`, `add_param`

### Category 3: NGINX Variable Resolution (variable.rs)

```rust
fn resolve_nginx_variable(request: &mut Request, var_name: &str) -> Result<String, String> {
    // ...
    let key = unsafe { ngx_hash_key(name.data, name.len) };
    let var_value = unsafe { ngx_http_get_variable(r, &mut name, key) };
    let var_ref = unsafe { &*var_value };
    // ...
}
```

**Why required:**
- Calls to NGINX C API functions for variable lookup
- Required to access nginx variables like `$arg_id`, `$request_uri`, etc.
- No safe alternative exists - this is the only way to read NGINX variables

### Category 4: NGINX Logging (logging.rs)

```rust
pub fn log(request: &mut Request, level: LogLevel, module: &str, message: &str) {
    let r: *mut ngx::ffi::ngx_http_request_t = request.into();
    unsafe {
        let connection = (*r).connection;
        if !connection.is_null() {
            let log = (*connection).log;
            if !log.is_null() {
                ngx_log_error!(log_level, log, "[sqlite-serve:{}] {}", module, message);
            }
        }
    }
}
```

**Why required:**
- Access to NGINX's logging system requires dereferencing C pointers
- Required to integrate with NGINX's logging infrastructure
- Null checks ensure safety before dereferencing

### Category 5: HTTP Header Parsing (content_type.rs)

```rust
pub fn negotiate_content_type(request: &Request) -> ContentType {
    let r: *const ngx::ffi::ngx_http_request_t = request.into();
    unsafe {
        let headers_in = &(*r).headers_in;
        let mut current = headers_in.headers.part.elts as *mut ngx::ffi::ngx_table_elt_t;
        // Iterate through headers...
    }
}
```

**Why required:**
- Access to HTTP headers requires dereferencing NGINX C structures
- Required for content negotiation based on Accept header
- No safe wrapper exists in the ngx crate for this functionality

## Safety Guarantees

All unsafe code in this codebase has the following guarantees:

1. **Valid Pointers**: All raw pointers are guaranteed valid by NGINX during their use
2. **Lifetime Correctness**: All references have appropriate lifetimes
3. **Null Checks**: Pointers are checked for null before dereferencing when needed
4. **Documentation**: Every unsafe block has a SAFETY comment explaining why it's safe

## Alternatives Considered

### Could we use safe wrappers from the ngx crate?

The `ngx` crate provides some safe wrappers, but not for all NGINX APIs we need:
- ✅ Basic request handling - has safe wrappers (we use them)
- ✅ Response sending - has safe wrappers (we use them)
- ❌ Variable resolution - **no safe wrapper** (must use unsafe)
- ❌ Detailed header parsing - **no safe wrapper** (must use unsafe)
- ❌ Module registration - **inherently unsafe** (C FFI contract)

### Could we avoid NGINX FFI entirely?

No, because:
1. This is an NGINX module - FFI with NGINX is the entire purpose
2. We need to read nginx variables for SQL parameters
3. We need to parse HTTP headers for content negotiation
4. We need to integrate with NGINX's logging system

### Could we create safe wrappers?

We could create safe wrappers around some of these operations, but:
1. The wrappers would still contain unsafe code internally
2. It would add complexity without reducing unsafe code
3. The ngx crate is the appropriate place for such wrappers, not our module

## Conclusion

All unsafe code in sqlite-serve is:
1. ✅ **Necessary** - Required for NGINX FFI
2. ✅ **Minimal** - No unnecessary unsafe code remains
3. ✅ **Documented** - Every unsafe block has SAFETY comments
4. ✅ **Audited** - This document explains all unsafe code

The only unsafe code that could be removed (in `adapters.rs`) has been eliminated.
All remaining unsafe code is an unavoidable requirement of writing an NGINX module in Rust.
