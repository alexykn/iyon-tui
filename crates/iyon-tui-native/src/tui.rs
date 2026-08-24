use napi::Env;
use napi::bindgen_prelude::{Array, JsObjectValue, JsValue, Object, Result, Unknown, ValueType};
use napi_derive::napi;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use iyon_tui::projection::ProjectionBuilder;
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{FormatId, LanguageId, SemanticTag, TextOrigin};
use iyon_tui::text::{TextRun, TextVisitor};
use iyon_tui::{
    BorderEdges, BorderGlyphs, BorderSpec, GridCellSpec, GridTrack, History, HorizontalAlign,
    HostCellStyle, HostHistory, HostScrollPane, HostTextInput, HostTextStream, HostViewSlot,
    IntoView, Key, KeyStroke, MarkdownOptions, MarkdownProjector, Modifiers, Output, Projector,
    Renderer, StyleRef, StyleSpec, TextContent, TextInput, TextPart, TextRole, TextSelector,
    TextSpan, TuiHost, VerticalAlign, View, WrapMode,
};
use serde_json::Map;
use serde_json::Value;

mod generated_view_abi_conformance {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/view_abi_conformance.rs"
    ));
}

mod view_abi;

type ViewBridgeCache = view_abi::NativeViewRuntime;
type ViewRuntimeHandle = view_abi::ViewRuntimeHandle;

macro_rules! tui_perf_inc {
    ($counter:ident) => {
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::$counter);
    };
}

macro_rules! tui_perf_add {
    ($counter:ident, $amount:expr) => {
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::add(iyon_tui::perf::Counter::$counter, $amount as u64);
    };
}

/// Link/surface probe only: construct one owned public TUI value and discard
/// it. The native bridge must not duplicate or serialize the TUI renderer.
#[napi(js_name = "tuiSmoke")]
pub fn tui_smoke() -> Result<String> {
    let _view = View::text("iyon-tui/t1").into_view();
    Ok("iyon-tui/t1".to_owned())
}

#[unsafe(no_mangle)]
pub extern "C" fn iyon_abi_probe_noop(value: u32) -> u32 {
    value.wrapping_add(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn iyon_abi_probe_u32_8(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
) -> u32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
        .wrapping_add(a4.wrapping_mul(13))
        .wrapping_add(a5.wrapping_mul(17))
        .wrapping_add(a6.wrapping_mul(19))
        .wrapping_add(a7.wrapping_mul(23))
}

#[unsafe(no_mangle)]
pub extern "C" fn iyon_abi_probe_i32_4(a0: i32, a1: i32, a2: i32, a3: i32) -> i32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_probe_buffer(bytes: *const u8, byte_length: usize) -> u32 {
    if bytes.is_null() {
        return u32::MAX;
    }
    let first = unsafe { *bytes } as u32;
    (byte_length as u32).wrapping_mul(257).wrapping_add(first)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_probe_cstring(value: *const std::ffi::c_char) -> u32 {
    if value.is_null() {
        return 0;
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(value) }.to_bytes();
    bytes.iter().fold(2166136261u32, |hash, byte| {
        hash.wrapping_mul(16777619).wrapping_add(u32::from(*byte))
    })
}

#[napi(js_name = "tuiPerfAbiProbe")]
pub fn tui_perf_abi_probe() -> Value {
    serde_json::json!({
        "noop_ptr": iyon_abi_probe_noop as *const () as usize as u64,
        "u32_8_ptr": iyon_abi_probe_u32_8 as *const () as usize as u64,
        "i32_4_ptr": iyon_abi_probe_i32_4 as *const () as usize as u64,
        "buffer_ptr": iyon_abi_probe_buffer as *const () as usize as u64,
        "cstring_ptr": iyon_abi_probe_cstring as *const () as usize as u64,
    })
}

#[napi(js_name = "tuiPerfAbiConformanceProbe")]
pub fn tui_perf_abi_conformance_probe() -> Value {
    serde_json::json!({
        "u8_8": generated_view_abi_conformance::iyon_abi_conformance_u8_8_v1 as *const () as usize as u64,
        "u16_8": generated_view_abi_conformance::iyon_abi_conformance_u16_8_v1 as *const () as usize as u64,
        "u32_8": generated_view_abi_conformance::iyon_abi_conformance_u32_8_v1 as *const () as usize as u64,
        "u32_16": generated_view_abi_conformance::iyon_abi_conformance_u32_16_v1 as *const () as usize as u64,
        "i32_4": generated_view_abi_conformance::iyon_abi_conformance_i32_4_v1 as *const () as usize as u64,
        "f32_4": generated_view_abi_conformance::iyon_abi_conformance_f32_4_v1 as *const () as usize as u64,
        "f64_4": generated_view_abi_conformance::iyon_abi_conformance_f64_4_v1 as *const () as usize as u64,
        "pointer": generated_view_abi_conformance::iyon_abi_conformance_pointer_v1 as *const () as usize as u64,
        "buffer": generated_view_abi_conformance::iyon_abi_conformance_buffer_v1 as *const () as usize as u64,
        "cstring": generated_view_abi_conformance::iyon_abi_conformance_cstring_v1 as *const () as usize as u64,
    })
}

#[cfg(feature = "perf-counters")]
#[napi(js_name = "tuiPerfReset")]
pub fn tui_perf_reset() {
    iyon_tui::perf::reset();
}

#[cfg(feature = "perf-counters")]
#[napi(js_name = "tuiPerfSnapshot")]
pub fn tui_perf_snapshot() -> Value {
    let mut counters = Map::new();
    for (name, value) in iyon_tui::perf::snapshot().iter() {
        counters.insert(name.to_owned(), Value::from(value));
    }
    Value::Object(counters)
}

#[napi(js_name = "tuiViewBridgeEnvironmentCount")]
pub fn tui_view_bridge_environment_count() -> i64 {
    view_abi::runtime_environment_count()
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfResetViewBridgeCache")]
pub fn tui_perf_reset_view_bridge_cache(env: Env) -> Result<()> {
    let cache = view_bridge_cache_for_env(&env)?;
    with_view_runtime(&cache, |runtime| runtime.nodes.clear())?;
    Ok(())
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfViewBridgeCacheSize")]
pub fn tui_perf_view_bridge_cache_size(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    let size = with_view_runtime(&cache, |runtime| runtime.nodes.len())?;
    i64::try_from(size).map_err(|_| crate::NativeError::internal("view bridge cache size overflow"))
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV3ResetViewBridgeCache")]
pub fn tui_perf_v3_reset_view_bridge_cache(env: Env) -> Result<()> {
    let cache = view_bridge_cache_for_env(&env)?;
    with_view_runtime(&cache, |runtime| runtime.packed_v3.reset_slots())?;
    Ok(())
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV3ViewBridgeCacheSize")]
pub fn tui_perf_v3_view_bridge_cache_size(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    let size = with_view_runtime(&cache, |runtime| runtime.packed_v3.slot_count())?;
    i64::try_from(size).map_err(|_| crate::NativeError::internal("packed V3 slot count overflow"))
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV3PackedSlotPages")]
pub fn tui_perf_v3_packed_slot_pages(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    let pages = with_view_runtime(&cache, |runtime| runtime.packed_v3.page_count())?;
    i64::try_from(pages).map_err(|_| crate::NativeError::internal("packed V3 page count overflow"))
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV4ResetViewBridgeCache")]
pub fn tui_perf_v4_reset_view_bridge_cache(env: Env) -> Result<()> {
    let cache = view_bridge_cache_for_env(&env)?;
    with_view_runtime(&cache, |runtime| {
        runtime.nodes.clear();
        runtime.packed_v4.reset_slots();
    })?;
    Ok(())
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV4ViewBridgeCacheSize")]
pub fn tui_perf_v4_view_bridge_cache_size(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    let size = with_view_runtime(&cache, |runtime| runtime.packed_v4.slot_count())?;
    i64::try_from(size).map_err(|_| crate::NativeError::internal("packed V4 slot count overflow"))
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV4ViewBridgeGeneration")]
pub fn tui_perf_v4_view_bridge_generation(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    Ok(i64::from(with_view_runtime(&cache, |runtime| {
        runtime.packed_v4.generation
    })?))
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi(js_name = "tuiPerfV3ViewBridgeGeneration")]
pub fn tui_perf_v3_view_bridge_generation(env: Env) -> Result<i64> {
    let cache = view_bridge_cache_for_env(&env)?;
    Ok(i64::from(with_view_runtime(&cache, |runtime| {
        runtime.packed_v3.generation
    })?))
}

mod tui_bridge_schema {
    include!(concat!(env!("OUT_DIR"), "/tui_bridge_schema.rs"));
}

use tui_bridge_schema::*;

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
mod fast_shared;
#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
mod packed;
#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
mod packed_v3;
#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
mod packed_v4;

fn view_bridge_cache(value: &Object<'_>) -> Result<ViewRuntimeHandle> {
    view_abi::runtime_handle_for_env(&Env::from_raw(value.value().env))
}

fn view_bridge_cache_for_env(env: &Env) -> Result<ViewRuntimeHandle> {
    view_abi::runtime_handle_for_env(env)
}

fn runtime_from_handle(runtime: &ViewRuntimeHandle) -> Result<&'static mut ViewBridgeCache> {
    view_abi::runtime_from_handle(runtime)
}

fn with_view_runtime<T>(
    runtime: &ViewRuntimeHandle,
    operation: impl FnOnce(&mut ViewBridgeCache) -> T,
) -> Result<T> {
    Ok(operation(runtime_from_handle(runtime)?))
}

#[napi]
pub struct NativeTuiOutput {
    output: Output<String>,
}

fn ensure_alive(alive: &AtomicBool) -> Result<()> {
    if alive.load(Ordering::Acquire) {
        return Ok(());
    }
    Err(crate::NativeError::coded(
        napi::Status::Closing,
        "ION_DISPOSED_HANDLE",
        "native TUI handle has been disposed",
    ))
}

fn resolve_native_view(runtime: usize, view_ref: i64) -> Result<View> {
    let view_ref = u32::try_from(view_ref)
        .map_err(|_| crate::NativeError::invalid_input("native View reference must fit in u32"))?;
    if view_ref == 0 {
        return Err(crate::NativeError::invalid_input(
            "native View reference must be positive",
        ));
    }
    view_abi::view_for_ref(runtime as *mut view_abi::NativeViewRuntime, view_ref)
        .map_err(|_| crate::NativeError::invalid_input("native View reference is unavailable"))
}

#[napi]
pub struct NativeHistory {
    state: Mutex<History>,
    host: Option<HostHistory>,
    alive: AtomicBool,
    view_runtime: usize,
}

#[napi]
impl NativeHistory {
    #[napi(constructor)]
    pub fn new(env: Env) -> Result<Self> {
        Ok(Self {
            state: Mutex::new(History::new()),
            host: None,
            alive: AtomicBool::new(true),
            view_runtime: view_abi::runtime_ptr_for_env(&env)? as usize,
        })
    }

    #[napi]
    pub fn dispose(&self) {
        self.alive.store(false, Ordering::Release);
    }

    #[napi(js_name = "isDetached")]
    pub fn is_detached(&self) -> bool {
        self.host.is_none()
    }

    fn take_for_host(&mut self) -> Result<History> {
        if self.host.is_some() {
            return Err(crate::NativeError::invalid_input(
                "history is already attached to a native host",
            ));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?;
        Ok(std::mem::replace(&mut *state, History::new()))
    }

    #[napi]
    pub fn layout(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            let layout = host
                .layout()
                .map_err(|error| crate::NativeError::internal(error.to_string()))?;
            return Ok(
                serde_json::json!({"padding": layout.padding().bottom(), "gap": layout.gap()}),
            );
        }
        let _layout = self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?
            .layout();
        Ok(serde_json::json!({"padding": _layout.padding().bottom(), "gap": _layout.gap()}))
    }

    #[napi(js_name = "setLayout")]
    pub fn set_layout(&self, value: Value) -> Result<()> {
        ensure_alive(&self.alive)?;
        let object = value
            .as_object()
            .ok_or_else(|| crate::NativeError::invalid_input("history layout must be an object"))?;
        let padding = u16_value(object, "padding")?;
        let gap = u16_value(object, "gap")?;
        let layout =
            iyon_tui::HistoryLayout::from_parts(iyon_tui::Insets::new(0, 0, padding, 0), gap);
        if let Some(host) = &self.host {
            return host
                .set_layout(layout)
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        self.state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?
            .set_layout(layout);
        Ok(())
    }

    #[napi]
    pub fn push(&self, view: Object) -> Result<i64> {
        ensure_alive(&self.alive)?;
        self.push_view(decode_view(&view)?)
    }

    #[napi(js_name = "pushRef")]
    pub fn push_ref(&self, view_ref: i64) -> Result<i64> {
        ensure_alive(&self.alive)?;
        self.push_view(resolve_native_view(self.view_runtime, view_ref)?)
    }

    fn push_view(&self, view: View) -> Result<i64> {
        if let Some(host) = &self.host {
            return host
                .push(view.clone())
                .map(|unit| unit.value() as i64)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()));
        }
        self.state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?
            .push(view)
            .map(|unit| unit.value() as i64)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn freeze(&self, unit: i64, view: Object) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.freeze_view(unit, decode_view(&view)?)
    }

    #[napi(js_name = "freezeRef")]
    pub fn freeze_ref(&self, unit: i64, view_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.freeze_view(unit, resolve_native_view(self.view_runtime, view_ref)?)
    }

    fn freeze_view(&self, unit: i64, view: View) -> Result<()> {
        let unit = u64::try_from(unit)
            .map_err(|_| crate::NativeError::invalid_input("history unit id must be positive"))?;
        if let Some(host) = &self.host {
            return host
                .freeze(unit, view)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()));
        }
        Err(crate::NativeError::invalid_input(
            "detached history cannot freeze a unit",
        ))
    }

    #[napi(js_name = "discardLive")]
    pub fn discard_live(&self, unit: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let unit = u64::try_from(unit)
            .map_err(|_| crate::NativeError::invalid_input("history unit id must be positive"))?;
        if let Some(host) = &self.host {
            return host
                .discard_live(unit)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()));
        }
        Err(crate::NativeError::invalid_input(
            "detached history cannot discard a unit",
        ))
    }

    fn from_host(host: HostHistory, view_runtime: usize) -> Self {
        Self {
            state: Mutex::new(History::new()),
            host: Some(host),
            alive: AtomicBool::new(true),
            view_runtime,
        }
    }

    #[napi(js_name = "pushStream")]
    pub fn push_stream(&self, stream: &NativeTextStream) -> Result<()> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .push_stream(&stream.stream)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()));
        }
        let mut history = self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?;
        stream
            .stream
            .attach(&mut history)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi(js_name = "sealStream")]
    pub fn seal_stream(&self, stream: &NativeTextStream) -> Result<()> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .seal_stream(&stream.stream)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()));
        }
        let mut history = self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?;
        stream
            .stream
            .seal_history(&mut history)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }
}

