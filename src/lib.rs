//! sqlite-serve - NGINX module for serving dynamic content from SQLite databases

mod adapters;
mod config;
mod domain;
mod nginx_helpers;
mod parsing;
mod query;
mod template;
mod types;
mod variable;

use adapters::{HandlebarsAdapter, NginxVariableResolver, SqliteQueryExecutor};
use config::{MainConfig, ModuleConfig};
use domain::RequestProcessor;
use nginx_helpers::{get_doc_root_and_uri, log_error, send_response};
use ngx::ffi::{
    NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF, NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE,
    NGX_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE, nginx_version, ngx_command_t, ngx_conf_t,
    ngx_http_module_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::http::{HttpModule, HttpModuleLocationConf, HttpModuleMainConf, NgxHttpCoreModule};
use ngx::{core::Status, http, http_request_handler, ngx_log_debug_http, ngx_modules, ngx_string};
use std::os::raw::{c_char, c_void};
use std::ptr::addr_of;

pub struct Module;

impl ngx::http::HttpModule for Module {
    fn module() -> &'static ngx_module_t {
        unsafe { &*addr_of!(ngx_http_howto_module) }
    }

    unsafe extern "C" fn postconfiguration(_cf: *mut ngx_conf_t) -> ngx_int_t {
        Status::NGX_OK.into()
    }
}

unsafe impl HttpModuleLocationConf for Module {
    type LocationConf = ModuleConfig;
}

unsafe impl HttpModuleMainConf for Module {
    type MainConf = MainConfig;
}

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

/// Directive handler for sqlite_db
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

/// Directive handler for sqlite_query
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

/// Directive handler for sqlite_template
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

/// Directive handler for sqlite_param
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

// HTTP request handler - minimal glue code orchestrating domain layer
http_request_handler!(howto_access_handler, |request: &mut http::Request| {
    let co = Module::location_conf(request).expect("module config is none");

    // Skip if not configured
    if co.db_path.is_empty() || co.query.is_empty() || co.template_path.is_empty() {
        return Status::NGX_OK;
    }

    // Parse configuration into validated domain types
    let validated_config = match parsing::parse_config(co) {
        Ok(config) => config,
        Err(e) => return log_error(request, "config parse error", &e, http::HTTPStatus::INTERNAL_SERVER_ERROR),
    };

    // Get document root and URI from nginx
    let (doc_root, uri) = match get_doc_root_and_uri(request) {
        Ok(paths) => paths,
        Err(e) => return log_error(request, "path resolution error", &e, http::HTTPStatus::INTERNAL_SERVER_ERROR),
    };

    // Resolve template path (pure function)
    let resolved_template = domain::resolve_template_path(&doc_root, &uri, &validated_config.template_path);

    // Resolve parameters from nginx variables
    let var_resolver = NginxVariableResolver::new(request);
    let resolved_params = match domain::resolve_parameters(&validated_config.parameters, &var_resolver) {
        Ok(params) => params,
        Err(e) => return log_error(request, "parameter resolution error", &e, http::HTTPStatus::BAD_REQUEST),
    };

    // Create processor with injected dependencies
    let mut reg = handlebars::Handlebars::new();
    let reg_ptr: *mut handlebars::Handlebars<'static> = unsafe { std::mem::transmute(&mut reg) };
    let hbs_adapter = unsafe { HandlebarsAdapter::new(reg_ptr) };
    let processor = RequestProcessor::new(SqliteQueryExecutor, hbs_adapter, hbs_adapter);

    // Get global template directory
    let main_conf = Module::main_conf(request).expect("main config is none");
    let global_dir = if !main_conf.global_templates_dir.is_empty() {
        Some(main_conf.global_templates_dir.as_str())
    } else {
        None
    };

    // Process request through functional core
    let body = match processor.process(&validated_config, &resolved_template, &resolved_params, global_dir) {
        Ok(html) => html,
        Err(e) => return log_error(request, "request processing error", &e, http::HTTPStatus::INTERNAL_SERVER_ERROR),
    };

    // Send response
    send_response(request, &body)
});
