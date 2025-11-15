use handlebars::Handlebars;
use ngx::core::Buffer;
use ngx::ffi::{
    ngx_hash_key, ngx_http_get_variable, NGX_CONF_TAKE1, NGX_HTTP_LOC_CONF,
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
    query_params: Vec<String>, // Variable names to use as query parameters
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
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
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
        let param = (*args.add(1)).to_string();
        conf.query_params.push(param);
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
    params: &[&str],
) -> Result<Vec<std::collections::HashMap<String, serde_json::Value>>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(query)?;
    
    let column_count = stmt.column_count();
    let column_names: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("").to_string())
        .collect();

    // Convert params to rusqlite parameters
    let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params
        .iter()
        .map(|p| p as &dyn rusqlite::ToSql)
        .collect();

    let rows = stmt.query_map(rusqlite_params.as_slice(), |row| {
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
                    let hex_string = v.iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<String>();
                    serde_json::Value::String(hex_string)
                }
            };
            map.insert(col_name.clone(), value);
        }
        Ok(map)
    })?;

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
    let mut param_values: Vec<String> = Vec::new();
    for var_name in &co.query_params {
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
        param_values.push(value);
    }

    ngx_log_debug_http!(
        request,
        "executing query with {} parameters: {:?}",
        param_values.len(),
        param_values
    );

    // Execute the configured SQL query with parameters
    let param_refs: Vec<&str> = param_values.iter().map(|s| s.as_str()).collect();
    let results = match execute_query(&co.db_path, &co.query, &param_refs) {
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