#[napi]
pub struct NativeTextInput {
    state: Mutex<TextInput>,
    host: Option<HostTextInput>,
    alive: AtomicBool,
}

#[napi]
impl NativeTextInput {
    #[napi(constructor)]
    pub fn new(multiline: Option<bool>) -> Self {
        Self {
            state: Mutex::new(TextInput::new().multiline(multiline.unwrap_or(false))),
            host: None,
            alive: AtomicBool::new(true),
        }
    }

    #[napi]
    pub fn dispose(&self) {
        self.alive.store(false, Ordering::Release);
    }

    #[napi]
    pub fn text(&self) -> Result<String> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .text()
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        Ok(self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .text()
            .to_owned())
    }

    #[napi(js_name = "cursorBytes")]
    pub fn cursor_bytes(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .cursor_bytes()
                .map(|cursor| cursor as i64)
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        Ok(self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .cursor_bytes() as i64)
    }

    #[napi(js_name = "setText")]
    pub fn set_text(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .set_text(text)
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        self.state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .set_text(text);
        Ok(())
    }

    #[napi]
    pub fn clear(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .clear()
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        self.state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .clear();
        Ok(())
    }

    #[napi(js_name = "setMultiline")]
    pub fn set_multiline(&self, enabled: bool) -> Result<()> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .set_multiline(enabled)
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        self.state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .set_multiline(enabled);
        Ok(())
    }

    #[napi(js_name = "isMultiline")]
    pub fn is_multiline(&self) -> Result<bool> {
        ensure_alive(&self.alive)?;
        if let Some(host) = &self.host {
            return host
                .is_multiline()
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        Ok(self
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
            .is_multiline())
    }

    #[napi]
    pub fn submitted(&self) -> Result<NativeTuiOutput> {
        ensure_alive(&self.alive)?;
        let output = if let Some(host) = &self.host {
            host.submitted()
                .map_err(|error| crate::NativeError::internal(error.to_string()))?
        } else {
            self.state
                .lock()
                .map_err(|_| crate::NativeError::internal("text input lock is poisoned"))?
                .submitted()
        };
        Ok(NativeTuiOutput { output })
    }

    #[napi(js_name = "componentId")]
    pub fn component_id(&self) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        Ok(self
            .host
            .as_ref()
            .and_then(HostTextInput::component_id)
            .map(|id| id as i64))
    }

    fn from_host(host: HostTextInput) -> Self {
        Self {
            state: Mutex::new(TextInput::new()),
            host: Some(host),
            alive: AtomicBool::new(true),
        }
    }
}

#[napi]
pub struct NativeTuiHost {
    host: Box<TuiHost>,
    alive: AtomicBool,
    view_runtime: usize,
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    fast_shared: Box<fast_shared::FastSession>,
}

#[napi]
impl NativeTuiHost {
    #[napi(constructor)]
    pub fn new(
        env: Env,
        width: Option<i64>,
        height: Option<i64>,
        headless: Option<bool>,
    ) -> Result<Self> {
        let width = width.unwrap_or(80);
        let height = height.unwrap_or(24);
        let width = u16::try_from(width)
            .map_err(|_| crate::NativeError::invalid_input("width must fit in u16"))?;
        let height = u16::try_from(height)
            .map_err(|_| crate::NativeError::invalid_input("height must fit in u16"))?;
        let mut host = Box::new(
            TuiHost::open(width, height, headless.unwrap_or(false))
                .map_err(|error| crate::NativeError::internal(error.to_string()))?,
        );
        let view_runtime = view_abi::runtime_ptr_for_env(&env)? as usize;
        #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
        let runtime = view_runtime as *mut view_abi::NativeViewRuntime;
        #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
        let mut fast_shared = Box::new(fast_shared::FastSession::new(&mut host, runtime));
        #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
        fast_shared.register().map_err(|_| {
            crate::NativeError::internal("FastShared runtime host registry is full")
        })?;
        Ok(Self {
            host,
            alive: AtomicBool::new(true),
            view_runtime,
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            fast_shared,
        })
    }

    #[napi]
    pub fn dispose(&self) -> Result<()> {
        if self.alive.swap(false, Ordering::AcqRel) {
            view_abi::abort_all_edit_txns(self.view_runtime as *mut view_abi::NativeViewRuntime);
            #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
            {
                self.fast_shared.close();
                self.fast_shared.unregister();
            }
            self.host
                .close()
                .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        }
        Ok(())
    }

    #[napi]
    pub fn exit(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .exit()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "nextWakeMs")]
    pub fn next_wake_ms(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(i64::try_from(self.host.next_wake_ms()).unwrap_or(i64::MAX))
    }

    #[napi]
    pub fn set_theme(&self, value: Value) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .set_theme(lower_theme(&value)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "setHistory")]
    pub fn set_history(&self, history: &mut NativeHistory) -> Result<()> {
        ensure_alive(&self.alive)?;
        let detached = history.take_for_host()?;
        self.host
            .set_history(detached)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        history.host = Some(self.host.history());
        Ok(())
    }

    #[napi]
    pub fn exited(&self) -> Result<bool> {
        Ok(self.host.exited())
    }

    #[napi(js_name = "styleAt")]
    pub fn style_at(&self, row: i64, column: i64) -> Result<Option<Value>> {
        ensure_alive(&self.alive)?;
        let row = u16::try_from(row)
            .map_err(|_| crate::NativeError::invalid_input("row must fit in u16"))?;
        let column = u16::try_from(column)
            .map_err(|_| crate::NativeError::invalid_input("column must fit in u16"))?;
        Ok(self.host.style_at(row, column).map(cell_style_value))
    }

    #[napi(js_name = "cellXOfText")]
    pub fn cell_x_of_text(&self, row: i64, text: String) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        let row = u16::try_from(row)
            .map_err(|_| crate::NativeError::invalid_input("row must fit in u16"))?;
        Ok(self.host.cell_x_of_text(row, &text).map(i64::from))
    }

    #[napi]
    pub fn history(&self) -> Result<NativeHistory> {
        ensure_alive(&self.alive)?;
        Ok(NativeHistory::from_host(
            self.host.history(),
            self.view_runtime,
        ))
    }

    #[napi(js_name = "textInput")]
    pub fn text_input(
        &self,
        multiline: Option<bool>,
        border: Option<Value>,
    ) -> Result<NativeTextInput> {
        ensure_alive(&self.alive)?;
        let input = self
            .host
            .create_text_input(multiline.unwrap_or(false))
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        if let Some(border) = border {
            input
                .set_border(lower_border(&border)?)
                .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        }
        Ok(NativeTextInput::from_host(input))
    }

    #[napi(js_name = "createViewSlot")]
    pub fn create_view_slot(&self, initial: Object) -> Result<NativeViewSlot> {
        ensure_alive(&self.alive)?;
        let initial = decode_view(&initial)?;
        let slot = self
            .host
            .create_view_slot(initial)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeViewSlot::from_host(slot, self.view_runtime))
    }

    #[napi(js_name = "createViewSlotRef")]
    pub fn create_view_slot_ref(&self, view_ref: i64) -> Result<NativeViewSlot> {
        ensure_alive(&self.alive)?;
        let slot = self
            .host
            .create_view_slot(resolve_native_view(self.view_runtime, view_ref)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeViewSlot::from_host(slot, self.view_runtime))
    }

    #[napi(js_name = "scrollPane")]
    pub fn scroll_pane(&self, initial: Object) -> Result<NativeScrollPane> {
        ensure_alive(&self.alive)?;
        let initial = decode_view(&initial)?;
        let pane = self
            .host
            .create_scroll_pane(initial)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeScrollPane::from_host(pane, self.view_runtime))
    }

    #[napi(js_name = "scrollPaneRef")]
    pub fn scroll_pane_ref(&self, view_ref: i64) -> Result<NativeScrollPane> {
        ensure_alive(&self.alive)?;
        let pane = self
            .host
            .create_scroll_pane(resolve_native_view(self.view_runtime, view_ref)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeScrollPane::from_host(pane, self.view_runtime))
    }

    #[napi(js_name = "bindKey")]
    pub fn bind_key(
        &self,
        key: String,
        modifiers: Option<Vec<String>>,
        route_id: String,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .bind_key(parse_key(&key, modifiers.as_deref())?, route_id)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn route(&self, output: &NativeTuiOutput, route_id: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .route_text_input_output(output.output, route_id)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi(js_name = "interceptPaste")]
    pub fn intercept_paste(&self, input: &NativeTextInput, route_id: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        let host_input = input
            .host
            .as_ref()
            .ok_or_else(|| crate::NativeError::invalid_input("text input is not mounted"))?;
        self.host
            .intercept_paste(host_input, route_id)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn render(&self, view: Object) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .render(decode_view(&view)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    /// Stable opaque pointer for generated host-mutating View ABI calls.
    /// The N-API class allocation owns `self` until finalization; `dispose`
    /// tombstones the host before the pointer can be used again.
    #[napi(js_name = "tuiViewAbiHostPointer")]
    pub fn view_abi_host_pointer(&self) -> i64 {
        if !self.alive.load(Ordering::Acquire) {
            return 0;
        }
        self as *const Self as usize as i64
    }

    #[napi(js_name = "dispatchKey")]
    pub fn dispatch_key(&self, key: String, modifiers: Option<Vec<String>>) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .dispatch_key(parse_key(&key, modifiers.as_deref())?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "dispatchPaste")]
    pub fn dispatch_paste(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .dispatch_paste(&text)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "forwardPaste")]
    pub fn forward_paste(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .forward_paste(&text)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "pollTerminal")]
    pub fn poll_terminal(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .poll_terminal()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "nextOutput")]
    pub fn next_output(&self) -> Result<Option<Value>> {
        ensure_alive(&self.alive)?;
        Ok(self.host.next_output().map(
            |output| serde_json::json!({"route_id": output.route_id, "payload": output.payload}),
        ))
    }

    #[napi(js_name = "nextAction")]
    pub fn next_action(&self) -> Result<Option<Value>> {
        ensure_alive(&self.alive)?;
        Ok(self.host.next_action().map(
            |action| serde_json::json!({"action_id": action.route_id, "payload": action.payload}),
        ))
    }

    /// Wait until native generic key routing produces a routed output.
    #[napi(js_name = "waitForOutput")]
    pub async fn wait_for_output(&self) -> Result<Option<Value>> {
        ensure_alive(&self.alive)?;
        let host = self.host.clone();
        let output = host
            .wait_for_output()
            .await
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(output.map(
            |output| serde_json::json!({"route_id": output.route_id, "payload": output.payload}),
        ))
    }

    /// Wait in the native TUI driver until Rust has routed a semantic action
    /// or the host exits. Raw terminal events never cross this boundary.
    #[napi(js_name = "waitForAction")]
    pub async fn wait_for_action(&self) -> Result<Option<Value>> {
        ensure_alive(&self.alive)?;
        let host = self.host.clone();
        let action = host
            .wait_for_action()
            .await
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(action.map(
            |action| serde_json::json!({"action_id": action.route_id, "payload": action.payload}),
        ))
    }

    #[napi(js_name = "screenRows")]
    pub fn screen_rows(&self) -> Result<Vec<String>> {
        ensure_alive(&self.alive)?;
        Ok(self.host.screen_rows())
    }

    #[napi(js_name = "nativeHistoryRows")]
    pub fn native_history_rows(&self) -> Result<Vec<String>> {
        ensure_alive(&self.alive)?;
        Ok(self.host.native_history_rows())
    }

    #[napi]
    pub fn resize(&self, width: i64, height: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let width = u16::try_from(width)
            .map_err(|_| crate::NativeError::invalid_input("width must fit in u16"))?;
        let height = u16::try_from(height)
            .map_err(|_| crate::NativeError::invalid_input("height must fit in u16"))?;
        self.host
            .resize(width, height)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "advanceTime")]
    pub fn advance_time(&self, milliseconds: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let milliseconds = u64::try_from(milliseconds)
            .map_err(|_| crate::NativeError::invalid_input("time must be non-negative"))?;
        self.host
            .advance_time(std::time::Duration::from_millis(milliseconds))
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }
}

#[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
#[napi]
impl NativeTuiHost {
    #[napi(js_name = "tuiPerfFastSharedAbi")]
    pub fn fast_shared_abi(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        Ok(self.fast_shared.descriptor())
    }

