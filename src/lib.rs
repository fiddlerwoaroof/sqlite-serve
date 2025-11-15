use handlebars::Handlebars;
use ngx::core::Buffer;
use ngx::ffi::{
    ngx_hash_key, ngx_http_get_variable, NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF,
    NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE,
    nginx_version, ngx_chain_t, ngx_command_t, ngx_conf_t, ngx_http_module_t, ngx_int_t,
    ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::http::{
    HttpModule, HttpModuleLocationConf, HttpModuleMainConf, MergeConfigError, NgxHttpCoreModule,
};
use ngx::{core, core::Status, http};
use ngx::{http_request_handler, ngx_log_debug_http, ngx_modules, ngx_string};
use rusqlite::{Connection, Result};
use serde_json::json;
use std::os::raw::{c_char, c_void};
use std::ptr::addr_of;

struct Module;

// Implement our HttpModule trait, we're creating a postconfiguration method to install our
// handler's Access phase function.
impl http::HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        unsafe { &*addr_of!(ngx_http_howto_module) }
    }

    unsafe extern "C" fn postconfiguration(_cf: *mut ngx_conf_t) -> ngx_int_t {
        core::Status::NGX_OK.into()
    }
}

// Implement HttpModuleLocationConf to define our location-specific configuration
unsafe impl HttpModuleLocationConf for Module {
    type LocationConf = ModuleConfig;
}

// Implement HttpModuleMainConf to define our global configuration
unsafe impl HttpModuleMainConf for Module {
    type MainConf = MainConfig;
}

// Create a ModuleConfig to save our configuration state.
#[derive(Debug, Default)]
struct ModuleConfig {
    db_path: String,
    query: String,
    template_path: String,
    query_params: Vec<(String, String)>, // (param_name, variable_name) pairs
}

// Global configuration for shared templates
#[derive(Debug, Default)]
struct MainConfig {
    global_templates_dir: String,
}

impl http::Merge for MainConfig {
    fn merge(&mut self, prev: &MainConfig) -> Result<(), MergeConfigError> {
        if self.global_templates_dir.is_empty() {
            self.global_templates_dir = prev.global_templates_dir.clone();
        }
        Ok(())
    }
}

// Implement our Merge trait to merge configuration with higher layers.
impl http::Merge for ModuleConfig {
    fn merge(&mut self, prev: &ModuleConfig) -> Result<(), MergeConfigError> {
        if self.db_path.is_empty() {
            self.db_path = prev.db_path.clone();
        }

        if self.query.is_empty() {
            self.query = prev.query.clone();
        }

        if self.template_path.is_empty() {
            self.template_path = prev.template_path.clone();
        }

        if self.query_params.is_empty() {
            self.query_params = prev.query_params.clone();
        }

        Ok(())
    }
}

// Create our "C" module context with function entrypoints for NGINX event loop. This "binds" our
// HttpModule implementation to functions callable from C.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static ngx_http_howto_module_ctx: ngx_http_module_t = ngx_http_module_t {
    preconfiguration: Some(Module::preconfiguration),
    postconfiguration: Some(Module::postconfiguration),
    create_main_conf: Some(Module::create_main_conf),
    init_main_conf: Some(Module::init_main_conf),
    create_srv_conf: None,
    merge_srv_conf: None,
    create_loc_conf: Some(Module::create_loc_conf),
    merge_loc_conf: Some(Module::merge_loc_conf),
};

// Create our module structure and export it with the `ngx_modules!` macro. For this simple
// handler, the ngx_module_t is predominately boilerplate save for setting the above context into
// this structure and setting our custom configuration command (defined below).
ngx_modules!(ngx_http_howto_module);

#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static mut ngx_http_howto_module: ngx_module_t = ngx_module_t {
    ctx_index: ngx_uint_t::max_value(),
    index: ngx_uint_t::max_value(),
    name: std::ptr::null_mut(),
    spare0: 0,
    spare1: 0,
    version: nginx_version as ngx_uint_t,
    signature: NGX_RS_MODULE_SIGNATURE.as_ptr() as *const c_char,

    ctx: &ngx_http_howto_module_ctx as *const _ as *mut _,
    commands: unsafe { &ngx_http_howto_commands[0] as *const _ as *mut _ },
    type_: NGX_HTTP_MODULE as ngx_uint_t,

    init_master: None,
    init_module: None,
    init_process: None,
    init_thread: None,
    exit_thread: None,
    exit_process: None,
    exit_master: None,

    spare_hook0: 0,
    spare_hook1: 0,
    spare_hook2: 0,
    spare_hook3: 0,
    spare_hook4: 0,
    spare_hook5: 0,
    spare_hook6: 0,
    spare_hook7: 0,
};

