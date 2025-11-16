//! sqlite-serve - NGINX module for serving dynamic content from SQLite databases

mod adapters;
mod config;
mod domain;
mod query;
mod template;
mod types;
mod variable;

use adapters::{HandlebarsAdapter, NginxVariableResolver, SqliteQueryExecutor};
use config::{MainConfig, ModuleConfig};
use domain::RequestProcessor;
use ngx::ffi::{
    NGX_CONF_TAKE1, NGX_CONF_TAKE2, NGX_HTTP_LOC_CONF, NGX_HTTP_MAIN_CONF, NGX_HTTP_MODULE,
    NGX_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE, nginx_version, ngx_command_t, ngx_conf_t,
    ngx_http_module_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t,
};
use ngx::core::Buffer;
use ngx::ffi::ngx_chain_t;
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

// HTTP request handler using functional core with dependency injection
http_request_handler!(howto_access_handler, |request: &mut http::Request| {
    let co = Module::location_conf(request).expect("module config is none");

    // Check if all required config values are set
    if co.db_path.is_empty() || co.query.is_empty() || co.template_path.is_empty() {
        return Status::NGX_OK;
    }

    ngx_log_debug_http!(request, "sqlite module handler called");

    // Parse and validate configuration into domain types
    let db_path = match types::DatabasePath::parse(&co.db_path) {
        Ok(p) => p,
        Err(e) => {
            ngx_log_debug_http!(request, "invalid db path: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    let query = match types::SqlQuery::parse(&co.query) {
        Ok(q) => q,
        Err(e) => {
            ngx_log_debug_http!(request, "invalid query: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    let template_path = match types::TemplatePath::parse(&co.template_path) {
        Ok(t) => t,
        Err(e) => {
            ngx_log_debug_http!(request, "invalid template path: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Parse parameter bindings
    let mut bindings = Vec::new();
    for (param_name, var_name) in &co.query_params {
        let binding = if var_name.starts_with('$') {
            let variable = match types::NginxVariable::parse(var_name) {
                Ok(v) => v,
                Err(e) => {
                    ngx_log_debug_http!(request, "invalid variable: {}", e);
                    return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
                }
            };

            if param_name.is_empty() {
                types::ParameterBinding::Positional { variable }
            } else {
                let name = match types::ParamName::parse(param_name) {
                    Ok(n) => n,
                    Err(e) => {
                        ngx_log_debug_http!(request, "invalid param name: {}", e);
                        return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
                    }
                };
                types::ParameterBinding::Named { name, variable }
            }
        } else {
            // Literal value
            if param_name.is_empty() {
                types::ParameterBinding::PositionalLiteral {
                    value: var_name.clone(),
                }
            } else {
                let name = match types::ParamName::parse(param_name) {
                    Ok(n) => n,
                    Err(e) => {
                        ngx_log_debug_http!(request, "invalid param name: {}", e);
                        return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
                    }
                };
                types::ParameterBinding::NamedLiteral {
                    name,
                    value: var_name.clone(),
                }
            }
        };
        bindings.push(binding);
    }

    let validated_config = domain::ValidatedConfig {
        db_path,
        query,
        template_path,
        parameters: bindings,
    };

    // Resolve template path relative to document root and URI
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

    let resolved_template = domain::resolve_template_path(doc_root, uri, &validated_config.template_path);

    ngx_log_debug_http!(request, "resolved template path: {}", resolved_template.full_path());

    // Resolve parameters using nginx variable resolver
    let var_resolver = NginxVariableResolver::new(request);
    let resolved_params = match domain::resolve_parameters(&validated_config.parameters, &var_resolver) {
        Ok(params) => params,
        Err(e) => {
            ngx_log_debug_http!(request, "failed to resolve parameters: {}", e);
            return http::HTTPStatus::BAD_REQUEST.into();
        }
    };

    ngx_log_debug_http!(request, "resolved {} parameters", resolved_params.len());

    // Create domain processor with adapters (dependency injection)
    let mut reg = handlebars::Handlebars::new();
    let reg_ptr: *mut handlebars::Handlebars<'static> = unsafe { std::mem::transmute(&mut reg) };
    let hbs_adapter = unsafe { HandlebarsAdapter::new(reg_ptr) };
    let processor = RequestProcessor::new(
        SqliteQueryExecutor,
        hbs_adapter,
        hbs_adapter,
    );

    // Get global template directory from main config
    let main_conf = Module::main_conf(request).expect("main config is none");
    let global_dir = if !main_conf.global_templates_dir.is_empty() {
        Some(main_conf.global_templates_dir.as_str())
    } else {
        None
    };

    // Process request through functional core
    let body = match processor.process(&validated_config, &resolved_template, &resolved_params, global_dir) {
        Ok(html) => html,
        Err(e) => {
            ngx_log_debug_http!(request, "request processing failed: {}", e);
            return http::HTTPStatus::INTERNAL_SERVER_ERROR.into();
        }
    };

    // Create output buffer (imperative shell)
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
    if rc == Status::NGX_ERROR || rc > Status::NGX_OK || request.header_only() {
        return rc;
    }

    request.output_filter(&mut out);
    Status::NGX_DONE
});