    /// Benchmark-only packed transport. The environment is injected by N-API
    /// so this path shares the direct decoder's environment-local weak cache.
    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfPackedRender")]
    pub fn render_packed(
        &self,
        env: Env,
        words: napi::bindgen_prelude::Uint32Array,
        strings: Vec<String>,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed::decode_one(words.as_ref(), &strings, cache)?;
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfV3PackedRender")]
    pub fn render_packed_v3(
        &self,
        env: Env,
        words: napi::bindgen_prelude::Uint32Array,
        bytes: napi::bindgen_prelude::Uint8Array,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed_v3::decode_render(words.as_ref(), bytes.as_ref(), cache)?;
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::NapiV3HostMutations);
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfV3PackedRenderStrings")]
    pub fn render_packed_v3_strings(
        &self,
        env: Env,
        words: napi::bindgen_prelude::Uint32Array,
        strings: Vec<String>,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed_v3::decode_render_strings(words.as_ref(), strings, cache)?;
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::NapiV3HostMutations);
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfV4PackedRender")]
    pub fn render_packed_v4(
        &self,
        env: Env,
        words: napi::bindgen_prelude::Uint32Array,
        bytes: napi::bindgen_prelude::Uint8Array,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed_v4::decode_render(words.as_ref(), bytes.as_ref(), cache)?;
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::NapiV4HostMutations);
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfV4PackedRenderRef")]
    pub fn render_packed_v4_ref(&self, env: Env, generation: i64, packed_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed_v4::resolve_ref(generation, packed_ref, cache)?;
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::NapiV4HostMutations);
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[cfg(any(feature = "perf-packed-benchmark", feature = "perf-packed-timing"))]
    #[napi(js_name = "tuiPerfV3PackedRenderRef")]
    pub fn render_packed_v3_ref(&self, env: Env, generation: i64, packed_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let cache = view_bridge_cache_for_env(&env)?;
        let view = packed_v3::resolve_ref(generation, packed_ref, cache)?;
        #[cfg(feature = "perf-counters")]
        iyon_tui::perf::inc(iyon_tui::perf::Counter::NapiV3HostMutations);
        self.host
            .render(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }
}

fn parse_key(key: &str, modifiers: Option<&[String]>) -> Result<KeyStroke> {
    let key = match key {
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        "Delete" => Key::Delete,
        "Insert" => Key::Insert,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        value => {
            let mut chars = value.chars();
            let Some(character) = chars.next() else {
                return Err(crate::NativeError::invalid_input("key must not be empty"));
            };
            if chars.next().is_some() {
                return Err(crate::NativeError::invalid_input(
                    "character key must contain one character",
                ));
            }
            Key::Char(character)
        }
    };
    let mut flags = Modifiers::NONE;
    for modifier in modifiers.unwrap_or_default() {
        flags = flags.union(match modifier.to_ascii_lowercase().as_str() {
            "shift" => Modifiers::SHIFT,
            "control" | "ctrl" => Modifiers::CONTROL,
            "alt" | "option" => Modifiers::ALT,
            "super" | "meta" => Modifiers::SUPER,
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown key modifier `{other}`"
                )));
            }
        });
    }
    Ok(KeyStroke::with_modifiers(key, flags))
}

#[napi]
pub struct NativeTextStream {
    stream: HostTextStream,
    alive: AtomicBool,
}

#[napi]
impl NativeTextStream {
    #[napi(constructor)]
    pub fn new(options: Option<Value>) -> Result<Self> {
        let (markdown, insets, pacing) = parse_stream_options(options)?;
        Ok(Self {
            stream: if markdown {
                HostTextStream::with_markdown_presentation(
                    iyon_tui::TextStreamPresentation::new(insets).with_pacing(pacing),
                )
            } else {
                HostTextStream::new()
            },
            alive: AtomicBool::new(true),
        })
    }

    #[napi]
    pub fn dispose(&self) {
        self.alive.store(false, Ordering::Release);
    }

    #[napi]
    pub fn update(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.stream
            .update(text)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn append(&self, text: String, annotations: Option<Vec<Value>>) -> Result<()> {
        ensure_alive(&self.alive)?;
        let annotations = annotations
            .unwrap_or_default()
            .into_iter()
            .map(parse_stream_annotation)
            .collect::<Result<Vec<_>>>()?;
        self.stream
            .append(text, &annotations)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn seal(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.stream
            .seal()
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi]
    pub fn snapshot(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let (text, revision, sealed, segments) = self
            .stream
            .snapshot_json()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        let mut snapshot =
            serde_json::json!({"text": text, "revision": revision, "sealed": sealed});
        if segments
            .iter()
            .any(|(annotations, _)| !annotations.is_empty())
        {
            snapshot["segments"] = serde_json::Value::Array(
                segments
                    .into_iter()
                    .map(|(annotations, text)| {
                        let annotations = annotations
                            .into_iter()
                            .map(|annotation| {
                                serde_json::json!({
                                    "namespace": annotation.namespace,
                                    "name": annotation.name,
                                })
                            })
                            .collect::<Vec<_>>();
                        serde_json::json!({"annotations": annotations, "text": text})
                    })
                    .collect(),
            );
        }
        Ok(snapshot)
    }
}

fn parse_stream_options(
    value: Option<Value>,
) -> Result<(bool, iyon_tui::Insets, iyon_tui::SmoothConfig)> {
    let Some(value) = value else {
        return Ok((false, iyon_tui::Insets::ZERO, iyon_tui::SmoothConfig::new()));
    };
    if let Some(projector) = value.as_str() {
        return match projector {
            "markdown" => Ok((true, iyon_tui::Insets::ZERO, iyon_tui::SmoothConfig::new())),
            "" => Ok((false, iyon_tui::Insets::ZERO, iyon_tui::SmoothConfig::new())),
            _ => Err(crate::NativeError::invalid_input(
                "stream projector must be markdown",
            )),
        };
    }
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("stream options must be an object"))?;
    let markdown = match object.get("projector").and_then(Value::as_str) {
        None | Some("") => false,
        Some("markdown") => true,
        Some(_) => {
            return Err(crate::NativeError::invalid_input(
                "stream projector must be markdown",
            ));
        }
    };
    let insets = object
        .get("presentation")
        .and_then(Value::as_object)
        .and_then(|presentation| presentation.get("insets"))
        .map(|value| -> Result<iyon_tui::Insets> {
            let insets = value.as_object().ok_or_else(|| {
                crate::NativeError::invalid_input("stream insets must be an object")
            })?;
            Ok(iyon_tui::Insets::new(
                u16_value(insets, "top")?,
                u16_value(insets, "right")?,
                u16_value(insets, "bottom")?,
                u16_value(insets, "left")?,
            ))
        })
        .transpose()?
        .unwrap_or(iyon_tui::Insets::ZERO);
    let pacing = parse_stream_pacing(object.get("pacing"))?;
    Ok((markdown, insets, pacing))
}

fn parse_stream_pacing(value: Option<&Value>) -> Result<iyon_tui::SmoothConfig> {
    let Some(value) = value else {
        return Ok(iyon_tui::SmoothConfig::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("stream pacing must be an object"))?;
    let defaults = iyon_tui::SmoothConfig::new();
    let tick_interval_ms = object
        .get("tickIntervalMs")
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                crate::NativeError::invalid_input("stream pacing tickIntervalMs must be an integer")
            })
        })
        .transpose()?
        .unwrap_or(
            u64::try_from(defaults.tick_interval().as_millis())
                .expect("default stream tick interval fits u64"),
        );
    let spring = pacing_f32(object, "spring", defaults.spring())?;
    let minimum = pacing_f32(object, "minUnitsPerSecond", defaults.min_units_per_second())?;
    let maximum = pacing_f32(object, "maxUnitsPerSecond", defaults.max_units_per_second())?;
    iyon_tui::SmoothConfig::try_from_parts(
        Duration::from_millis(tick_interval_ms),
        spring,
        minimum,
        maximum,
    )
    .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

fn pacing_f32(object: &Map<String, Value>, field: &str, default: f32) -> Result<f32> {
    let Some(value) = object.get(field) else {
        return Ok(default);
    };
    let value = value.as_f64().ok_or_else(|| {
        crate::NativeError::invalid_input(format!("stream pacing {field} must be a number"))
    })?;
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(crate::NativeError::invalid_input(format!(
            "stream pacing {field} must be finite"
        )));
    }
    Ok(value as f32)
}

fn parse_stream_annotation(value: Value) -> Result<iyon_tui::TextStreamAnnotation> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("stream annotation must be an object"))?;
    let namespace = object
        .get("namespace")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            crate::NativeError::invalid_input("stream annotation namespace is required")
        })?;
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::NativeError::invalid_input("stream annotation name is required"))?;
    Ok(iyon_tui::TextStreamAnnotation {
        namespace: namespace.to_owned(),
        name: name.to_owned(),
    })
}

#[napi]
pub struct NativeMarkdownProjector {
    projector: Mutex<MarkdownProjector>,
    alive: AtomicBool,
}

#[napi]
pub struct NativePlainProjector {
    alive: AtomicBool,
}

#[napi]
impl NativePlainProjector {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            alive: AtomicBool::new(true),
        }
    }

    #[napi]
    pub fn dispose(&self) {
        self.alive.store(false, Ordering::Release);
    }

    #[napi]
    pub fn project(&self, text: String) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let length = text.len() as u64;
        Ok(serde_json::json!({
            "spans": [{"sourceStart": 0, "sourceEnd": length, "text": text}],
        }))
    }
}

#[napi]
impl NativeMarkdownProjector {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            projector: Mutex::new(MarkdownProjector::new(MarkdownOptions::commonmark())),
            alive: AtomicBool::new(true),
        }
    }

    #[napi]
    pub fn dispose(&self) {
        self.alive.store(false, Ordering::Release);
    }

    #[napi]
    pub fn project(&self, text: String, sealed: Option<bool>) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let sealed = sealed.unwrap_or(true);
        let end = StreamOffset::new(text.len() as u64);
        let input = ProjectionBuilder::new(
            StreamOffset::ZERO,
            if sealed { end } else { StreamOffset::ZERO },
            end,
            sealed,
        )
        .emit(
            StreamRange::new(StreamOffset::ZERO, end),
            TextContent::raw(text),
        )
        .finish()
        .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        let projection = self
            .projector
            .lock()
            .map_err(|_| crate::NativeError::internal("markdown projector lock is poisoned"))?
            .project(&input)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        let spans = projection
            .spans()
            .iter()
            .map(|span| {
                let mut output = String::new();
                for value in span.values() {
                    let mut visitor = PlainTextVisitor {
                        output: String::new(),
                    };
                    visitor.visit_content(value);
                    output.push_str(&visitor.output);
                }
                serde_json::json!({
                    "sourceStart": span.source().start().as_u64(),
                    "sourceEnd": span.source().end().as_u64(),
                    "text": output,
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({"spans": spans}))
    }
}

struct PlainTextVisitor {
    output: String,
}

impl TextVisitor for PlainTextVisitor {
    fn visit_raw(&mut self, raw: &iyon_tui::RawText) {
        self.output.push_str(raw.text());
    }

    fn visit_text_run(&mut self, run: &TextRun) {
        self.output.push_str(run.text());
    }
}

#[napi]
pub struct NativeViewSlot {
    slot: HostViewSlot,
    alive: AtomicBool,
    view_runtime: usize,
}

#[napi]
pub struct NativeScrollPane {
    pane: HostScrollPane,
    alive: AtomicBool,
    view_runtime: usize,
}

#[napi]
impl NativeScrollPane {
    #[napi(constructor)]
    pub fn new(env: Env, initial: Object) -> Result<Self> {
        Ok(Self {
            pane: HostScrollPane::new(decode_view(&initial)?),
            alive: AtomicBool::new(true),
            view_runtime: view_abi::runtime_ptr_for_env(&env)? as usize,
        })
    }

    #[napi]
    pub fn dispose(&self) {
        // PERF-12 T13.1 R8: disposal REQUESTS deferred retirement of the
        // registered component (idempotent); physical reclamation happens in
        // RunningApp::reap_retired_components after reconciliation proves the
        // component unmounted. The N-API surface is the durable public path.
        self.pane.retire();
        self.alive.store(false, Ordering::Release);
    }

    #[napi(js_name = "componentId")]
    pub fn component_id(&self) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        Ok(self.pane.component_id().map(|id| id as i64))
    }

    #[napi(js_name = "setContent")]
    pub fn set_content(&self, view: Object) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_content_view(decode_view(&view)?)
    }

    #[napi(js_name = "setContentRef")]
    pub fn set_content_ref(&self, view_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_content_view(resolve_native_view(self.view_runtime, view_ref)?)
    }

    fn set_content_view(&self, view: View) -> Result<()> {
        self.pane
            .set_content(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "followEnd")]
    pub fn follow_end(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.pane
            .follow_end()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    fn from_host(pane: HostScrollPane, view_runtime: usize) -> Self {
        Self {
            pane,
            alive: AtomicBool::new(true),
            view_runtime,
        }
    }
}

#[napi]
impl NativeViewSlot {
    #[napi(constructor)]
    pub fn new(env: Env, initial: Object) -> Result<Self> {
        Ok(Self {
            slot: HostViewSlot::new(decode_view(&initial)?),
            alive: AtomicBool::new(true),
            view_runtime: view_abi::runtime_ptr_for_env(&env)? as usize,
        })
    }

    #[napi]
    pub fn dispose(&self) {
        // PERF-12 T13.1 R8: disposal REQUESTS deferred retirement of the
        // registered component (idempotent). Physical reclamation happens in
        // RunningApp::reap_retired_components after reconciliation proves the
        // component unmounted — committed roots may still reference it.
        self.slot.retire();
        self.alive.store(false, Ordering::Release);
    }

    #[napi]
    pub fn revision(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(self.slot.revision() as i64)
    }

