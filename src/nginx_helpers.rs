//! NGINX-specific helper functions

use crate::logging;
use ngx::core::Buffer;
use ngx::ffi::ngx_chain_t;
use ngx::http::{HttpModuleLocationConf, NgxHttpCoreModule, Request};
use ngx::{core::Status, http};

/// Get document root and URI from request
pub fn get_doc_root_and_uri(request: &mut Request) -> Result<(String, String), String> {
    let core_loc_conf = NgxHttpCoreModule::location_conf(request)
        .ok_or_else(|| "failed to get core location conf".to_string())?;

    let doc_root = (*core_loc_conf)
        .root
        .to_str()
        .map_err(|e| format!("failed to decode root path: {}", e))?
        .to_string();

    let uri = request
        .path()
        .to_str()
        .map_err(|e| format!("failed to decode URI path: {}", e))?
        .to_string();

    Ok((doc_root, uri))
}

/// Create and send nginx response buffer
pub fn send_response(request: &mut Request, body: &str) -> Status {

    // Create output buffer
    let mut buf = match request.pool().create_buffer_from_str(body) {
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
}

/// Log and return error status (deprecated - use logging module directly)
#[allow(dead_code)]
pub fn log_error(request: &mut Request, context: &str, error: &str, status: http::HTTPStatus) -> Status {
    logging::log(request, logging::LogLevel::Error, context, error);
    status.into()
}

