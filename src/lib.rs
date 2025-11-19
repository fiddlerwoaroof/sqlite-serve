//! sqlite-serve - NGINX module for serving dynamic content from SQLite databases
//!
//! # Safety and Unsafe Code
//!
//! This module contains `unsafe` code that is **required** for FFI (Foreign Function Interface)
//! with NGINX's C API. All unsafe code in this codebase falls into these necessary categories:
//!
//! ## Required Unsafe Code (Cannot be removed):
//!
//! 1. **NGINX Module Registration** (`lib.rs`)
//!    - `#[unsafe(no_mangle)]` - Required for C linkage
//!    - `unsafe impl` for HttpModuleLocationConf/HttpModuleMainConf - Required by ngx crate
//!    - `unsafe extern "C"` - Required for C FFI callbacks
//!    - Static mutable globals - Required by NGINX module ABI
//!
//! 2. **NGINX Variable Resolution** (`variable.rs`)
//!    - Raw pointer dereferencing to access NGINX request structures
//!    - Calls to NGINX C API functions (ngx_hash_key, ngx_http_get_variable)
//!    - Required for reading nginx variables like $arg_id
//!
//! 3. **NGINX Logging** (`logging.rs`)
//!    - Raw pointer dereferencing to access NGINX log structures
//!    - Required for structured logging through nginx's logging system
//!
//! 4. **HTTP Header Parsing** (`content_type.rs`)
//!    - Raw pointer dereferencing to access NGINX header structures
//!    - Required for content negotiation based on Accept headers
//!
//! All unsafe code has been audited and is necessary for interfacing with NGINX's C API.
//! The rest of the codebase uses safe Rust with type-driven correctness guarantees.

mod adapters;
mod config;
mod content_type;
mod domain;
mod handler_types;
mod logging;
mod nginx_helpers;
mod parsing;
mod query;
mod template;
mod types;
mod variable;

use config::{MainConfig, ModuleConfig};
use handler_types::{ValidConfigToken, process_request};
use nginx_helpers::get_doc_root_and_uri;
use ngx::ffi::{
    NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF, NGX_HTTP_LOC_CONF_OFFSET,
    NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE, NGX_RS_MODULE_SIGNATURE, nginx_version, ngx_command_t,
    ngx_conf_t, ngx_http_module_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::http::{HttpModule, HttpModuleLocationConf, HttpModuleMainConf, NgxHttpCoreModule};
use ngx::{core::Status, http, http_request_handler, ngx_modules, ngx_string};
use std::os::raw::{c_char, c_void};
use std::ptr::addr_of;

pub struct Module;

impl ngx::http::HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        // SAFETY: Required for module registration. addr_of! creates a raw pointer to the
        // static ngx_http_howto_module which is then dereferenced to get a static reference.
        // This is safe because the module is a static with 'static lifetime.
        unsafe { &*addr_of!(ngx_http_howto_module) }
    }

    // SAFETY: Required C FFI callback signature for NGINX module postconfiguration hook.
    // Must be unsafe extern "C" to match NGINX's expected function signature.
    unsafe extern "C" fn postconfiguration(_cf: *mut ngx_conf_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
}

// SAFETY: Required by ngx crate's trait system to associate config types with the module.
// These impls guarantee that ModuleConfig/MainConfig are valid NGINX configuration structures.
unsafe impl HttpModuleLocationConf for Module {
    type LocationConf = ModuleConfig;
}

// SAFETY: Required by ngx crate's trait system to associate config types with the module.
unsafe impl HttpModuleMainConf for Module {
    type MainConf = MainConfig;
}

// SAFETY: no_mangle required for C linkage. NGINX needs to find this symbol by name.
// This static defines the module's callback functions for NGINX lifecycle hooks.
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

ngx_modules!(ngx_http_howto_module);

// SAFETY: no_mangle + static mut required for NGINX module ABI.
// NGINX expects to find this symbol by name and will mutate it during initialization.
// This is the main module descriptor that NGINX uses to load and configure the module.
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
    // SAFETY: Creating a mutable pointer to the commands array for NGINX's C API.
    // NGINX expects a mutable pointer but won't actually mutate this data.
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

// SAFETY: no_mangle + static mut required for NGINX configuration directive system.
// This array defines the configuration directives (sqlite_db, sqlite_query, etc.).
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
static mut ngx_http_howto_commands: [ngx_command_t; 6] = [
    ngx_command_t {
        name: ngx_string!("sqlite_global_templates"),
        type_: (NGX_HTTP_MAIN_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_global_templates),
        conf: 0,
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

/// Directive handler for sqlite_global_templates
// SAFETY: no_mangle + extern "C" required for NGINX to call this directive handler.
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_global_templates(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: NGINX guarantees these pointers are valid during config parsing.
    // - conf is a valid pointer to MainConfig allocated by NGINX
    // - cf.args points to the directive arguments array
    unsafe {
        let conf = &mut *(conf as *mut MainConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.global_templates_dir = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

/// Directive handler for sqlite_db
// SAFETY: no_mangle + extern "C" required for NGINX to call this directive handler.
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_db_path(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: NGINX guarantees these pointers are valid during config parsing.
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.db_path = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

/// Directive handler for sqlite_query
// SAFETY: no_mangle + extern "C" required for NGINX to call this directive handler.
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_query(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: NGINX guarantees these pointers are valid during config parsing.
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.query = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

/// Directive handler for sqlite_template
// SAFETY: no_mangle + extern "C" required for NGINX to call this directive handler.
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_template_path(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: NGINX guarantees these pointers are valid during config parsing.
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.template_path = (*args.add(1)).to_string();

        // Set the content handler for this location
        let clcf =
            NgxHttpCoreModule::location_conf_mut(&*cf).expect("failed to get core location conf");
        clcf.handler = Some(howto_access_handler);
    };

    std::ptr::null_mut()
}

/// Directive handler for sqlite_param
// SAFETY: no_mangle + extern "C" required for NGINX to call this directive handler.
#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_add_param(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    // SAFETY: NGINX guarantees these pointers are valid during config parsing.
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

// HTTP request handler - correctness guaranteed by types (Ghost of Departed Proofs)
http_request_handler!(howto_access_handler, |request: &mut http::Request| {
    let (doc_root, uri) = match get_doc_root_and_uri(request) {
        Ok(res) => res,
        Err(e) => {
            logging::log(
                request,
                logging::LogLevel::Error,
                "nginx",
                &format!("Path resolution failed: {}", e),
            );
            return ngx::http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    let config = Module::location_conf(request).expect("module config is none");

    // Type-safe gate: only proceed if we have proof of valid config
    match ValidConfigToken::new(config, doc_root, uri) {
        Some(valid_config) => process_request(request, valid_config.get()),
        None => Status::NGX_OK, // Not configured - skip silently
    }
});
