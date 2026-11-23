use std::alloc::{alloc as std_alloc, dealloc as std_dealloc, Layout};
use std::slice;

use anyhow::{anyhow, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::github::{
    build_metrics_from_graphql_responses, build_overview_variables, build_repositories_variables,
    extract_overview_identity, extract_repo_page_cursor, GITHUB_GRAPHQL_ENDPOINT, OVERVIEW_QUERY,
    REPOSITORIES_QUERY, USER_AGENT,
};
use crate::svg::{render_metrics_svg, Theme};

/// Allocates `len` bytes with 1-byte alignment in linear WASM memory.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = match Layout::from_size_align(len, 1) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };
    // SAFETY: `len > 0` and alignment is a valid power of two.
    unsafe { std_alloc(layout) }
}

/// Frees a buffer previously allocated by `alloc` (or returned by a `wasm_*` entrypoint).
///
/// # Safety
/// `ptr` must have been allocated by `alloc(len)` with the exact same `len`.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    if len == 0 || ptr.is_null() {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(len, 1) {
        // SAFETY: Caller guarantees `ptr` came from `alloc(len)`.
        unsafe { std_dealloc(ptr, layout) };
    }
}

fn read_input_str(ptr: *const u8, len: usize) -> Result<String> {
    if len == 0 {
        return Ok(String::new());
    }
    if ptr.is_null() {
        return Err(anyhow!("null pointer passed to wasm entrypoint"));
    }
    // SAFETY: Host passes a valid `(ptr, len)` buffer allocated via `alloc`.
    let bytes = unsafe { slice::from_raw_parts(ptr, len) };
    let text = std::str::from_utf8(bytes).map_err(|e| anyhow!("invalid UTF-8 input: {e}"))?;
    Ok(text.to_string())
}

fn pack_output_bytes(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    let out_ptr = alloc(len);
    if len > 0 && !out_ptr.is_null() {
        // SAFETY: `out_ptr` has `len` freshly allocated writable bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr, len);
        }
    }
    ((out_ptr as u32 as u64) << 32) | (len as u32 as u64)
}

#[derive(Serialize)]
struct WasmError {
    ok: bool,
    error: String,
}

