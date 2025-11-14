use std::fmt::{Display, Formatter};
use std::fmt;
use ngx::core::Buffer;
use ngx::ffi::{
    NGX_CONF_TAKE1, NGX_HTTP_LOC_CONF, NGX_HTTP_MODULE, NGX_RS_HTTP_LOC_CONF_OFFSET, NGX_RS_MODULE_SIGNATURE, nginx_version, ngx_array_push, ngx_buf_t, ngx_chain_t, ngx_command_t, ngx_conf_t, ngx_http_core_module, ngx_http_discard_request_body, ngx_http_handler_pt, ngx_http_module_t, ngx_http_phases_NGX_HTTP_ACCESS_PHASE, ngx_http_request_t, ngx_int_t, ngx_module_t, ngx_str_t, ngx_uint_t
};
use ngx::http::{HTTPModule, MergeConfigError};
use ngx::{core, core::Status, http};
use ngx::{http_request_handler, ngx_log_debug_http, ngx_modules, ngx_null_command, ngx_string};
use std::os::raw::{c_char, c_void};
use std::ptr::addr_of;
use rusqlite::{Connection, Result};

struct Module;

// Implement our HTTPModule trait, we're creating a postconfiguration method to install our
// handler's Access phase function.
impl http::HTTPModule for Module {
    type MainConf = ();
    type SrvConf = ();
    type LocConf = ModuleConfig;

    unsafe extern "C" fn postconfiguration(cf: *mut ngx_conf_t) -> ngx_int_t {
        let htcf = http::ngx_http_conf_get_module_main_conf(cf, &*addr_of!(ngx_http_core_module));

        let h = ngx_array_push(
            &mut (*htcf).phases[ngx_http_phases_NGX_HTTP_ACCESS_PHASE as usize].handlers,
        ) as *mut ngx_http_handler_pt;
        if h.is_null() {
            return core::Status::NGX_ERROR.into();
        }

        // set an Access phase handler
        *h = Some(howto_access_handler);
        core::Status::NGX_OK.into()
    }
}

// Create a ModuleConfig to save our configuration state.
#[derive(Debug, Default)]
struct ModuleConfig {
    enabled: bool,
    method: String,
}

// Implement our Merge trait to merge configuration with higher layers.
impl http::Merge for ModuleConfig {
    fn merge(&mut self, prev: &ModuleConfig) -> Result<(), MergeConfigError> {
        if prev.enabled {
            self.enabled = true;
        }

        if self.method.is_empty() {
            self.method = String::from(if !prev.method.is_empty() {
                &prev.method
            } else {
                ""
            });
        }

        if self.enabled && self.method.is_empty() {
            return Err(MergeConfigError::NoValue);
        }
        Ok(())
    }
}

// Create our "C" module context with function entrypoints for NGINX event loop. This "binds" our
// HTTPModule implementation to functions callable from C.
#[unsafe(no_mangle)]
static ngx_http_howto_module_ctx: ngx_http_module_t = ngx_http_module_t {
    preconfiguration: Some(Module::preconfiguration),
    postconfiguration: Some(Module::postconfiguration),
    create_main_conf: Some(Module::create_main_conf),
    init_main_conf: Some(Module::init_main_conf),
    create_srv_conf: Some(Module::create_srv_conf),
    merge_srv_conf: Some(Module::merge_srv_conf),
    create_loc_conf: Some(Module::create_loc_conf),
    merge_loc_conf: Some(Module::merge_loc_conf),
};

// Create our module structure and export it with the `ngx_modules!` macro. For this simple
// handler, the ngx_module_t is predominately boilerplate save for setting the above context into
// this structure and setting our custom configuration command (defined below).
ngx_modules!(ngx_http_howto_module);