    #[napi(js_name = "componentId")]
    pub fn component_id(&self) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        Ok(self.slot.component_id().map(|id| id as i64))
    }

    #[napi(js_name = "setView")]
    pub fn set_view(&self, view: Object) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_view_value(decode_view(&view)?)
    }

    #[napi(js_name = "setViewRef")]
    pub fn set_view_ref(&self, view_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_view_value(resolve_native_view(self.view_runtime, view_ref)?)
    }

    fn set_view_value(&self, view: View) -> Result<()> {
        self.slot
            .set_view(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "setAnimation")]
    pub fn set_animation(&self, frames: Vec<Object>, interval_ms: i64) -> Result<()> {
        self.set_animation_with_mode(
            frames
                .into_iter()
                .map(|frame| decode_view(&frame))
                .collect::<Result<Vec<_>>>()?,
            interval_ms,
            false,
        )
    }

    #[napi(js_name = "setAnimationAtCycleBoundary")]
    pub fn set_animation_at_cycle_boundary(
        &self,
        frames: Vec<Object>,
        interval_ms: i64,
    ) -> Result<()> {
        self.set_animation_with_mode(
            frames
                .into_iter()
                .map(|frame| decode_view(&frame))
                .collect::<Result<Vec<_>>>()?,
            interval_ms,
            true,
        )
    }

    #[napi(js_name = "setAnimationRefs")]
    pub fn set_animation_refs(
        &self,
        refs: napi::bindgen_prelude::Uint32Array,
        used_count: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let used_count = usize::try_from(used_count).map_err(|_| {
            crate::NativeError::invalid_input("animation used count must be non-negative")
        })?;
        let refs = refs.as_ref();
        if used_count == 0 || used_count > refs.len() {
            return Err(crate::NativeError::invalid_input(
                "animation used count is out of range",
            ));
        }
        let frames = refs[..used_count]
            .iter()
            .copied()
            .map(|view_ref| resolve_native_view(self.view_runtime, i64::from(view_ref)))
            .collect::<Result<Vec<_>>>()?;
        self.set_animation_with_mode(frames, interval_ms, false)
    }

    #[napi(js_name = "setAnimationRefsAtCycleBoundary")]
    pub fn set_animation_refs_at_cycle_boundary(
        &self,
        refs: napi::bindgen_prelude::Uint32Array,
        used_count: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let used_count = usize::try_from(used_count).map_err(|_| {
            crate::NativeError::invalid_input("animation used count must be non-negative")
        })?;
        let refs = refs.as_ref();
        if used_count == 0 || used_count > refs.len() {
            return Err(crate::NativeError::invalid_input(
                "animation used count is out of range",
            ));
        }
        let frames = refs[..used_count]
            .iter()
            .copied()
            .map(|view_ref| resolve_native_view(self.view_runtime, i64::from(view_ref)))
            .collect::<Result<Vec<_>>>()?;
        self.set_animation_with_mode(frames, interval_ms, true)
    }

    fn set_animation_ref_values(
        &self,
        refs: &[i64],
        interval_ms: i64,
        at_cycle_boundary: bool,
    ) -> Result<()> {
        let frames = refs
            .iter()
            .copied()
            .map(|view_ref| resolve_native_view(self.view_runtime, view_ref))
            .collect::<Result<Vec<_>>>()?;
        self.set_animation_with_mode(frames, interval_ms, at_cycle_boundary)
    }

    #[napi(js_name = "setAnimationRef1")]
    pub fn set_animation_ref1(&self, ref0: i64, interval_ms: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0], interval_ms, false)
    }

    #[napi(js_name = "setAnimationRef2")]
    pub fn set_animation_ref2(&self, ref0: i64, ref1: i64, interval_ms: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1], interval_ms, false)
    }

    #[napi(js_name = "setAnimationRef3")]
    pub fn set_animation_ref3(
        &self,
        ref0: i64,
        ref1: i64,
        ref2: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1, ref2], interval_ms, false)
    }

    #[napi(js_name = "setAnimationRef4")]
    pub fn set_animation_ref4(
        &self,
        ref0: i64,
        ref1: i64,
        ref2: i64,
        ref3: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1, ref2, ref3], interval_ms, false)
    }

    #[napi(js_name = "setAnimationRef1AtCycleBoundary")]
    pub fn set_animation_ref1_at_cycle_boundary(&self, ref0: i64, interval_ms: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0], interval_ms, true)
    }

    #[napi(js_name = "setAnimationRef2AtCycleBoundary")]
    pub fn set_animation_ref2_at_cycle_boundary(
        &self,
        ref0: i64,
        ref1: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1], interval_ms, true)
    }

    #[napi(js_name = "setAnimationRef3AtCycleBoundary")]
    pub fn set_animation_ref3_at_cycle_boundary(
        &self,
        ref0: i64,
        ref1: i64,
        ref2: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1, ref2], interval_ms, true)
    }

    #[napi(js_name = "setAnimationRef4AtCycleBoundary")]
    pub fn set_animation_ref4_at_cycle_boundary(
        &self,
        ref0: i64,
        ref1: i64,
        ref2: i64,
        ref3: i64,
        interval_ms: i64,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.set_animation_ref_values(&[ref0, ref1, ref2, ref3], interval_ms, true)
    }

    fn set_animation_with_mode(
        &self,
        frames: Vec<View>,
        interval_ms: i64,
        at_cycle_boundary: bool,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let interval_ms = u64::try_from(interval_ms).map_err(|_| {
            crate::NativeError::invalid_input("animation interval must be positive")
        })?;
        if interval_ms == 0 {
            return Err(crate::NativeError::invalid_input(
                "animation interval must be positive",
            ));
        }
        if frames.is_empty() {
            return Err(crate::NativeError::invalid_input(
                "animation requires at least one frame",
            ));
        }
        let interval = std::time::Duration::from_millis(interval_ms);
        let result = if at_cycle_boundary {
            self.slot.set_animation_at_cycle_boundary(frames, interval)
        } else {
            self.slot.set_animation(frames, interval)
        };
        result.map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "stopAnimation")]
    pub fn stop_animation(&self, view: Object) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.stop_animation_view(decode_view(&view)?)
    }

    #[napi(js_name = "stopAnimationRef")]
    pub fn stop_animation_ref(&self, view_ref: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.stop_animation_view(resolve_native_view(self.view_runtime, view_ref)?)
    }

    fn stop_animation_view(&self, view: View) -> Result<()> {
        self.slot
            .stop_animation(view)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    fn from_host(slot: HostViewSlot, view_runtime: usize) -> Self {
        Self {
            slot,
            alive: AtomicBool::new(true),
            view_runtime,
        }
    }
}

fn decode_view(value: &Object<'_>) -> Result<View> {
    let cache = view_bridge_cache(value)?;
    let mut decoder = ViewDecoder {
        cache,
        active: HashSet::new(),
    };
    decoder.decode(*value)
}

/// Exceptional PERF-12 §73 recovery helper. It decodes one semantic bridge
/// node synchronously, publishes through the shared runtime identity funnel,
/// and returns one leased root reference. No N-API value is retained.
#[napi(js_name = "tuiViewAbiDecodeRef")]
pub fn tui_view_abi_decode_ref(value: Object) -> Result<i64> {
    let node_id = required_u64(&value, "id")?;
    let cache = view_bridge_cache(&value)?;
    let view = decode_view(&value)?;
    let reference = view_abi::publish_decoded_view(&cache, node_id, view)?;
    Ok(i64::from(reference))
}

struct ViewDecoder {
    cache: ViewRuntimeHandle,
    active: HashSet<u64>,
}

fn is_known_view_kind(kind: u32) -> bool {
    matches!(
        kind,
        VIEW_KIND_TEXT
            | VIEW_KIND_DIFF
            | VIEW_KIND_SPACER
            | VIEW_KIND_ROW
            | VIEW_KIND_COLUMN
            | VIEW_KIND_HANGING
            | VIEW_KIND_GRID
            | VIEW_KIND_CONTAINER
            | VIEW_KIND_CLAMP
            | VIEW_KIND_CONTENT_MAX
            | VIEW_KIND_COMPONENT
            | VIEW_KIND_DECORATED
    )
}

fn has_cached_node_payload(value: &Object<'_>) -> Result<bool> {
    [
        "spans",
        "hunks",
        "rows",
        "children",
        "columns",
        "child",
        "prefix",
        "continuation",
        "body",
        "handle",
        "decoration",
    ]
    .into_iter()
    .map(|field| {
        value
            .get::<Unknown>(field)
            .map(|property| property.is_some())
    })
    .try_fold(false, |found, property| {
        property.map(|present| found || present)
    })
}

fn validate_cached_node_header(value: &Object<'_>, kind: u32) -> Result<()> {
    match kind {
        VIEW_KIND_TEXT => {
            required_prop::<Array>(value, "spans")?;
            required_prop::<u32>(value, "wrap")?;
            required_prop::<u32>(value, "align")?;
        }
        VIEW_KIND_DIFF => {
            required_prop::<Array>(value, "hunks")?;
        }
        VIEW_KIND_SPACER => {
            required_u16(value, "rows")?;
        }
        VIEW_KIND_ROW | VIEW_KIND_COLUMN => {
            required_u16(value, "gap")?;
            required_prop::<Array>(value, "children")?;
        }
        VIEW_KIND_HANGING => {
            required_prop::<Object>(value, "prefix")?;
            required_prop::<Object>(value, "continuation")?;
            required_prop::<Object>(value, "body")?;
        }
        VIEW_KIND_GRID => {
            required_prop::<Array>(value, "columns")?;
            let rows = required_prop::<Array>(value, "rows")?;
            for index in 0..rows.len() {
                let row = rows.get_element::<Object>(index)?;
                required_prop::<Object>(&row, "track")?;
                let cells = required_prop::<Array>(&row, "cells")?;
                for cell_index in 0..cells.len() {
                    let cell = cells.get_element::<Object>(cell_index)?;
                    required_positive_u16(&cell, "columnSpan")?;
                    required_positive_u16(&cell, "rowSpan")?;
                    required_prop::<Object>(&cell, "view")?;
                }
            }
            required_u16(value, "columnGap")?;
            required_u16(value, "rowGap")?;
        }
        VIEW_KIND_CONTAINER => {
            required_prop::<Object>(value, "child")?;
        }
        VIEW_KIND_CLAMP => {
            required_prop::<Object>(value, "child")?;
            required_u16(value, "maxRows")?;
            required_prop::<Object>(value, "overflow")?;
        }
        VIEW_KIND_CONTENT_MAX => {
            required_prop::<Object>(value, "child")?;
            required_u16(value, "maxRows")?;
        }
        VIEW_KIND_COMPONENT => {
            required_positive_u64(value, "handle")?;
        }
        VIEW_KIND_DECORATED => {
            required_prop::<Object>(value, "child")?;
            required_prop::<Object>(value, "decoration")?;
        }
        _ => {}
    }
    Ok(())
}