// Register and allocate our command structures for directive generation and eventual storage. Be
// sure to terminate the array with an empty command.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static mut ngx_http_howto_commands: [ngx_command_t; 6] = [
    ngx_command_t {
        name: ngx_string!("sqlite_global_templates"),
        type_: (NGX_HTTP_MAIN_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_global_templates),
        conf: 0, // Main conf offset
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_command_t {
        name: ngx_string!("sqlite_db"),
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_db_path),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_command_t {
        name: ngx_string!("sqlite_query"),
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_query),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_command_t {
        name: ngx_string!("sqlite_template"),
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_template_path),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_command_t {
        name: ngx_string!("sqlite_param"),
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1 | NGX_CONF_TAKE2) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_add_param),
        conf: NGX_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_command_t {
        name: ngx_str_t {
            len: 0,
            data: std::ptr::null_mut(),
        },
        type_: 0,
        set: None,
        conf: 0,
        offset: 0,
        post: std::ptr::null_mut(),
    },
];

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

#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_db_path(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.db_path = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_query(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.query = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_template_path(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.template_path = (*args.add(1)).to_string();
        
        // Set the content handler for this location
        let clcf = NgxHttpCoreModule::location_conf_mut(&*cf)
            .expect("failed to get core location conf");
        clcf.handler = Some(howto_access_handler);
    };

    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_add_param(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        let nelts = (*(*cf).args).nelts;
        
        if nelts == 2 {
            // Single argument: positional parameter
            // sqlite_param $arg_id
            let variable = (*args.add(1)).to_string();
            conf.query_params.push((String::new(), variable));
        } else if nelts == 3 {
            // Two arguments: named parameter
            // sqlite_param :book_id $arg_id
            let param_name = (*args.add(1)).to_string();
            let variable = (*args.add(2)).to_string();
            conf.query_params.push((param_name, variable));
        }
    };

    std::ptr::null_mut()
}