fn encode_result<T: Serialize>(result: Result<T>) -> u64 {
    let json_str = match result {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|e| {
            serde_json::to_string(&WasmError {
                ok: false,
                error: format!("failed to serialize wasm response: {e}"),
            })
            .unwrap_or_else(|_| r#"{"ok":false,"error":"serialization failure"}"#.to_string())
        }),
        Err(err) => serde_json::to_string(&WasmError {
            ok: false,
            error: format!("{err:#}"),
        })
        .unwrap_or_else(|_| r#"{"ok":false,"error":"unknown error"}"#.to_string()),
    };
    pack_output_bytes(json_str.as_bytes())
}

#[derive(Deserialize)]
struct PrepareOverviewInput {
    username: String,
    now_ms: i64,
    tz_offset_hours: i32,
}

#[derive(Serialize)]
struct PrepareOverviewOutput {
    ok: bool,
    endpoint: &'static str,
    user_agent: &'static str,
    query: &'static str,
    variables: serde_json::Value,
}

#[no_mangle]
pub extern "C" fn wasm_prepare_overview(ptr: *const u8, len: usize) -> u64 {
    let res = (|| -> Result<PrepareOverviewOutput> {
        let raw = read_input_str(ptr, len)?;
        let input: PrepareOverviewInput = serde_json::from_str(&raw)?;
        let now = DateTime::from_timestamp_millis(input.now_ms)
            .ok_or_else(|| anyhow!("invalid now_ms timestamp: {}", input.now_ms))?;
        Ok(PrepareOverviewOutput {
            ok: true,
            endpoint: GITHUB_GRAPHQL_ENDPOINT,
            user_agent: USER_AGENT,
            query: OVERVIEW_QUERY,
            variables: build_overview_variables(&input.username, now, input.tz_offset_hours),
        })
    })();
    encode_result(res)
}

#[derive(Deserialize)]
struct ParseIdentityInput {
    username: String,
    overview_json: String,
}

#[derive(Serialize)]
struct ParseIdentityOutput {
    ok: bool,
    login: String,
    author_id: String,
}

#[no_mangle]
pub extern "C" fn wasm_parse_overview_identity(ptr: *const u8, len: usize) -> u64 {
    let res = (|| -> Result<ParseIdentityOutput> {
        let raw = read_input_str(ptr, len)?;
        let input: ParseIdentityInput = serde_json::from_str(&raw)?;
        let identity = extract_overview_identity(&input.overview_json, &input.username)?;
        Ok(ParseIdentityOutput {
            ok: true,
            login: identity.login,
            author_id: identity.id,
        })
    })();
    encode_result(res)
}

#[derive(Deserialize)]
struct PrepareReposPageInput {
    login: String,
    author_id: String,
    cursor: Option<String>,
    now_ms: i64,
}

#[derive(Serialize)]
struct PrepareReposPageOutput {
    ok: bool,
    query: &'static str,
    variables: serde_json::Value,
}

#[no_mangle]
pub extern "C" fn wasm_prepare_repos_page(ptr: *const u8, len: usize) -> u64 {
    let res = (|| -> Result<PrepareReposPageOutput> {
        let raw = read_input_str(ptr, len)?;
        let input: PrepareReposPageInput = serde_json::from_str(&raw)?;
        let now = DateTime::from_timestamp_millis(input.now_ms)
            .ok_or_else(|| anyhow!("invalid now_ms timestamp: {}", input.now_ms))?;
        Ok(PrepareReposPageOutput {
            ok: true,
            query: REPOSITORIES_QUERY,
            variables: build_repositories_variables(
                &input.login,
                &input.author_id,
                input.cursor.as_deref(),
                now,
            ),
        })
    })();
    encode_result(res)
}

#[derive(Deserialize)]
struct ParseCursorInput {
    page_json: String,
}

#[derive(Serialize)]
struct ParseCursorOutput {
    ok: bool,
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[no_mangle]
pub extern "C" fn wasm_parse_repos_cursor(ptr: *const u8, len: usize) -> u64 {
    let res = (|| -> Result<ParseCursorOutput> {
        let raw = read_input_str(ptr, len)?;
        let input: ParseCursorInput = serde_json::from_str(&raw)?;
        let page = extract_repo_page_cursor(&input.page_json)?;
        Ok(ParseCursorOutput {
            ok: true,
            has_next_page: page.has_next_page,
            end_cursor: page.end_cursor,
        })
    })();
    encode_result(res)
}

#[derive(Deserialize)]
struct RenderSvgsInput {
    username: String,
    now_ms: i64,
    tz_offset_hours: i32,
    overview_json: String,
    repo_pages_json: Vec<String>,
}

#[derive(Serialize)]
struct RenderSvgsOutput {
    ok: bool,
    dark_svg: String,
    light_svg: String,
}

#[no_mangle]
pub extern "C" fn wasm_render_svgs(ptr: *const u8, len: usize) -> u64 {
    let res = (|| -> Result<RenderSvgsOutput> {
        let raw = read_input_str(ptr, len)?;
        let input: RenderSvgsInput = serde_json::from_str(&raw)?;
        let now = DateTime::from_timestamp_millis(input.now_ms)
            .ok_or_else(|| anyhow!("invalid now_ms timestamp: {}", input.now_ms))?;
        let metrics = build_metrics_from_graphql_responses(
            &input.overview_json,
            &input.repo_pages_json,
            &input.username,
            now,
            input.tz_offset_hours,
        )?;
        let dark_svg = render_metrics_svg(&metrics, Theme::Dark);
        let light_svg = render_metrics_svg(&metrics, Theme::Light);
        Ok(RenderSvgsOutput {
            ok: true,
            dark_svg,
            light_svg,
        })
    })();
    encode_result(res)
}