impl ViewDecoder {
    fn decode(&mut self, value: Object<'_>) -> Result<View> {
        let node_id = required_u64(&value, "id")?;
        if node_id == 0 {
            return Err(crate::NativeError::invalid_input(
                "view node id must be positive",
            ));
        }
        tui_perf_inc!(NapiViewNodesSeen);

        let schema = required_prop::<u32>(&value, "schema")?;
        if schema != VIEW_BRIDGE_SCHEMA_VERSION {
            return Err(crate::NativeError::invalid_input(format!(
                "unsupported TUI View bridge schema {schema}, expected {VIEW_BRIDGE_SCHEMA_VERSION}"
            )));
        }
        let kind = required_prop::<u32>(&value, "kind")?;
        if !is_known_view_kind(kind) && !has_cached_node_payload(&value)? {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown numeric TUI View node kind {kind}"
            )));
        }
        validate_cached_node_header(&value, kind)?;
        let cached = with_view_runtime(&self.cache, |cache| cache.live_cached_view(node_id))?;
        if let Some(view) = cached {
            tui_perf_inc!(NapiViewCacheHits);
            return Ok(view);
        }
        with_view_runtime(&self.cache, |cache| cache.drop_cached_entry(node_id))?;
        tui_perf_inc!(NapiViewCacheMisses);

        if !self.active.insert(node_id) {
            return Err(crate::NativeError::invalid_input(
                "cyclic TUI View node graph",
            ));
        }
        let result = self.decode_miss(&value);
        self.active.remove(&node_id);
        let view = result?;

        with_view_runtime(&self.cache, |cache| {
            cache.record_decoded_semantic_view(node_id, &view)
        })
        .and_then(|recorded| {
            recorded.map_err(|_| {
                crate::NativeError::invalid_input(format!(
                    "view node id {node_id} changed semantic identity"
                ))
            })
        })?;
        Ok(view)
    }

    fn decode_miss(&mut self, value: &Object<'_>) -> Result<View> {
        let kind = required_prop::<u32>(value, "kind")?;
        match kind {
            VIEW_KIND_TEXT => {
                let spans = required_prop::<Array>(value, "spans")?;
                let mut lowered = Vec::with_capacity(spans.len() as usize);
                for index in 0..spans.len() {
                    lowered.push(decode_text_span(&spans.get_element::<Object>(index)?)?);
                }
                let view = View::styled_text(lowered)
                    .wrap(decode_wrap(required_prop::<u32>(value, "wrap")?)?)
                    .text_align(decode_horizontal_align(required_prop::<u32>(
                        value, "align",
                    )?)?)
                    .into_view();
                Ok(view)
            }
            VIEW_KIND_DIFF => {
                let hunks = required_prop::<Array>(value, "hunks")?;
                let mut lowered = Vec::with_capacity(hunks.len() as usize);
                for index in 0..hunks.len() {
                    lowered.push(decode_diff_hunk(&hunks.get_element::<Object>(index)?)?);
                }
                Ok(iyon_tui::DiffRenderer::new().render(lowered.as_slice()))
            }
            VIEW_KIND_SPACER => Ok(View::spacer(required_u16(value, "rows")?)),
            VIEW_KIND_ROW => self.decode_axis(value, true),
            VIEW_KIND_COLUMN => self.decode_axis(value, false),
            VIEW_KIND_HANGING => Ok(View::hanging(
                self.decode(required_prop::<Object>(value, "prefix")?)?,
                self.decode(required_prop::<Object>(value, "continuation")?)?,
                self.decode(required_prop::<Object>(value, "body")?)?,
            )),
            VIEW_KIND_GRID => self.decode_grid(value),
            VIEW_KIND_CONTAINER => Ok(self
                .decode(required_prop::<Object>(value, "child")?)?
                .container()),
            VIEW_KIND_CLAMP => {
                let child = self.decode(required_prop::<Object>(value, "child")?)?;
                Ok(child.clamp_rows(
                    required_u16(value, "maxRows")?,
                    decode_overflow(value.get::<Object>("overflow")?.as_ref())?,
                ))
            }
            VIEW_KIND_CONTENT_MAX => {
                let child = self.decode(required_prop::<Object>(value, "child")?)?;
                Ok(child.clamp_rows(
                    required_u16(value, "maxRows")?,
                    iyon_tui::OverflowIndicator::None,
                ))
            }
            VIEW_KIND_COMPONENT => Ok(View::native_component(required_positive_u64(
                value, "handle",
            )?)),
            VIEW_KIND_DECORATED => {
                let child = self.decode(required_prop::<Object>(value, "child")?)?;
                decode_decoration(child, &required_prop::<Object>(value, "decoration")?)
            }
            other => Err(crate::NativeError::invalid_input(format!(
                "unknown numeric TUI View node kind {other}"
            ))),
        }
    }

    fn decode_axis(&mut self, value: &Object<'_>, horizontal: bool) -> Result<View> {
        let gap = required_u16(value, "gap")?;
        let children = required_prop::<Array>(value, "children")?;
        let mut lowered = Vec::with_capacity(children.len() as usize);
        for index in 0..children.len() {
            let child = children.get_element::<Object>(index)?;
            let kind = required_prop::<u32>(&child, "kind")?;
            let view = self.decode(required_prop::<Object>(&child, "child")?)?;
            let size = child
                .get::<f64>("size")?
                .map(|value| number_to_u16(value, "layout child size"))
                .transpose()?;
            let max_rows = child
                .get::<f64>("maxRows")?
                .map(|value| number_to_u16(value, "layout child maxRows"))
                .transpose()?;
            match kind {
                LAYOUT_CHILD_NORMAL | LAYOUT_CHILD_FLEX => {}
                LAYOUT_CHILD_FIXED if size.is_some() => {}
                LAYOUT_CHILD_FIXED => {
                    return Err(crate::NativeError::invalid_input(
                        "fixed layout child size is required",
                    ));
                }
                LAYOUT_CHILD_CONTENT_MAX if !horizontal && max_rows.is_some() => {}
                LAYOUT_CHILD_CONTENT_MAX if horizontal => {
                    return Err(crate::NativeError::invalid_input(
                        "contentMax is only valid for vertical children",
                    ));
                }
                LAYOUT_CHILD_CONTENT_MAX => {
                    return Err(crate::NativeError::invalid_input(
                        "contentMax maxRows is required",
                    ));
                }
                LAYOUT_CHILD_FLEX_MAX if !horizontal && max_rows.is_some() => {}
                LAYOUT_CHILD_FLEX_MAX if horizontal => {
                    return Err(crate::NativeError::invalid_input(
                        "flexMax is only valid for vertical children",
                    ));
                }
                LAYOUT_CHILD_FLEX_MAX => {
                    return Err(crate::NativeError::invalid_input(
                        "flexMax maxRows is required",
                    ));
                }
                other => {
                    return Err(crate::NativeError::invalid_input(format!(
                        "unknown layout child kind {other}"
                    )));
                }
            }
            lowered.push((kind, size, max_rows, view));
        }
        if horizontal {
            Ok(View::horizontal(|row| {
                row.gap(gap);
                for (kind, size, _max_rows, view) in lowered {
                    match kind {
                        LAYOUT_CHILD_NORMAL => {
                            row.child(view);
                        }
                        LAYOUT_CHILD_FIXED => {
                            row.fixed(size.expect("validated fixed size"), view);
                        }
                        LAYOUT_CHILD_FLEX => {
                            row.flex(view);
                        }
                        _ => unreachable!("invalid horizontal layout child"),
                    }
                }
            }))
        } else {
            Ok(View::vertical(|column| {
                column.gap(gap);
                for (kind, size, max_rows, view) in lowered {
                    match kind {
                        LAYOUT_CHILD_NORMAL => {
                            column.child(view);
                        }
                        LAYOUT_CHILD_FIXED => {
                            column.fixed(size.expect("validated fixed size"), view);
                        }
                        LAYOUT_CHILD_FLEX => {
                            column.flex(view);
                        }
                        LAYOUT_CHILD_CONTENT_MAX => {
                            column.content_max(max_rows.expect("validated content max"), view);
                        }
                        LAYOUT_CHILD_FLEX_MAX => {
                            column.flex_max(max_rows.expect("validated flex max"), view);
                        }
                        _ => unreachable!("invalid vertical layout child"),
                    }
                }
            }))
        }
    }

    fn decode_grid(&mut self, value: &Object<'_>) -> Result<View> {
        let columns = required_prop::<Array>(value, "columns")?;
        let mut lowered_columns = Vec::with_capacity(columns.len() as usize);
        for index in 0..columns.len() {
            lowered_columns.push(decode_grid_track(&columns.get_element::<Object>(index)?)?);
        }
        let rows = required_prop::<Array>(value, "rows")?;
        let mut lowered_rows = Vec::with_capacity(rows.len() as usize);
        for index in 0..rows.len() {
            let row = rows.get_element::<Object>(index)?;
            let track = decode_grid_track(&required_prop::<Object>(&row, "track")?)?;
            let cells = required_prop::<Array>(&row, "cells")?;
            let mut lowered_cells = Vec::with_capacity(cells.len() as usize);
            for cell_index in 0..cells.len() {
                let cell = cells.get_element::<Object>(cell_index)?;
                let spec = GridCellSpec::new()
                    .column_span(required_positive_u16(&cell, "columnSpan")?)
                    .row_span(required_positive_u16(&cell, "rowSpan")?)
                    .horizontal_align(decode_horizontal_align(
                        cell.get::<u32>("horizontalAlign")?.unwrap_or(ALIGN_START),
                    )?)
                    .vertical_align(decode_vertical_align(
                        cell.get::<u32>("verticalAlign")?.unwrap_or(VERTICAL_TOP),
                    )?);
                lowered_cells.push((spec, self.decode(required_prop::<Object>(&cell, "view")?)?));
            }
            lowered_rows.push((track, lowered_cells));
        }
        let column_gap = required_u16(value, "columnGap")?;
        let row_gap = required_u16(value, "rowGap")?;
        Ok(View::grid(|grid| {
            grid.columns(lowered_columns);
            grid.column_gap(column_gap);
            grid.row_gap(row_gap);
            for (track, cells) in lowered_rows {
                grid.row_with(track, |row| {
                    for (spec, view) in &cells {
                        row.cell_with(*spec, view.clone());
                    }
                });
            }
        }))
    }
}

fn required_prop<'env, T: napi::bindgen_prelude::FromNapiValue>(
    object: &Object<'env>,
    field: &str,
) -> Result<T> {
    object.get(field)?.ok_or_else(|| {
        crate::NativeError::invalid_input(format!("view node field `{field}` is required"))
    })
}

fn required_u64(object: &Object<'_>, field: &str) -> Result<u64> {
    let value = required_prop::<f64>(object, field)?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > 9_007_199_254_740_991.0
    {
        return Err(crate::NativeError::invalid_input(format!(
            "{field} must be a safe integer"
        )));
    }
    Ok(value as u64)
}

fn number_to_u16(value: f64, field: &str) -> Result<u16> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > f64::from(u16::MAX) {
        return Err(crate::NativeError::invalid_input(format!(
            "{field} must fit in u16"
        )));
    }
    Ok(value as u16)
}

fn required_u16(object: &Object<'_>, field: &str) -> Result<u16> {
    number_to_u16(required_prop::<f64>(object, field)?, field)
}

fn required_positive_u16(object: &Object<'_>, field: &str) -> Result<u16> {
    let value = required_u16(object, field)?;
    if value == 0 {
        return Err(crate::NativeError::invalid_input(format!(
            "{field} must be positive"
        )));
    }
    Ok(value)
}

fn required_positive_u64(object: &Object<'_>, field: &str) -> Result<u64> {
    let value = required_u64(object, field)?;
    if value == 0 {
        return Err(crate::NativeError::invalid_input(format!(
            "{field} must be positive"
        )));
    }
    Ok(value)
}

fn decode_wrap(value: u32) -> Result<WrapMode> {
    match value {
        WRAP_WORD_THEN_GRAPHEME => Ok(WrapMode::WordThenGrapheme),
        WRAP_GRAPHEME => Ok(WrapMode::Grapheme),
        WRAP_NO_WRAP => Ok(WrapMode::NoWrap),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown wrap mode {other}"
        ))),
    }
}

fn decode_horizontal_align(value: u32) -> Result<HorizontalAlign> {
    match value {
        ALIGN_START => Ok(HorizontalAlign::Start),
        ALIGN_CENTER => Ok(HorizontalAlign::Center),
        ALIGN_END => Ok(HorizontalAlign::End),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown horizontal alignment {other}"
        ))),
    }
}

fn decode_vertical_align(value: u32) -> Result<VerticalAlign> {
    match value {
        VERTICAL_TOP => Ok(VerticalAlign::Top),
        VERTICAL_CENTER => Ok(VerticalAlign::Center),
        VERTICAL_BOTTOM => Ok(VerticalAlign::Bottom),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown vertical alignment {other}"
        ))),
    }
}

fn decode_diff_hunk(value: &Object<'_>) -> Result<iyon_tui::DiffHunk> {
    use iyon_tui::{DiffHunk, DiffLine, DiffLineNumber, DiffLineTermination};
    let old_range = decode_diff_range(&required_prop::<Object>(value, "oldRange")?)?;
    let new_range = decode_diff_range(&required_prop::<Object>(value, "newRange")?)?;
    let lines = required_prop::<Array>(value, "lines")?;
    let mut lowered = Vec::with_capacity(lines.len() as usize);
    for index in 0..lines.len() {
        let line = lines.get_element::<Object>(index)?;
        let kind = required_prop::<u32>(&line, "kind")?;
        let text = required_prop::<String>(&line, "text")?;
        tui_perf_add!(NapiViewStringBytesCopied, text.len());
        let termination = match line.get::<u32>("termination")?.unwrap_or(DIFF_TERMINATED) {
            DIFF_TERMINATED => DiffLineTermination::Terminated,
            DIFF_UNTERMINATED => DiffLineTermination::Unterminated,
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown diff line termination {other}"
                )));
            }
        };
        let lowered_line = match kind {
            DIFF_CONTEXT => DiffLine::context(
                DiffLineNumber::new(required_u64(&line, "oldLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("oldLine must be >= 1"))?,
                DiffLineNumber::new(required_u64(&line, "newLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("newLine must be >= 1"))?,
                text,
            ),
            DIFF_ADDITION => DiffLine::addition(
                DiffLineNumber::new(required_u64(&line, "newLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("newLine must be >= 1"))?,
                text,
            ),
            DIFF_DELETION => DiffLine::deletion(
                DiffLineNumber::new(required_u64(&line, "oldLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("oldLine must be >= 1"))?,
                text,
            ),
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown diff line kind {other}"
                )));
            }
        };
        lowered.push(lowered_line.with_termination(termination));
    }
    DiffHunk::new(old_range, new_range, lowered)
        .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

fn decode_diff_range(value: &Object<'_>) -> Result<iyon_tui::DiffRange> {
    use iyon_tui::{DiffLineOffset, DiffRange};
    DiffRange::new(
        DiffLineOffset::new(required_u64(value, "start")?),
        required_u64(value, "count")?,
    )
    .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

fn decode_grid_track(value: &Object<'_>) -> Result<GridTrack> {
    match required_prop::<u32>(value, "kind")? {
        GRID_TRACK_CONTENT => Ok(GridTrack::content()),
        GRID_TRACK_CONTENT_MAX => Ok(GridTrack::content_max(required_u16(value, "max")?)),
        GRID_TRACK_FIXED => Ok(GridTrack::fixed(required_u16(value, "size")?)),
        GRID_TRACK_FLEX => Ok(GridTrack::flex()),
        GRID_TRACK_FLEX_MAX => Ok(GridTrack::flex_max(required_u16(value, "max")?)),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown grid track kind {other}"
        ))),
    }
}

fn decode_overflow(value: Option<&Object<'_>>) -> Result<iyon_tui::OverflowIndicator> {
    let Some(value) = value else {
        return Ok(iyon_tui::OverflowIndicator::None);
    };
    match required_prop::<u32>(value, "kind")? {
        OVERFLOW_NONE => Ok(iyon_tui::OverflowIndicator::None),
        OVERFLOW_ELLIPSIS => Ok(iyon_tui::OverflowIndicator::Ellipsis {
            style: decode_style_ref(&required_prop::<Object>(value, "style")?)?,
        }),
        OVERFLOW_FOOTER => Ok(iyon_tui::OverflowIndicator::Footer {
            prefix: required_prop::<String>(value, "prefix")?,
            style: decode_style_ref(&required_prop::<Object>(value, "style")?)?,
        }),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown overflow indicator kind {other}"
        ))),
    }
}

fn decode_text_span(value: &Object<'_>) -> Result<TextSpan> {
    let text = required_prop::<String>(value, "text")?;
    tui_perf_add!(NapiViewStringBytesCopied, text.len());
    let style = value
        .get::<Object>("style")?
        .map(|style| decode_style_ref(&style))
        .transpose()?
        .unwrap_or_else(|| StyleRef::direct(StyleSpec::new()));
    Ok(TextSpan::styled(text, style))
}