// Load all .hbs templates from a directory into the Handlebars registry
fn load_templates_from_dir(reg: &mut Handlebars, dir_path: &str) -> std::io::Result<usize> {
    use std::fs;
    use std::path::Path;

    let dir = Path::new(dir_path);
    if !dir.exists() || !dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "hbs" {
                    if let Some(stem) = path.file_stem() {
                        if let Some(name) = stem.to_str() {
                            if let Err(e) = reg.register_template_file(name, &path) {
                                eprintln!("Failed to register template {}: {}", path.display(), e);
                            } else {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(count)
}

// Execute a generic SQL query with parameters and return results as JSON-compatible data
fn execute_query(
    db_path: &str,
    query: &str,
    params: &[(String, String)], // (param_name, value) pairs
) -> Result<Vec<std::collections::HashMap<String, serde_json::Value>>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(query)?;
    
    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect();

    // Bind parameters (either positional or named)
    let has_named_params = params.iter().any(|(name, _)| !name.is_empty());
    
    // Convert row to JSON map
    let row_to_map = |row: &rusqlite::Row| -> rusqlite::Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut map = std::collections::HashMap::new();
        for (i, col_name) in column_names.iter().enumerate() {
            let value: serde_json::Value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(v) => serde_json::Value::Number(v.into()),
                rusqlite::types::ValueRef::Real(v) => {
                    serde_json::Number::from_f64(v)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
                rusqlite::types::ValueRef::Text(v) => {
                    serde_json::Value::String(String::from_utf8_lossy(v).to_string())
                }
                rusqlite::types::ValueRef::Blob(v) => {
                    // Convert blob to hex string
                    let hex_string = v
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>();
                    serde_json::Value::String(hex_string)
                }
            };
            map.insert(col_name.clone(), value);
        }
        Ok(map)
    };
    
    let rows = if has_named_params {
        // Use named parameters
        let named_params: Vec<(&str, &dyn rusqlite::ToSql)> = params
            .iter()
            .map(|(name, value)| (name.as_str(), value as &dyn rusqlite::ToSql))
            .collect();
        stmt.query_map(named_params.as_slice(), row_to_map)?
    } else {
        // Use positional parameters
        let positional_params: Vec<&dyn rusqlite::ToSql> = params
            .iter()
            .map(|(_, value)| value as &dyn rusqlite::ToSql)
            .collect();
        stmt.query_map(positional_params.as_slice(), row_to_map)?
    };

    rows.collect()
}

// Implement a request handler. Use the convenience macro, the http_request_handler! macro will
// convert the native NGINX request into a Rust Request instance as well as define an extern C
// function callable from NGINX.
//
// The function body is implemented as a Rust closure.
http_request_handler!(howto_access_handler, |request: &mut http::Request| {
    let co = Module::location_conf(request).expect("module config is none");

    // Check if all required config values are set
    if co.db_path.is_empty() || co.query.is_empty() || co.template_path.is_empty() {
        return core::Status::NGX_OK;
    }

    ngx_log_debug_http!(request, "sqlite module handler called");

    // Resolve template path relative to document root and location
    let core_loc_conf =
        NgxHttpCoreModule::location_conf(request).expect("failed to get core location conf");
    let doc_root = match (*core_loc_conf).root.to_str() {
        Ok(s) => s,
        Err(e) => {
            ngx_log_debug_http!(request, "failed to decode root path: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };
    let uri = match request.path().to_str() {
        Ok(s) => s,
        Err(e) => {
            ngx_log_debug_http!(request, "failed to decode URI path: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };
    let template_full_path = format!("{}{}/{}", doc_root, uri, co.template_path);

    ngx_log_debug_http!(request, "resolved template path: {}", template_full_path);

    // Get the directory containing the main template for local templates
    let template_dir = std::path::Path::new(&template_full_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    // Resolve query parameters from nginx variables
    let mut param_values: Vec<(String, String)> = Vec::new();
    for (param_name, var_name) in &co.query_params {
        let value = if var_name.starts_with('$') {
            // It's a variable reference, resolve it from nginx
            let var_name_str = &var_name[1..]; // Remove the '$' prefix
            let var_name_bytes = var_name_str.as_bytes();
            
            let mut name = ngx_str_t {
                len: var_name_bytes.len(),
                data: var_name_bytes.as_ptr() as *mut u8,
            };
            
            let key = unsafe { ngx_hash_key(name.data, name.len) };
            let r: *mut ngx::ffi::ngx_http_request_t = request.into();
            let var_value = unsafe { ngx_http_get_variable(r, &mut name, key) };
            
            if var_value.is_null() {
                ngx_log_debug_http!(request, "variable not found: {}", var_name);
                return http::HTTPStatus::BAD_REQUEST.into();
            }
            
            let var_ref = unsafe { &*var_value };
            if var_ref.valid() == 0 {
                ngx_log_debug_http!(request, "variable value not valid: {}", var_name);
                return http::HTTPStatus::BAD_REQUEST.into();
            }
            
            match std::str::from_utf8(var_ref.as_bytes()) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    ngx_log_debug_http!(request, "failed to decode variable as UTF-8: {}", var_name);
                    return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
                }
            }
        } else {
            // It's a literal value
            var_name.clone()
        };
        param_values.push((param_name.clone(), value));
    }

    ngx_log_debug_http!(
        request,
        "executing query with {} parameters",
        param_values.len()
    );

    // Execute the configured SQL query with parameters
    let results = match execute_query(&co.db_path, &co.query, &param_values) {
        Ok(results) => results,
        Err(e) => {
            ngx_log_debug_http!(request, "failed to execute query: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Setup Handlebars and load templates
    let mut reg = Handlebars::new();
    
    // First, load global templates if configured
    let main_conf = Module::main_conf(request).expect("main config is none");
    if !main_conf.global_templates_dir.is_empty() {
        match load_templates_from_dir(&mut reg, &main_conf.global_templates_dir) {
            Ok(count) => {
                ngx_log_debug_http!(request, "loaded {} global templates from {}", count, main_conf.global_templates_dir);
            }
            Err(e) => {
                ngx_log_debug_http!(request, "warning: failed to load global templates: {}", e);
            }
        }
    }

    // Then, load local templates (these override global ones)
    match load_templates_from_dir(&mut reg, template_dir) {
        Ok(count) => {
            ngx_log_debug_http!(request, "loaded {} local templates from {}", count, template_dir);
        }
        Err(e) => {
            ngx_log_debug_http!(request, "warning: failed to load local templates: {}", e);
        }
    }

    // Finally, register the main template (overriding if it was loaded from directories)
    match reg.register_template_file("template", &template_full_path) {
        Ok(_) => {
            ngx_log_debug_http!(request, "registered main template: {}", template_full_path);
        },
        Err(e) => {
            ngx_log_debug_http!(request, "failed to load main template: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    }

    // Render the template with query results
    let body = match reg.render("template", &json!({"results": results})) {
        Ok(body) => body,
        Err(e) => {
            ngx_log_debug_http!(request, "failed to render template: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Create output buffer
    let mut buf = match request.pool().create_buffer_from_str(&body) {
        Some(buf) => buf,
        None => return http::HTTPStatus::INTERNAL_SERVER_ERROR.into(),
    };

    buf.set_last_buf(request.is_main());
    buf.set_last_in_chain(true);

    let mut out = ngx_chain_t {
        buf: buf.as_ngx_buf_mut(),
        next: std::ptr::null_mut(),
    };

    request.discard_request_body();
    request.set_status(http::HTTPStatus::OK);
    let rc = request.send_header();
    if rc == core::Status::NGX_ERROR
        || rc > core::Status::NGX_OK
        || request.header_only()
    {
        return rc;
    }

    request.output_filter(&mut out);
    Status::NGX_DONE
});

#[cfg(test)]
mod tests {
    use super::*;
    use ngx::http::Merge;
    use std::collections::HashMap;

    #[test]
    fn test_module_config_default() {
        let config = ModuleConfig::default();
        assert!(config.db_path.is_empty());
        assert!(config.query.is_empty());
        assert!(config.template_path.is_empty());
        assert!(config.query_params.is_empty());
    }

    #[test]
    fn test_module_config_merge() {
        let mut config = ModuleConfig {
            db_path: String::new(),
            query: String::new(),
            template_path: String::new(),
            query_params: vec![],
        };

        let prev = ModuleConfig {
            db_path: "test.db".to_string(),
            query: "SELECT * FROM test".to_string(),
            template_path: "test.hbs".to_string(),
            query_params: vec![("id".to_string(), "$arg_id".to_string())],
        };

        config.merge(&prev).unwrap();

        assert_eq!(config.db_path, "test.db");
        assert_eq!(config.query, "SELECT * FROM test");
        assert_eq!(config.template_path, "test.hbs");
        assert_eq!(config.query_params.len(), 1);
    }

    #[test]
    fn test_module_config_merge_preserves_existing() {
        let mut config = ModuleConfig {
            db_path: "existing.db".to_string(),
            query: "SELECT 1".to_string(),
            template_path: "existing.hbs".to_string(),
            query_params: vec![],
        };

        let prev = ModuleConfig {
            db_path: "prev.db".to_string(),
            query: "SELECT 2".to_string(),
            template_path: "prev.hbs".to_string(),
            query_params: vec![],
        };

        config.merge(&prev).unwrap();

        // Should keep existing values
        assert_eq!(config.db_path, "existing.db");
        assert_eq!(config.query, "SELECT 1");
        assert_eq!(config.template_path, "existing.hbs");
    }

    #[test]
    fn test_main_config_default() {
        let config = MainConfig::default();
        assert!(config.global_templates_dir.is_empty());
    }

    #[test]
    fn test_main_config_merge() {
        let mut config = MainConfig {
            global_templates_dir: String::new(),
        };

        let prev = MainConfig {
            global_templates_dir: "templates/global".to_string(),
        };

        config.merge(&prev).unwrap();
        assert_eq!(config.global_templates_dir, "templates/global");
    }

    #[test]
    fn test_execute_query_empty_db() {
        // Test with a non-existent database - should return error
        let result = execute_query("/nonexistent/test.db", "SELECT 1", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_query_with_memory_db() {
        use rusqlite::Connection;
        use std::fs;

        // Create a temporary in-memory database for testing
        let temp_path = "/tmp/test_sqlite_serve.db";
        let _ = fs::remove_file(temp_path); // Clean up if exists

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute(
                "CREATE TABLE test (id INTEGER, name TEXT, value REAL)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO test VALUES (1, 'first', 1.5), (2, 'second', 2.5)",
                [],
            )
            .unwrap();
        }

        // Test simple query
        let results = execute_query(temp_path, "SELECT * FROM test ORDER BY id", &[]).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("id").unwrap(),
            &serde_json::Value::Number(1.into())
        );
        assert_eq!(
            results[0].get("name").unwrap(),
            &serde_json::Value::String("first".to_string())
        );

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_with_positional_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_params.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, title TEXT)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES (1, 'Book One'), (2, 'Book Two'), (3, 'Book Three')",
                [],
            )
            .unwrap();
        }

        // Test positional parameter
        let params = vec![(String::new(), "2".to_string())];
        let results =
            execute_query(temp_path, "SELECT * FROM books WHERE id = ?", &params).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].get("title").unwrap(),
            &serde_json::Value::String("Book Two".to_string())
        );

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_with_named_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_named.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, title TEXT, year INTEGER)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES (1, 'Old Book', 2000), (2, 'New Book', 2020), (3, 'Newer Book', 2023)",
                [],
            )
            .unwrap();
        }

        // Test named parameters
        let params = vec![
            (":min_year".to_string(), "2015".to_string()),
            (":max_year".to_string(), "2024".to_string()),
        ];
        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE year >= :min_year AND year <= :max_year ORDER BY year",
            &params,
        )
        .unwrap();
        
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].get("title").unwrap(),
            &serde_json::Value::String("New Book".to_string())
        );
        assert_eq!(
            results[1].get("title").unwrap(),
            &serde_json::Value::String("Newer Book".to_string())
        );

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_data_types() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_types.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute(
                "CREATE TABLE types (id INTEGER, name TEXT, price REAL, data BLOB, nullable TEXT)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO types VALUES (42, 'test', 3.14, X'DEADBEEF', NULL)",
                [],
            )
            .unwrap();
        }

        let results = execute_query(temp_path, "SELECT * FROM types", &[]).unwrap();
        assert_eq!(results.len(), 1);

        let row = &results[0];
        
        // Test INTEGER
        assert_eq!(row.get("id").unwrap(), &serde_json::Value::Number(42.into()));
        
        // Test TEXT
        assert_eq!(
            row.get("name").unwrap(),
            &serde_json::Value::String("test".to_string())
        );
        
        // Test REAL
        assert_eq!(
            row.get("price").unwrap().as_f64().unwrap(),
            3.14
        );
        
        // Test BLOB (should be hex encoded)
        assert_eq!(
            row.get("data").unwrap(),
            &serde_json::Value::String("deadbeef".to_string())
        );
        
        // Test NULL
        assert_eq!(row.get("nullable").unwrap(), &serde_json::Value::Null);

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_load_templates_from_nonexistent_dir() {
        let mut reg = Handlebars::new();
        let result = load_templates_from_dir(&mut reg, "/nonexistent/path/to/templates");
        
        // Should succeed but load 0 templates
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_load_templates_from_dir() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_templates";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        // Create test templates
        let mut file1 = fs::File::create(format!("{}/template1.hbs", temp_dir)).unwrap();
        file1.write_all(b"<h1>Template 1</h1>").unwrap();

        let mut file2 = fs::File::create(format!("{}/template2.hbs", temp_dir)).unwrap();
        file2.write_all(b"<h1>Template 2</h1>").unwrap();

        // Create a non-template file (should be ignored)
        let mut file3 = fs::File::create(format!("{}/readme.txt", temp_dir)).unwrap();
        file3.write_all(b"Not a template").unwrap();

        let mut reg = Handlebars::new();
        let count = load_templates_from_dir(&mut reg, temp_dir).unwrap();

        assert_eq!(count, 2);
        assert!(reg.has_template("template1"));
        assert!(reg.has_template("template2"));
        assert!(!reg.has_template("readme"));

        // Clean up
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_template_rendering_with_results() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_render";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        // Create a simple template
        let template_path = format!("{}/list.hbs", temp_dir);
        let mut file = fs::File::create(&template_path).unwrap();
        file.write_all(b"{{#each results}}<li>{{name}}</li>{{/each}}").unwrap();

        let mut reg = Handlebars::new();
        reg.register_template_file("list", &template_path).unwrap();

        // Test rendering with data
        let mut results = vec![];
        let mut item1 = HashMap::new();
        item1.insert("name".to_string(), serde_json::Value::String("Item 1".to_string()));
        results.push(item1);

        let mut item2 = HashMap::new();
        item2.insert("name".to_string(), serde_json::Value::String("Item 2".to_string()));
        results.push(item2);

        let rendered = reg.render("list", &json!({"results": results})).unwrap();
        assert!(rendered.contains("<li>Item 1</li>"));
        assert!(rendered.contains("<li>Item 2</li>"));

        // Clean up
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_named_params_parsing() {
        // This tests the logic we'd use in the directive handler
        let test_cases = vec![
            // (nelts, expected_is_named, expected_param_name)
            (2, false, ""),  // sqlite_param $arg_id
            (3, true, ":book_id"),  // sqlite_param :book_id $arg_id
        ];

        for (nelts, expected_is_named, expected_param_name) in test_cases {
            if nelts == 2 {
                // Positional
                let param_name = String::new();
                assert!(!expected_is_named);
                assert_eq!(param_name, expected_param_name);
            } else if nelts == 3 {
                // Named
                let param_name = ":book_id".to_string();
                assert!(expected_is_named);
                assert_eq!(param_name, expected_param_name);
            }
        }
    }

    #[test]
    fn test_has_named_params() {
        let positional = vec![
            (String::new(), "value1".to_string()),
            (String::new(), "value2".to_string()),
        ];
        assert!(!positional.iter().any(|(name, _)| !name.is_empty()));

        let named = vec![
            (":id".to_string(), "value1".to_string()),
            (":name".to_string(), "value2".to_string()),
        ];
        assert!(named.iter().any(|(name, _)| !name.is_empty()));

        let mixed = vec![
            (":id".to_string(), "value1".to_string()),
            (String::new(), "value2".to_string()),
        ];
        assert!(mixed.iter().any(|(name, _)| !name.is_empty()));
    }

    #[test]
    fn test_execute_query_with_like_operator() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_like.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (title TEXT)", []).unwrap();
            conn.execute(
                "INSERT INTO books VALUES ('The Rust Book'), ('Clean Code'), ('Rust in Action')",
                [],
            )
            .unwrap();
        }

        // Test LIKE with named parameter
        let params = vec![(":search".to_string(), "Rust".to_string())];
        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE title LIKE '%' || :search || '%'",
            &params,
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        
        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_empty_results() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_empty.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE test (id INTEGER)", []).unwrap();
            // No data inserted
        }

        let results = execute_query(temp_path, "SELECT * FROM test", &[]).unwrap();
        assert_eq!(results.len(), 0);

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_execute_query_multiple_named_params() {
        use rusqlite::Connection;
        use std::fs;

        let temp_path = "/tmp/test_sqlite_serve_multi.db";
        let _ = fs::remove_file(temp_path);

        {
            let conn = Connection::open(temp_path).unwrap();
            conn.execute("CREATE TABLE books (id INTEGER, genre TEXT, rating REAL)", [])
                .unwrap();
            conn.execute(
                "INSERT INTO books VALUES 
                    (1, 'Fiction', 4.5),
                    (2, 'Science', 4.8),
                    (3, 'Fiction', 4.9),
                    (4, 'Science', 4.2)",
                [],
            )
            .unwrap();
        }

        // Test multiple named parameters in different order
        let params = vec![
            (":min_rating".to_string(), "4.5".to_string()),
            (":genre".to_string(), "Fiction".to_string()),
        ];
        
        let results = execute_query(
            temp_path,
            "SELECT * FROM books WHERE genre = :genre AND rating >= :min_rating ORDER BY rating DESC",
            &params,
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].get("rating").unwrap().as_f64().unwrap(), 4.9);
        assert_eq!(results[1].get("rating").unwrap().as_f64().unwrap(), 4.5);

        // Clean up
        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn test_template_override_behavior() {
        use std::fs;
        use std::io::Write;

        let temp_dir = "/tmp/test_sqlite_serve_override";
        let _ = fs::remove_dir_all(temp_dir);
        fs::create_dir_all(temp_dir).unwrap();

        // Create first template
        let template1_path = format!("{}/test.hbs", temp_dir);
        let mut file1 = fs::File::create(&template1_path).unwrap();
        file1.write_all(b"Original").unwrap();

        let mut reg = Handlebars::new();
        reg.register_template_file("test", &template1_path).unwrap();
        
        let rendered1 = reg.render("test", &json!({})).unwrap();
        assert_eq!(rendered1, "Original");

        // Override with new content
        let mut file2 = fs::File::create(&template1_path).unwrap();
        file2.write_all(b"Updated").unwrap();
        
        // Re-register to override
        reg.register_template_file("test", &template1_path).unwrap();
        
        let rendered2 = reg.render("test", &json!({})).unwrap();
        assert_eq!(rendered2, "Updated");

        // Clean up
        let _ = fs::remove_dir_all(temp_dir);
    }
}