#[unsafe(no_mangle)]
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
// sure to terminate the array with the ngx_null_command! macro.
#[unsafe(no_mangle)]
static mut ngx_http_howto_commands: [ngx_command_t; 2] = [
    ngx_command_t {
        name: ngx_string!("howto"),
        type_: (NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1) as ngx_uint_t,
        set: Some(ngx_http_howto_commands_set_method),
        conf: NGX_RS_HTTP_LOC_CONF_OFFSET,
        offset: 0,
        post: std::ptr::null_mut(),
    },
    ngx_null_command!(),
];

#[unsafe(no_mangle)]
extern "C" fn ngx_http_howto_commands_set_method(
    cf: *mut ngx_conf_t,
    _cmd: *mut ngx_command_t,
    conf: *mut c_void,
) -> *mut c_char {
    unsafe {
        let conf = &mut *(conf as *mut ModuleConfig);
        let args = (*(*cf).args).elts as *mut ngx_str_t;
        conf.enabled = true;
        conf.method = (*args.add(1)).to_string();
    };

    std::ptr::null_mut()
}

#[derive(Debug)]
struct Person {
    id: u64,
    name: String,
    address: String
}

impl Display for Person {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Person ({}, {}, {})", self.id, self.name, self.address)
    }
}

fn get_person() -> Result<Vec<Person>> {
    let conn = Connection::open("db.sqlite3")?;
    let mut stmt = conn.prepare("SELECT id, name, address FROM person")?;

    let person_iter = stmt.query_map([], |row| {
        Ok(Person {
            id: row.get(0)?,
            name: row.get(1)?,
            address: row.get(2)?,
        })
    })?;

    person_iter.collect()
}

// Implement a request handler. Use the convenience macro, the http_request_handler! macro will
// convert the native NGINX request into a Rust Request instance as well as define an extern C
// function callable from NGINX.
//
// The function body is implemented as a Rust closure.
http_request_handler!(howto_access_handler, |request: &mut http::Request| {
    let m_persons = get_person();

    let enabled = {
        let co = unsafe { request.get_module_loc_conf::<ModuleConfig>(&*addr_of!(ngx_http_howto_module)) };
        let co = co.expect("module config is none");
        co.enabled
    };

    ngx_log_debug_http!(request, "howto module enabled called");

    match enabled {
        true => {
            match m_persons {
                Ok(persons) => {
                    request.discard_request_body();
                    request.set_status(http::HTTPStatus::OK);
                    // request.set_content_length_n(full_len);
                    let rc = request.send_header();
                    if rc == core::Status::NGX_ERROR || rc > core::Status::NGX_OK || request.header_only() {
                        return rc;
                    }

                    for person in persons {
                        ngx_log_debug_http!(request, "person: {}\n", person);

                        let s = fmt::format(format_args!("{}\n", person));
                        let mut buf = match request.pool().create_buffer_from_str(&s) {
                            Some(buf) => buf,
                            None => return http::HTTPStatus::INTERNAL_SERVER_ERROR.into(),
                        };

                        buf.set_last_buf(false);
                        buf.set_last_in_chain(false);

                        let mut out = ngx_chain_t {
                            buf: buf.as_ngx_buf_mut(),
                            next: std::ptr::null_mut()
                        };

                        request.output_filter(&mut out);
                    }

                    let mut buf = match request.pool().create_buffer_from_static_str("\n") {
                        Some(buf) => buf,
                        None => return http::HTTPStatus::INTERNAL_SERVER_ERROR.into(),
                    };

                    buf.set_last_buf(request.is_main());
                    buf.set_last_in_chain(true);

                    let mut out = ngx_chain_t {
                        buf: buf.as_ngx_buf_mut(),
                        next: std::ptr::null_mut()
                    };

                    request.output_filter(&mut out);
                },
                Err(e) => {
                    //todo!();
                    ngx_log_debug_http!(request, "failed to find persons: {}", e);

                    return http::HTTPStatus::INTERNAL_SERVER_ERROR.into()
                }
            }


            // let method = request.method();

            // if method.as_str() == co.method {
            //     return core::Status::NGX_OK;
            // }
            Status::NGX_DONE
        }
        false => core::Status::NGX_OK,
    }
});