fn decode_style_ref(value: &Object<'_>) -> Result<StyleRef> {
    let style = decode_style_spec(value)?;
    match value.get::<String>("theme")? {
        Some(theme) => Ok(StyleRef::themed(theme, style)),
        None => Ok(StyleRef::direct(style)),
    }
}

fn decode_style_spec(value: &Object<'_>) -> Result<StyleSpec> {
    let mut style = StyleSpec::new();
    if let Some(color) = value.get::<Unknown>("foreground")? {
        style = style.foreground(decode_color(&color)?);
    }
    if let Some(color) = value.get::<Unknown>("background")? {
        style = style.background(decode_color(&color)?);
    }
    if let Some(attributes) = value.get::<Object>("attributes")? {
        for name in Object::keys(&attributes)? {
            let enabled = required_prop::<bool>(&attributes, &name)?;
            let attribute = text_attribute(&name).ok_or_else(|| {
                crate::NativeError::invalid_input(format!("unknown text attribute `{name}`"))
            })?;
            style = style.attribute(attribute, enabled);
        }
    }
    Ok(style)
}

fn decode_color(value: &Unknown<'_>) -> Result<iyon_tui::ColorSpec> {
    match value.get_type()? {
        ValueType::String => {
            let value = unsafe { value.cast::<String>()? };
            decode_color_string(&value)
        }
        ValueType::Object => {
            let object = unsafe { value.cast::<Object>()? };
            if object.get::<String>("type")?.as_deref() != Some("ansi") {
                return Err(crate::NativeError::invalid_input(
                    "unknown color object type",
                ));
            }
            let number = required_u64(&object, "value")?;
            Ok(iyon_tui::ColorSpec::ansi(u8::try_from(number).map_err(
                |_| crate::NativeError::invalid_input("ANSI color value must fit in u8"),
            )?))
        }
        _ => Err(crate::NativeError::invalid_input(
            "color must be a string or ANSI color object",
        )),
    }
}

fn decode_color_string(value: &str) -> Result<iyon_tui::ColorSpec> {
    if let Some(value) = value.strip_prefix("theme:") {
        return Ok(iyon_tui::ColorSpec::theme(value));
    }
    if let Some(value) = value.strip_prefix("ansi:") {
        return Ok(iyon_tui::ColorSpec::ansi(value.parse::<u8>().map_err(
            |_| crate::NativeError::invalid_input("ANSI color must fit in u8"),
        )?));
    }
    if let Some(value) = value.strip_prefix('#') {
        if value.len() == 6 {
            let r = u8::from_str_radix(&value[0..2], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            let g = u8::from_str_radix(&value[2..4], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            let b = u8::from_str_radix(&value[4..6], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            return Ok(iyon_tui::ColorSpec::rgb(r, g, b));
        }
    }
    let color = match value.to_ascii_lowercase().as_str() {
        "black" => iyon_tui::AnsiColor::Black,
        "red" => iyon_tui::AnsiColor::Red,
        "green" => iyon_tui::AnsiColor::Green,
        "yellow" => iyon_tui::AnsiColor::Yellow,
        "blue" => iyon_tui::AnsiColor::Blue,
        "magenta" => iyon_tui::AnsiColor::Magenta,
        "cyan" => iyon_tui::AnsiColor::Cyan,
        "gray" => iyon_tui::AnsiColor::Gray,
        "darkgray" => iyon_tui::AnsiColor::DarkGray,
        "lightred" => iyon_tui::AnsiColor::LightRed,
        "lightgreen" => iyon_tui::AnsiColor::LightGreen,
        "lightyellow" => iyon_tui::AnsiColor::LightYellow,
        "lightblue" => iyon_tui::AnsiColor::LightBlue,
        "lightmagenta" => iyon_tui::AnsiColor::LightMagenta,
        "lightcyan" => iyon_tui::AnsiColor::LightCyan,
        "white" => iyon_tui::AnsiColor::White,
        _ => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown color `{value}`"
            )));
        }
    };
    Ok(iyon_tui::ColorSpec::named(color))
}

fn decode_decoration(mut view: View, decoration: &Object<'_>) -> Result<View> {
    if let Some(value) = decoration.get::<Object>("padding")? {
        view = view.padding(iyon_tui::Insets::new(
            required_u16(&value, "top")?,
            required_u16(&value, "right")?,
            required_u16(&value, "bottom")?,
            required_u16(&value, "left")?,
        ));
    }
    if let Some(value) = decoration.get::<Unknown>("background")? {
        view = view.background(decode_color(&value)?);
    }
    if let Some(value) = decoration.get::<Unknown>("foreground")? {
        view = view.foreground(decode_color(&value)?);
    }
    if let Some(value) = decoration.get::<Object>("border")? {
        view = view.border(decode_border(&value)?);
    }
    if let Some(value) = decoration.get::<Object>("style")? {
        view = view.style(decode_style_ref(&value)?);
    }
    if let Some(states) = decoration.get::<Object>("styleStates")? {
        for key in Object::keys(&states)? {
            view = view.style_state(key.clone(), required_prop::<String>(&states, &key)?);
        }
    }
    match decoration.get::<String>("width")?.as_deref() {
        Some("fit") => view = view.fit_width(),
        Some("fill") => view = view.fill_width(),
        Some(other) => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown width rule `{other}`"
            )));
        }
        None => {}
    }
    match decoration.get::<String>("height")?.as_deref() {
        Some("fit") => view = view.fit_height(),
        Some("fill") => view = view.fill_height(),
        Some(other) => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown height rule `{other}`"
            )));
        }
        None => {}
    }
    if decoration.get::<f64>("minWidth")?.is_some() {
        view = view.min_width(required_u16(decoration, "minWidth")?);
    }
    if decoration.get::<f64>("maxWidth")?.is_some() {
        view = view.max_width(required_u16(decoration, "maxWidth")?);
    }
    if decoration.get::<f64>("minHeight")?.is_some() {
        view = view.min_height(required_u16(decoration, "minHeight")?);
    }
    if decoration.get::<f64>("maxHeight")?.is_some() {
        view = view.max_height(required_u16(decoration, "maxHeight")?);
    }
    Ok(view)
}

fn decode_border(value: &Object<'_>) -> Result<BorderSpec> {
    let mut spec = match value.get::<String>("style")?.as_deref().unwrap_or("plain") {
        "plain" => BorderSpec::plain(),
        "rounded" => BorderSpec::rounded(),
        "double" => BorderSpec::double(),
        other => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown border style `{other}`"
            )));
        }
    };
    let top_bottom = value.get::<String>("edges")?.as_deref() == Some("topBottom");
    let color = value
        .get::<Unknown>("color")?
        .map(|value| decode_color(&value))
        .transpose()?;
    if let Some(glyphs) = value.get::<Object>("glyphs")? {
        let fields = [
            "top",
            "right",
            "bottom",
            "left",
            "topLeft",
            "topRight",
            "bottomLeft",
            "bottomRight",
        ];
        let values = fields
            .iter()
            .map(|field| required_prop::<String>(&glyphs, field))
            .collect::<Result<Vec<_>>>()?;
        spec = BorderSpec::custom(
            BorderGlyphs::new(
                values[0].as_str(),
                values[1].as_str(),
                values[2].as_str(),
                values[3].as_str(),
                values[4].as_str(),
                values[5].as_str(),
                values[6].as_str(),
                values[7].as_str(),
            )
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?,
        );
    }
    if top_bottom {
        spec = spec.edges(BorderEdges::TOP_BOTTOM);
    }
    if let Some(color) = color {
        spec = spec.color(color);
    }
    Ok(spec)
}

#[cfg(test)]
fn lower_view(value: &Value) -> Result<View> {
    tui_perf_inc!(NapiViewNodesSeen);
    tui_perf_inc!(NapiViewCacheMisses);
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("view node must be an object"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::NativeError::invalid_input("view node type must be a string"))?;
    let view = match kind {
        "text" => {
            let spans = object
                .get("spans")
                .and_then(Value::as_array)
                .ok_or_else(|| crate::NativeError::invalid_input("text spans must be an array"))?;
            let spans = spans
                .iter()
                .map(lower_text_span)
                .collect::<Result<Vec<_>>>()?;
            let text = View::styled_text(spans);
            let text = match object
                .get("wrap")
                .and_then(Value::as_str)
                .unwrap_or("wordThenGrapheme")
            {
                "wordThenGrapheme" => text.wrap(WrapMode::WordThenGrapheme),
                "grapheme" => text.wrap(WrapMode::Grapheme),
                "noWrap" => text.wrap(WrapMode::NoWrap),
                other => {
                    return Err(crate::NativeError::invalid_input(format!(
                        "unknown wrap mode `{other}`"
                    )));
                }
            };
            let text = match object
                .get("align")
                .and_then(Value::as_str)
                .unwrap_or("start")
            {
                "start" => text.text_align(HorizontalAlign::Start),
                "center" => text.text_align(HorizontalAlign::Center),
                "end" => text.text_align(HorizontalAlign::End),
                other => {
                    return Err(crate::NativeError::invalid_input(format!(
                        "unknown text alignment `{other}`"
                    )));
                }
            };
            text.into_view()
        }
        "diff" => lower_diff(object)?,
        "spacer" => {
            let rows = u16_value(object, "rows")?;
            View::spacer(rows)
        }
        "row" => lower_axis(object, true)?,
        "column" => lower_axis(object, false)?,
        "hanging" => View::hanging(
            lower_required(object, "prefix")?,
            lower_required(object, "continuation")?,
            lower_required(object, "body")?,
        ),
        "grid" => lower_grid(object)?,
        "container" => lower_required(object, "child")?.container(),
        "clamp" => lower_required(object, "child")?.clamp_rows(
            u16_value(object, "maxRows")?,
            lower_overflow(object.get("overflow"))?,
        ),
        "decorated" => {
            apply_decoration(lower_required(object, "child")?, object.get("decoration"))?
        }
        "component" => {
            View::native_component(object.get("handle").and_then(Value::as_u64).ok_or_else(
                || crate::NativeError::invalid_input("component handle must be an integer"),
            )?)
        }
        "contentMax" => lower_required(object, "child")?.clamp_rows(
            u16_value(object, "maxRows")?,
            iyon_tui::OverflowIndicator::None,
        ),
        other => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown view node type `{other}`"
            )));
        }
    };
    Ok(view)
}

#[cfg(test)]
fn lower_diff(object: &Map<String, Value>) -> Result<View> {
    let hunks = object
        .get("hunks")
        .and_then(Value::as_array)
        .ok_or_else(|| crate::NativeError::invalid_input("diff hunks must be an array"))?;
    let mut lowered = Vec::with_capacity(hunks.len());
    for hunk in hunks {
        lowered.push(lower_diff_hunk(hunk)?);
    }
    Ok(iyon_tui::DiffRenderer::new().render(lowered.as_slice()))
}

#[cfg(test)]
fn lower_diff_hunk(value: &Value) -> Result<iyon_tui::DiffHunk> {
    use iyon_tui::{DiffHunk, DiffLine, DiffLineNumber, DiffLineTermination};

    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("diff hunk must be an object"))?;
    let old_range = lower_diff_range(
        object
            .get("oldRange")
            .ok_or_else(|| crate::NativeError::invalid_input("diff hunk oldRange is required"))?,
    )?;
    let new_range = lower_diff_range(
        object
            .get("newRange")
            .ok_or_else(|| crate::NativeError::invalid_input("diff hunk newRange is required"))?,
    )?;
    let lines = object
        .get("lines")
        .and_then(Value::as_array)
        .ok_or_else(|| crate::NativeError::invalid_input("diff hunk lines must be an array"))?;
    let mut lowered = Vec::with_capacity(lines.len());
    for line in lines {
        let line = line
            .as_object()
            .ok_or_else(|| crate::NativeError::invalid_input("diff line must be an object"))?;
        let kind = line
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| crate::NativeError::invalid_input("diff line kind is required"))?;
        let text = line
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| crate::NativeError::invalid_input("diff line text is required"))?;
        let termination = match line
            .get("termination")
            .and_then(Value::as_str)
            .unwrap_or("terminated")
        {
            "terminated" => DiffLineTermination::Terminated,
            "unterminated" => DiffLineTermination::Unterminated,
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown diff line termination `{other}`"
                )));
            }
        };
        let lowered_line = match kind {
            "context" => {
                let old = DiffLineNumber::new(u64_value(line, "oldLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("oldLine must be >= 1"))?;
                let new = DiffLineNumber::new(u64_value(line, "newLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("newLine must be >= 1"))?;
                DiffLine::context(old, new, text)
            }
            "addition" => {
                let new = DiffLineNumber::new(u64_value(line, "newLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("newLine must be >= 1"))?;
                DiffLine::addition(new, text)
            }
            "deletion" => {
                let old = DiffLineNumber::new(u64_value(line, "oldLine")?)
                    .ok_or_else(|| crate::NativeError::invalid_input("oldLine must be >= 1"))?;
                DiffLine::deletion(old, text)
            }
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown diff line kind `{other}`"
                )));
            }
        };
        lowered.push(lowered_line.with_termination(termination));
    }
    DiffHunk::new(old_range, new_range, lowered)
        .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

#[cfg(test)]
fn lower_diff_range(value: &Value) -> Result<iyon_tui::DiffRange> {
    use iyon_tui::{DiffLineOffset, DiffRange};
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("diff range must be an object"))?;
    DiffRange::new(
        DiffLineOffset::new(u64_value(object, "start")?),
        u64_value(object, "count")?,
    )
    .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
}

#[cfg(test)]
fn u64_value(object: &Map<String, Value>, field: &str) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| crate::NativeError::invalid_input(format!("{field} must be a u64 integer")))
}

#[cfg(test)]
fn lower_axis(object: &Map<String, Value>, horizontal: bool) -> Result<View> {
    let gap = u16_value(object, "gap")?;
    let children = object
        .get("children")
        .and_then(Value::as_array)
        .ok_or_else(|| crate::NativeError::invalid_input("view children must be an array"))?;
    let mut lowered = Vec::with_capacity(children.len());
    for child in children {
        let child = child
            .as_object()
            .ok_or_else(|| crate::NativeError::invalid_input("layout child must be an object"))?;
        let kind = child.get("kind").and_then(Value::as_str).ok_or_else(|| {
            crate::NativeError::invalid_input("layout child kind must be a string")
        })?;
        let view =
            lower_view(child.get("child").ok_or_else(|| {
                crate::NativeError::invalid_input("layout child view is required")
            })?)?;
        let size = child
            .get("size")
            .map(|_| u16_value(child, "size"))
            .transpose()?;
        let max_rows = child
            .get("maxRows")
            .map(|_| u16_value(child, "maxRows"))
            .transpose()?;
        match kind {
            "normal" | "flex" => {}
            "fixed" if size.is_some() => {}
            "fixed" => {
                return Err(crate::NativeError::invalid_input(
                    "fixed layout child size is required",
                ));
            }
            "contentMax" if !horizontal && max_rows.is_some() => {}
            "contentMax" if !horizontal => {
                return Err(crate::NativeError::invalid_input(
                    "contentMax maxRows is required",
                ));
            }
            "contentMax" => {
                return Err(crate::NativeError::invalid_input(
                    "contentMax is only valid for vertical children",
                ));
            }
            "flexMax" if !horizontal && max_rows.is_some() => {}
            "flexMax" if !horizontal => {
                return Err(crate::NativeError::invalid_input(
                    "flexMax maxRows is required",
                ));
            }
            "flexMax" => {
                return Err(crate::NativeError::invalid_input(
                    "flexMax is only valid for vertical children",
                ));
            }
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown layout child kind `{other}`"
                )));
            }
        }
        lowered.push((kind.to_owned(), size, max_rows, view));
    }
    if horizontal {
        Ok(View::horizontal(|row| {
            row.gap(gap);
            for (kind, size, _max_rows, view) in lowered {
                match kind.as_str() {
                    "normal" => {
                        row.child(view);
                    }
                    "fixed" => {
                        row.fixed(size.expect("fixed size was validated"), view);
                    }
                    "flex" => {
                        row.flex(view);
                    }
                    "contentMax" => unreachable!("contentMax was rejected for horizontal layout"),
                    "flexMax" => unreachable!("flexMax was rejected for horizontal layout"),
                    _ => unreachable!("layout child kind was validated"),
                }
            }
        }))
    } else {
        Ok(View::vertical(|column| {
            column.gap(gap);
            for (kind, size, max_rows, view) in lowered {
                match kind.as_str() {
                    "normal" => {
                        column.child(view);
                    }
                    "fixed" => {
                        column.fixed(size.expect("fixed size was validated"), view);
                    }
                    "flex" => {
                        column.flex(view);
                    }
                    "contentMax" => {
                        column.content_max(max_rows.expect("validated content max"), view);
                    }
                    "flexMax" => {
                        column.flex_max(max_rows.expect("validated flex max"), view);
                    }
                    _ => unreachable!("layout child kind was validated"),
                }
            }
        }))
    }
}

#[cfg(test)]
fn lower_grid(object: &Map<String, Value>) -> Result<View> {
    let columns = object
        .get("columns")
        .and_then(Value::as_array)
        .ok_or_else(|| crate::NativeError::invalid_input("grid columns must be an array"))?
        .iter()
        .map(lower_grid_track)
        .collect::<Result<Vec<_>>>()?;
    let rows = object
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| crate::NativeError::invalid_input("grid rows must be an array"))?;
    let column_gap = u16_value(object, "columnGap")?;
    let row_gap = u16_value(object, "rowGap")?;
    let mut lowered_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| crate::NativeError::invalid_input("grid row must be an object"))?;
        let track = lower_grid_track(
            row.get("track")
                .ok_or_else(|| crate::NativeError::invalid_input("grid row track is required"))?,
        )?;
        let cells = row
            .get("cells")
            .and_then(Value::as_array)
            .ok_or_else(|| crate::NativeError::invalid_input("grid cells must be an array"))?;
        let mut lowered_cells = Vec::with_capacity(cells.len());
        for cell in cells {
            let cell = cell
                .as_object()
                .ok_or_else(|| crate::NativeError::invalid_input("grid cell must be an object"))?;
            let spec = GridCellSpec::new()
                .column_span(u16_value(cell, "columnSpan")?)
                .row_span(u16_value(cell, "rowSpan")?)
                .horizontal_align(parse_horizontal_align(
                    cell.get("horizontalAlign")
                        .and_then(Value::as_str)
                        .unwrap_or("start"),
                )?)
                .vertical_align(parse_vertical_align(
                    cell.get("verticalAlign")
                        .and_then(Value::as_str)
                        .unwrap_or("top"),
                )?);
            lowered_cells.push((spec, lower_required(cell, "view")?));
        }
        lowered_rows.push((track, lowered_cells));
    }
    Ok(View::grid(|grid| {
        grid.columns(columns);
        grid.column_gap(column_gap);
        grid.row_gap(row_gap);
        for (track, cells) in lowered_rows {
            grid.row_with(track, |row| {
                for (spec, view) in &cells {
                    row.cell_with(*spec, view.clone());
                }
            });
        }
    }))
}

#[cfg(test)]
fn lower_grid_track(value: &Value) -> Result<GridTrack> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("grid track must be an object"))?;
    match object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::NativeError::invalid_input("grid track kind must be a string"))?
    {
        "content" => Ok(GridTrack::content()),
        "contentMax" => Ok(GridTrack::content_max(u16_value(object, "max")?)),
        "fixed" => Ok(GridTrack::fixed(u16_value(object, "size")?)),
        "flex" => Ok(GridTrack::flex()),
        "flexMax" => Ok(GridTrack::flex_max(u16_value(object, "max")?)),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown grid track kind `{other}`"
        ))),
    }
}

#[cfg(test)]
fn lower_overflow(value: Option<&Value>) -> Result<iyon_tui::OverflowIndicator> {
    let Some(value) = value else {
        return Ok(iyon_tui::OverflowIndicator::None);
    };
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("overflow indicator must be an object"))?;
    let kind = object.get("kind").and_then(Value::as_str).ok_or_else(|| {
        crate::NativeError::invalid_input("overflow indicator kind must be a string")
    })?;
    match kind {
        "none" => Ok(iyon_tui::OverflowIndicator::None),
        "ellipsis" => {
            Ok(iyon_tui::OverflowIndicator::Ellipsis {
                style: lower_style_ref(object.get("style").ok_or_else(|| {
                    crate::NativeError::invalid_input("ellipsis style is required")
                })?)?,
            })
        }
        "footer" => {
            let prefix = object
                .get("prefix")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    crate::NativeError::invalid_input("footer prefix must be a string")
                })?;
            tui_perf_add!(NapiViewStringBytesCopied, prefix.len());
            Ok(iyon_tui::OverflowIndicator::Footer {
                prefix: prefix.to_owned(),
                style: lower_style_ref(object.get("style").ok_or_else(|| {
                    crate::NativeError::invalid_input("footer style is required")
                })?)?,
            })
        }
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown overflow indicator `{other}`"
        ))),
    }
}

#[cfg(test)]
fn parse_horizontal_align(value: &str) -> Result<HorizontalAlign> {
    match value {
        "start" => Ok(HorizontalAlign::Start),
        "center" => Ok(HorizontalAlign::Center),
        "end" => Ok(HorizontalAlign::End),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown horizontal alignment `{other}`"
        ))),
    }
}

#[cfg(test)]
fn parse_vertical_align(value: &str) -> Result<VerticalAlign> {
    match value {
        "top" => Ok(VerticalAlign::Top),
        "center" => Ok(VerticalAlign::Center),
        "bottom" => Ok(VerticalAlign::Bottom),
        other => Err(crate::NativeError::invalid_input(format!(
            "unknown vertical alignment `{other}`"
        ))),
    }
}

#[cfg(test)]
fn lower_text_span(value: &Value) -> Result<TextSpan> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("text span must be an object"))?;
    let text = object
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| crate::NativeError::invalid_input("text span text must be a string"))?;
    tui_perf_add!(NapiViewStringBytesCopied, text.len());
    let style = object
        .get("style")
        .map(lower_style_ref)
        .transpose()?
        .unwrap_or_else(|| StyleRef::direct(StyleSpec::new()));
    Ok(TextSpan::styled(text, style))
}

#[cfg(test)]
fn lower_style_ref(value: &Value) -> Result<StyleRef> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("style must be an object"))?;
    let style = lower_style_spec(value)?;
    Ok(match object.get("theme").and_then(Value::as_str) {
        Some(theme) => StyleRef::themed(theme, style),
        None => StyleRef::direct(style),
    })
}

fn lower_style_spec(value: &Value) -> Result<StyleSpec> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("style must be an object"))?;
    let mut style = StyleSpec::new();
    if let Some(color) = object.get("foreground") {
        style = style.foreground(color_spec(color)?);
    }
    if let Some(color) = object.get("background") {
        style = style.background(color_spec(color)?);
    }
    if let Some(attributes) = object.get("attributes").and_then(Value::as_object) {
        for (name, enabled) in attributes {
            let attribute = text_attribute(name).ok_or_else(|| {
                crate::NativeError::invalid_input(format!("unknown text attribute `{name}`"))
            })?;
            let enabled = enabled.as_bool().ok_or_else(|| {
                crate::NativeError::invalid_input("text attributes must be booleans")
            })?;
            style = style.attribute(attribute, enabled);
        }
    }
    Ok(style)
}

#[cfg(test)]
fn lower_required(object: &Map<String, Value>, field: &str) -> Result<View> {
    lower_view(object.get(field).ok_or_else(|| {
        crate::NativeError::invalid_input(format!("view node field `{field}` is required"))
    })?)
}

fn u16_value(object: &Map<String, Value>, field: &str) -> Result<u16> {
    let value = object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| crate::NativeError::invalid_input(format!("{field} must be an integer")))?;
    u16::try_from(value)
        .map_err(|_| crate::NativeError::invalid_input(format!("{field} must fit in u16")))
}

fn cell_style_value(style: HostCellStyle) -> Value {
    serde_json::json!({
        "foreground": style.foreground,
        "background": style.background,
        "bold": style.bold,
        "dim": style.dim,
        "italic": style.italic,
        "underline": style.underline,
        "reversed": style.reversed,
        "strikethrough": style.strikethrough,
    })
}

fn lower_theme(value: &Value) -> Result<iyon_tui::Theme> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("theme must be an object"))?;
    let mut theme = iyon_tui::Theme::new();
    if let Some(colors) = object.get("colors").and_then(Value::as_object) {
        for (key, entry) in colors {
            let entry = entry.as_object().ok_or_else(|| {
                crate::NativeError::invalid_input("theme color entry must be an object")
            })?;
            if let Some(base) = entry.get("base") {
                theme.set_color(key.as_str(), lower_theme_color(base)?);
            }
            for variant in entry
                .get("variants")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let variant = variant.as_object().ok_or_else(|| {
                    crate::NativeError::invalid_input("theme color variant must be an object")
                })?;
                theme.set_color_variant(
                    key.as_str(),
                    lower_selector(variant.get("selector").ok_or_else(|| {
                        crate::NativeError::invalid_input("theme color selector is required")
                    })?)?,
                    lower_theme_color(variant.get("value").ok_or_else(|| {
                        crate::NativeError::invalid_input("theme color value is required")
                    })?)?,
                );
            }
        }
    }
    if let Some(styles) = object.get("styles").and_then(Value::as_object) {
        for (key, entry) in styles {
            let entry = entry.as_object().ok_or_else(|| {
                crate::NativeError::invalid_input("theme style entry must be an object")
            })?;
            if let Some(base) = entry.get("base") {
                theme.set_style(key.as_str(), lower_style_spec(base)?);
            }
            for variant in entry
                .get("variants")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let variant = variant.as_object().ok_or_else(|| {
                    crate::NativeError::invalid_input("theme style variant must be an object")
                })?;
                theme.set_style_variant(
                    key.as_str(),
                    lower_selector(variant.get("selector").ok_or_else(|| {
                        crate::NativeError::invalid_input("theme style selector is required")
                    })?)?,
                    lower_style_spec(variant.get("value").ok_or_else(|| {
                        crate::NativeError::invalid_input("theme style value is required")
                    })?)?,
                );
            }
        }
    }
    if let Some(text_styles) = object.get("textStyles").and_then(Value::as_array) {
        for entry in text_styles {
            let entry = entry.as_object().ok_or_else(|| {
                crate::NativeError::invalid_input("theme text style entry must be an object")
            })?;
            let selector = lower_text_selector(entry.get("selector").ok_or_else(|| {
                crate::NativeError::invalid_input("theme text style selector is required")
            })?)?;
            let style = lower_style_spec(entry.get("value").ok_or_else(|| {
                crate::NativeError::invalid_input("theme text style value is required")
            })?)?;
            theme.set_text_style(selector, style);
        }
    }
    Ok(theme)
}

fn lower_text_selector(value: &Value) -> Result<TextSelector> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("text selector must be an object"))?;
    let mut selector = TextSelector::any();
    if let Some(roles) = object.get("roles").and_then(Value::as_array) {
        for role in roles {
            selector = selector.and_role(lower_text_role(role.as_str().ok_or_else(|| {
                crate::NativeError::invalid_input("text selector role must be a string")
            })?)?);
        }
    }
    if let Some(parts) = object.get("parts").and_then(Value::as_array) {
        for part in parts {
            selector = selector.and_part(lower_text_part(part.as_str().ok_or_else(|| {
                crate::NativeError::invalid_input("text selector part must be a string")
            })?)?);
        }
    }
    if let Some(annotations) = object.get("annotations").and_then(Value::as_array) {
        for annotation in annotations {
            let annotation = annotation.as_object().ok_or_else(|| {
                crate::NativeError::invalid_input("text selector annotation must be an object")
            })?;
            let namespace = annotation
                .get("namespace")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    crate::NativeError::invalid_input(
                        "text selector annotation namespace is required",
                    )
                })?;
            let name = annotation
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    crate::NativeError::invalid_input("text selector annotation name is required")
                })?;
            let tag = SemanticTag::new(namespace, name)
                .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
            selector = selector.and_annotation(&tag);
        }
    }
    if let Some(language) = object.get("language").and_then(Value::as_str) {
        let language = LanguageId::new(language)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        selector = selector.language(&language);
    }
    if let Some(origin) = object.get("origin").and_then(Value::as_str) {
        let origin = TextOrigin::new(origin)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        selector = selector.origin(origin);
    }
    if let Some(format) = object.get("format").and_then(Value::as_str) {
        let format = FormatId::new(format)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        selector = selector.format(&format);
    }
    if object
        .get("focused")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        selector = selector.and_focused();
    }
    if object
        .get("focusWithin")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        selector = selector.and_focus_within();
    }
    if let Some(states) = object.get("states").and_then(Value::as_object) {
        for (key, value) in states {
            selector = selector.and_state(
                key.clone(),
                value.as_str().ok_or_else(|| {
                    crate::NativeError::invalid_input("text selector states must be strings")
                })?,
            );
        }
    }
    Ok(selector)
}

fn lower_text_role(value: &str) -> Result<TextRole> {
    let role = match value {
        "paragraph" => TextRole::Paragraph,
        "heading" => TextRole::Heading,
        "blockQuote" => TextRole::BlockQuote,
        "list" => TextRole::List,
        "listItem" => TextRole::ListItem,
        "codeBlock" => TextRole::CodeBlock,
        "table" => TextRole::Table,
        "tableRow" => TextRole::TableRow,
        "tableCell" => TextRole::TableCell,
        "thematicBreak" => TextRole::ThematicBreak,
        "rawBlock" => TextRole::RawBlock,
        "container" => TextRole::Container,
        "strong" => TextRole::Strong,
        "emphasis" => TextRole::Emphasis,
        "strikethrough" => TextRole::Strikethrough,
        "underline" => TextRole::Underline,
        "superscript" => TextRole::Superscript,
        "subscript" => TextRole::Subscript,
        "smallCaps" => TextRole::SmallCaps,
        "inlineCode" => TextRole::InlineCode,
        "link" => TextRole::Link,
        "image" => TextRole::Image,
        "rawInline" => TextRole::RawInline,
        _ => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown text selector role `{value}`"
            )));
        }
    };
    Ok(role)
}

fn lower_text_part(value: &str) -> Result<TextPart> {
    let part = match value {
        "listMarker" => TextPart::ListMarker,
        "taskMarker" => TextPart::TaskMarker,
        "quoteMarker" => TextPart::QuoteMarker,
        "codeLabel" => TextPart::CodeLabel,
        "tableRule" => TextPart::TableRule,
        "thematicRule" => TextPart::ThematicRule,
        "imageFallback" => TextPart::ImageFallback,
        _ => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown text selector part `{value}`"
            )));
        }
    };
    Ok(part)
}

fn lower_selector(value: &Value) -> Result<iyon_tui::StyleSelector> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("theme selector must be an object"))?;
    let mut selector = iyon_tui::StyleSelector::default();
    if object
        .get("focused")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        selector = selector.and_focused();
    }
    if object
        .get("focusWithin")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        selector = selector.and_focus_within();
    }
    if let Some(states) = object.get("states").and_then(Value::as_object) {
        for (key, value) in states {
            selector = selector.and_state(
                key.clone(),
                value.as_str().ok_or_else(|| {
                    crate::NativeError::invalid_input("theme selector states must be strings")
                })?,
            );
        }
    }
    Ok(selector)
}

fn lower_theme_color(value: &Value) -> Result<iyon_tui::ThemeColor> {
    match color_spec(value)? {
        iyon_tui::ColorSpec::Theme(_) => Err(crate::NativeError::invalid_input(
            "theme colors cannot reference another theme color",
        )),
        iyon_tui::ColorSpec::Named(color) => Ok(iyon_tui::ThemeColor::Named(color)),
        iyon_tui::ColorSpec::Ansi(value) => Ok(iyon_tui::ThemeColor::Indexed(value)),
        iyon_tui::ColorSpec::Rgb { r, g, b } => Ok(iyon_tui::ThemeColor::Rgb { r, g, b }),
    }
}

fn lower_border(value: &Value) -> Result<BorderSpec> {
    let border = value
        .as_object()
        .ok_or_else(|| crate::NativeError::invalid_input("border must be an object"))?;
    let mut spec = match border
        .get("style")
        .and_then(Value::as_str)
        .unwrap_or("plain")
    {
        "plain" => BorderSpec::plain(),
        "rounded" => BorderSpec::rounded(),
        "double" => BorderSpec::double(),
        other => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown border style `{other}`"
            )));
        }
    };
    if border.get("edges").and_then(Value::as_str) == Some("topBottom") {
        spec = spec.edges(BorderEdges::TOP_BOTTOM);
    }
    if let Some(color) = border.get("color") {
        spec = spec.color(color_spec(color)?);
    }
    if let Some(glyphs) = border.get("glyphs").and_then(Value::as_object) {
        let fields = [
            "top",
            "right",
            "bottom",
            "left",
            "topLeft",
            "topRight",
            "bottomLeft",
            "bottomRight",
        ];
        let values = fields
            .iter()
            .map(|field| {
                glyphs.get(*field).and_then(Value::as_str).ok_or_else(|| {
                    crate::NativeError::invalid_input(format!(
                        "border glyph `{field}` must be a string"
                    ))
                })
            })
            .collect::<Result<Vec<_>>>()?;
        spec = BorderSpec::custom(
            BorderGlyphs::new(
                values[0], values[1], values[2], values[3], values[4], values[5], values[6],
                values[7],
            )
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?,
        );
        if border.get("edges").and_then(Value::as_str) == Some("topBottom") {
            spec = spec.edges(BorderEdges::TOP_BOTTOM);
        }
        if let Some(color) = border.get("color") {
            spec = spec.color(color_spec(color)?);
        }
    }
    Ok(spec)
}

#[cfg(test)]
fn apply_decoration(view: View, decoration: Option<&Value>) -> Result<View> {
    let Some(decoration) = decoration.and_then(Value::as_object) else {
        return Ok(view);
    };
    let mut view = view;
    if let Some(value) = decoration.get("padding") {
        let padding = value
            .as_object()
            .ok_or_else(|| crate::NativeError::invalid_input("padding must be an object"))?;
        view = view.padding(iyon_tui::Insets::new(
            u16_value(padding, "top")?,
            u16_value(padding, "right")?,
            u16_value(padding, "bottom")?,
            u16_value(padding, "left")?,
        ));
    }
    if let Some(color) = decoration.get("background") {
        view = view.background(color_spec(color)?);
    }
    if let Some(color) = decoration.get("foreground") {
        view = view.foreground(color_spec(color)?);
    }
    if let Some(border) = decoration.get("border").and_then(Value::as_object) {
        view = view.border(lower_border(&Value::Object(border.clone()))?);
    }
    if let Some(style) = decoration.get("style").and_then(Value::as_object) {
        view = view.style(lower_style_ref(&Value::Object(style.clone()))?);
        if let Some(attributes) = style.get("attributes").and_then(Value::as_object) {
            for (name, enabled) in attributes {
                if let Some(attribute) = text_attribute(name) {
                    view = view.text_attribute(
                        attribute,
                        enabled.as_bool().ok_or_else(|| {
                            crate::NativeError::invalid_input("text attributes must be booleans")
                        })?,
                    );
                }
            }
        }
    }
    if let Some(states) = decoration.get("styleStates").and_then(Value::as_object) {
        for (key, value) in states {
            let value = value.as_str().ok_or_else(|| {
                crate::NativeError::invalid_input("style state values must be strings")
            })?;
            view = view.style_state(key.as_str(), value);
        }
    }
    match decoration.get("width").and_then(Value::as_str) {
        Some("fit") => view = view.fit_width(),
        Some("fill") => view = view.fill_width(),
        Some(other) => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown width rule `{other}`"
            )));
        }
        None => {}
    }
    match decoration.get("height").and_then(Value::as_str) {
        Some("fit") => view = view.fit_height(),
        Some("fill") => view = view.fill_height(),
        Some(other) => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown height rule `{other}`"
            )));
        }
        None => {}
    }
    if let Some(value) = decoration.get("minWidth") {
        view = view.min_width(u16_value(decoration, "minWidth")?);
        let _ = value;
    }
    if decoration.get("maxWidth").is_some() {
        view = view.max_width(u16_value(decoration, "maxWidth")?);
    }
    if decoration.get("minHeight").is_some() {
        view = view.min_height(u16_value(decoration, "minHeight")?);
    }
    if decoration.get("maxHeight").is_some() {
        view = view.max_height(u16_value(decoration, "maxHeight")?);
    }
    Ok(view)
}

fn color_spec(value: &Value) -> Result<iyon_tui::ColorSpec> {
    if let Some(object) = value.as_object() {
        let kind = object.get("type").and_then(Value::as_str).ok_or_else(|| {
            crate::NativeError::invalid_input("color object type must be a string")
        })?;
        if kind == "ansi" {
            let number = object.get("value").and_then(Value::as_u64).ok_or_else(|| {
                crate::NativeError::invalid_input("ANSI color value must be an integer")
            })?;
            return Ok(iyon_tui::ColorSpec::ansi(u8::try_from(number).map_err(
                |_| crate::NativeError::invalid_input("ANSI color value must fit in u8"),
            )?));
        }
        return Err(crate::NativeError::invalid_input(format!(
            "unknown color object type `{kind}`"
        )));
    }
    let value = value.as_str().ok_or_else(|| {
        crate::NativeError::invalid_input("color must be a string or ANSI color object")
    })?;
    if let Some(value) = value.strip_prefix("theme:") {
        return Ok(iyon_tui::ColorSpec::theme(value));
    }
    if let Some(value) = value.strip_prefix("ansi:") {
        return Ok(iyon_tui::ColorSpec::ansi(value.parse::<u8>().map_err(
            |_| crate::NativeError::invalid_input("ANSI color must fit in u8"),
        )?));
    }
    if let Some(value) = value.strip_prefix('#') {
        if value.len() == 6 {
            let r = u8::from_str_radix(&value[0..2], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            let g = u8::from_str_radix(&value[2..4], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            let b = u8::from_str_radix(&value[4..6], 16).map_err(|_| {
                crate::NativeError::invalid_input("RGB color must contain hexadecimal bytes")
            })?;
            return Ok(iyon_tui::ColorSpec::rgb(r, g, b));
        }
    }
    let color = match value.to_ascii_lowercase().as_str() {
        "black" => iyon_tui::AnsiColor::Black,
        "red" => iyon_tui::AnsiColor::Red,
        "green" => iyon_tui::AnsiColor::Green,
        "yellow" => iyon_tui::AnsiColor::Yellow,
        "blue" => iyon_tui::AnsiColor::Blue,
        "magenta" => iyon_tui::AnsiColor::Magenta,
        "cyan" => iyon_tui::AnsiColor::Cyan,
        "gray" => iyon_tui::AnsiColor::Gray,
        "darkgray" => iyon_tui::AnsiColor::DarkGray,
        "lightred" => iyon_tui::AnsiColor::LightRed,
        "lightgreen" => iyon_tui::AnsiColor::LightGreen,
        "lightyellow" => iyon_tui::AnsiColor::LightYellow,
        "lightblue" => iyon_tui::AnsiColor::LightBlue,
        "lightmagenta" => iyon_tui::AnsiColor::LightMagenta,
        "lightcyan" => iyon_tui::AnsiColor::LightCyan,
        "white" => iyon_tui::AnsiColor::White,
        _ => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown color `{value}`"
            )));
        }
    };
    Ok(iyon_tui::ColorSpec::named(color))
}

fn text_attribute(value: &str) -> Option<iyon_tui::TextAttribute> {
    match value {
        "bold" => Some(iyon_tui::TextAttribute::Bold),
        "dim" => Some(iyon_tui::TextAttribute::Dim),
        "italic" => Some(iyon_tui::TextAttribute::Italic),
        "underline" => Some(iyon_tui::TextAttribute::Underline),
        "reversed" => Some(iyon_tui::TextAttribute::Reversed),
        "strikethrough" => Some(iyon_tui::TextAttribute::Strikethrough),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lowers_nested_composition_through_canonical_views() {
        let value = json!({
            "type": "column",
            "gap": 0,
            "children": [
                {"kind": "normal", "child": {"type": "text", "spans": [{"text": "one"}]}},
                {"kind": "normal", "child": {"type": "row", "gap": 0, "children": [{"kind": "normal", "child": {"type": "text", "spans": [{"text": "two"}]}}]}}
            ]
        });
        assert!(lower_view(&value).is_ok());
    }

    #[test]
    fn rejects_unknown_nodes_before_native_construction() {
        let error = lower_view(&json!({"type": "unknown"})).unwrap_err();
        assert!(error.to_string().contains("unknown view node type"));
    }

    #[test]
    fn native_text_input_owns_unicode_cursor_state() {
        let input = NativeTextInput::new(None);
        input.set_text("hello 🌍".into()).unwrap();
        assert_eq!(input.text().unwrap(), "hello 🌍");
        assert_eq!(input.cursor_bytes().unwrap(), "hello 🌍".len() as i64);
        input.dispose();
        assert!(input.text().is_err());
    }

    #[test]
    fn native_stream_rejects_updates_after_seal() {
        let stream = NativeTextStream::new(None).unwrap();
        stream.update("first".into()).unwrap();
        stream.seal().unwrap();
        assert!(stream.update("late".into()).is_err());
    }
}
