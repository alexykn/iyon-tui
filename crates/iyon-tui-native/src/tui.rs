use napi::Env;
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use iyon_tui::binding::{
    AnsiColor, ColorSpec, ContentDelivery, ContentFamily, History, HistoryLayout, HorizontalAlign,
    HostCellStyle, HostContentConnector, HostContentFunnel, HostContentPort, HostContentSource,
    HostHistory, HostScrollPane, HostTextInput, HostViewSlot, Insets, Key, KeyStroke, Modifiers,
    Output, SmoothConfig, TextAttribute, TextFunnelKind, TextInput, TextSourceKind, TextWrapMode,
    TuiEnvironment, TuiHost, View, WrapMode, view_native_text_final,
};
use serde_json::Map;
use serde_json::Value;

mod generated_view_abi_conformance {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/view_abi_conformance.rs"
    ));
}

#[allow(dead_code)]
mod view_state_schema {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/generated/view_state_schema.rs"
    ));
}

mod theme_dto;
mod ui_commit;
mod view_abi;
mod view_state;

use view_state::NativeViewState;

type ViewRuntimeHandle = view_abi::ViewRuntimeHandle;

static HOST_ENVIRONMENTS: OnceLock<Mutex<HashMap<usize, TuiEnvironment>>> = OnceLock::new();
static CONTENT_ENVIRONMENTS: OnceLock<Mutex<HashMap<u32, TuiEnvironment>>> = OnceLock::new();

fn host_environments() -> &'static Mutex<HashMap<usize, TuiEnvironment>> {
    HOST_ENVIRONMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_environments() -> &'static Mutex<HashMap<u32, TuiEnvironment>> {
    CONTENT_ENVIRONMENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_connector_status_value(
    phase: String,
    requested: bool,
    visible: bool,
    projected_source_revision: Option<u64>,
    operating_error: Option<Value>,
    cleanup_error: Option<Value>,
) -> Value {
    // Keep the supported native/TypeScript status shape stable: deferred
    // Source cleanup uses the existing error lane, with cleanup taking
    // precedence over an older operating diagnostic.  The Rust core keeps
    // the two causes distinct, but consumers do not need new TS fields to
    // diagnose and explicitly retry the accepted operation.
    serde_json::json!({
        "phase": phase,
        "requested": requested,
        "visible": visible,
        "projectedSourceRevision": projected_source_revision.map(|revision| revision.to_string()),
        "error": cleanup_error.or(operating_error),
    })
}

fn register_content_environment(environment: &TuiEnvironment) -> Result<()> {
    content_environments()
        .lock()
        .map_err(|_| {
            crate::NativeError::internal("native content environment registry is poisoned")
        })?
        .insert(environment.environment_slot(), environment.clone());
    Ok(())
}

fn remove_content_environment(slot: u32) {
    if let Ok(mut environments) = content_environments().lock() {
        environments.remove(&slot);
    }
}

#[cfg(test)]
pub(crate) fn register_content_environment_for_test(environment: &TuiEnvironment) {
    content_environments()
        .lock()
        .expect("test content environment registry must be healthy")
        .insert(environment.environment_slot(), environment.clone());
}

#[cfg(test)]
pub(crate) fn remove_content_environment_for_test(slot: u32) {
    remove_content_environment(slot);
}

pub(crate) fn content_environment_for_identity(
    slot: u32,
    generation: u32,
) -> std::result::Result<TuiEnvironment, String> {
    let environment = content_environments()
        .lock()
        .map_err(|_| "native content environment registry is poisoned".to_owned())?
        .get(&slot)
        .cloned()
        .ok_or_else(|| format!("STALE_ENVIRONMENT: environment {slot} is unavailable"))?;
    if environment.environment_generation() != generation {
        return Err(format!(
            "STALE_ENVIRONMENT: environment {slot} generation is stale"
        ));
    }
    Ok(environment)
}

fn host_environment_for_env(env: &Env) -> Result<TuiEnvironment> {
    let env_key = env.raw() as usize;
    let mut environments = host_environments().lock().map_err(|_| {
        crate::NativeError::internal("native host environment registry is poisoned")
    })?;
    if let Some(environment) = environments.get(&env_key) {
        return Ok(environment.clone());
    }
    let environment = TuiEnvironment::new();
    let environment_slot = environment.environment_slot();
    register_content_environment(&environment)?;
    let cleanup_key = env_key ^ 0x484f_5354;
    if let Err(error) = env.add_env_cleanup_hook(cleanup_key, move |_| {
        if let Some(registry) = HOST_ENVIRONMENTS.get()
            && let Ok(mut environments) = registry.lock()
        {
            environments.remove(&env_key);
        }
        remove_content_environment(environment_slot);
    }) {
        remove_content_environment(environment_slot);
        return Err(error);
    }
    environments.insert(env_key, environment.clone());
    Ok(environment)
}

/// Link/surface probe only: construct one owned public TUI value and discard
/// it. The native boundary must not duplicate or serialize the TUI renderer.
#[napi(js_name = "tuiSmoke")]
pub fn tui_smoke() -> Result<String> {
    let _view = view_native_text_final(
        vec![iyon_tui::binding::text_span_plain("iyon-tui/t1")],
        WrapMode::default(),
        HorizontalAlign::Start,
    );
    Ok("iyon-tui/t1".to_owned())
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub extern "C" fn iyon_abi_probe_noop(value: u32) -> u32 {
    value.wrapping_add(1)
}

#[cfg(feature = "direct-ffi")]
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

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub extern "C" fn iyon_abi_probe_i32_4(a0: i32, a1: i32, a2: i32, a3: i32) -> i32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_probe_buffer(bytes: *const u8, byte_length: usize) -> u32 {
    if bytes.is_null() {
        return u32::MAX;
    }
    let first = unsafe { *bytes } as u32;
    (byte_length as u32).wrapping_mul(257).wrapping_add(first)
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_probe_cstring(value: *const std::ffi::c_char) -> u32 {
    if value.is_null() {
        return 0;
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(value) }.to_bytes();
    bytes.iter().fold(2_166_136_261_u32, |hash, byte| {
        hash.wrapping_mul(16_777_619).wrapping_add(u32::from(*byte))
    })
}

#[cfg(feature = "direct-ffi")]
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

#[cfg(feature = "direct-ffi")]
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
    iyon_tui::binding::reset();
}

#[cfg(feature = "perf-counters")]
#[napi(js_name = "tuiPerfSnapshot")]
pub fn tui_perf_snapshot() -> Value {
    let mut counters = Map::new();
    for (name, value) in iyon_tui::binding::snapshot().iter() {
        counters.insert(name.to_owned(), Value::from(value));
    }
    Value::Object(counters)
}

#[napi(js_name = "tuiViewEnvironmentCount")]
pub fn tui_view_environment_count() -> i64 {
    view_abi::runtime_environment_count()
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
    pub fn dispose(&self) -> Result<()> {
        if !self.alive.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        Ok(())
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
        let layout = HistoryLayout::from_parts(Insets::new(0, 0, padding, 0), gap);
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
        if self.alive.swap(false, Ordering::AcqRel)
            && let Some(host) = &self.host
        {
            host.retire();
        }
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
    ui_state: Mutex<ui_commit::NativeUiState>,
    ui_environment: iyon_tui::binding::TuiEnvironment,
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
        let environment = host_environment_for_env(&env)?;
        let ui_environment = environment.clone();
        let host = Box::new(
            TuiHost::open_in_environment(width, height, headless.unwrap_or(false), environment)
                .map_err(|error| crate::NativeError::internal(error.to_string()))?,
        );
        let view_runtime = view_abi::runtime_ptr_for_env(&env)? as usize;
        let ui_namespace = iyon_tui::binding::HostNamespace::allocate()
            .ok_or_else(|| crate::NativeError::internal("UI host namespace exhausted"))?;
        Ok(Self {
            host,
            alive: AtomicBool::new(true),
            view_runtime,
            ui_state: Mutex::new(ui_commit::NativeUiState::new(
                ui_namespace,
                ui_environment.clone(),
            )),
            ui_environment,
        })
    }

    /// Returns desired/visible revisions and authoritative host epochs.
    #[napi]
    pub fn epochs(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let epochs = self
            .host
            .epochs()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(serde_json::json!({
            "host_id": epochs.host_id.to_string(),
            "desired_structural_revision": epochs.desired_structural_revision.to_string(),
            "visible_structural_revision": epochs.visible_structural_revision.to_string(),
            "visible_frame_revision": epochs.visible_frame_revision.to_string(),
            "pending_epoch": epochs.pending_epoch.to_string(),
            "committed_epoch": epochs.committed_epoch.to_string(),
        }))
    }

    #[napi(js_name = "uiNamespace")]
    pub fn ui_namespace(&self) -> Result<u32> {
        ensure_alive(&self.alive)?;
        self.ui_state
            .lock()
            .map_err(|_| crate::NativeError::internal("UI state lock is poisoned"))
            .map(|state| state.namespace().get())
    }

    /// Accepts a native retained root as desired structure without presenting
    /// it. The next environment drain performs the frame transaction.
    #[napi(js_name = "setDesiredViewRef")]
    pub fn set_desired_view_ref(&self, view_ref: i64) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let view = resolve_native_view(self.view_runtime, view_ref)?;
        let disposition = self
            .host
            .set_desired_view(view)
            .map_err(|error| crate::NativeError::content(error.to_string()))?;
        let epochs = self
            .host
            .epochs()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(serde_json::json!({
            "host_id": epochs.host_id.to_string(),
            "schedule_environment_drain": disposition.schedule_environment_drain,
        }))
    }

    #[napi(js_name = "commitUiV1")]
    pub fn commit_ui_v1(
        &self,
        env: Env,
        words: napi::bindgen_prelude::TypedArray,
        metadata: napi::bindgen_prelude::TypedArray,
        owned_content: napi::bindgen_prelude::TypedArray,
        sources: napi::bindgen_prelude::Array,
    ) -> Result<napi::bindgen_prelude::Uint32Array> {
        ensure_alive(&self.alive)?;
        ui_commit::commit_ui_v1(self, &env, words, metadata, owned_content, sources)
    }

    #[napi(js_name = "clearViewStateBindings")]
    pub fn clear_view_state_bindings(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .clear_view_state_bindings()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    /// Drains the native environment's fair pending-host queue. Automatic
    /// callers leave retry-blocked hosts blocked; explicit barriers force one
    /// retry and surface the returned error records synchronously.
    #[napi(js_name = "flushPendingHosts")]
    pub fn flush_pending_hosts(
        &self,
        budget: Option<i64>,
        force_retry: Option<bool>,
    ) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let budget = budget.unwrap_or(32);
        let budget = usize::try_from(budget)
            .ok()
            .filter(|budget| (1..=1024).contains(budget))
            .ok_or_else(|| {
                crate::NativeError::invalid_input("host flush budget must be 1 through 1024")
            })?;
        let report = self
            .host
            .flush_pending_hosts(budget, force_retry.unwrap_or(false))
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        let errors = report
            .errors
            .iter()
            .map(|error| {
                serde_json::json!({
                    "host_id": error.host_id.to_string(),
                    "attempted_epoch": error.attempted_epoch.to_string(),
                    "desired_revision": error.desired_revision.to_string(),
                    "phase": error.phase,
                    "code": error.code,
                    "retryable": error.retryable,
                    "diagnostic": error.diagnostic,
                })
            })
            .collect::<Vec<_>>();
        let commits = report
            .commits
            .iter()
            .map(|commit| {
                serde_json::json!({
                    "host_id": commit.host_id.to_string(),
                    "committed_epoch": commit.committed_epoch.to_string(),
                    "visible_structural_revision": commit
                        .visible_structural_revision
                        .to_string(),
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "rearm": report.rearm,
            "waiting_for_presentation": report.waiting_for_presentation,
            "attempted": report.attempted,
            "commits": commits,
            "errors": errors,
            "wake_epoch": report.wake_epoch.to_string(),
        }))
    }

    #[napi]
    pub fn dispose(&self) -> Result<()> {
        if self.alive.load(Ordering::Acquire) {
            view_abi::abort_all_edit_txns(self.view_runtime as *mut view_abi::NativeViewRuntime);
            let host_error = self.host.close().err();
            let ui_error = self
                .ui_state
                .lock()
                .map_err(|_| "UI state lock is poisoned".to_owned())
                .and_then(|mut state| state.close());
            match (host_error, ui_error) {
                (None, Ok(())) => self.alive.store(false, Ordering::Release),
                (host_error, ui_error) => {
                    let mut diagnostics = Vec::new();
                    if let Some(error) = host_error {
                        diagnostics.push(format!("host close failed: {error:#}"));
                    }
                    if let Err(error) = ui_error {
                        diagnostics.push(format!("UI resource cleanup failed: {error}"));
                    }
                    return Err(crate::NativeError::internal(diagnostics.join("; ")));
                }
            }
        }
        Ok(())
    }

    #[napi(js_name = "disposeContentResources")]
    pub fn dispose_content_resources(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .dispose_content_resources()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
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
            .set_theme(theme_dto::decode_theme(value)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "setHistory")]
    pub fn set_history(&self, history: &mut NativeHistory) -> Result<()> {
        ensure_alive(&self.alive)?;
        if history.host.is_some() {
            return Err(crate::NativeError::invalid_input(
                "history is already attached to a native host",
            ));
        }
        let state = history
            .state
            .lock()
            .map_err(|_| crate::NativeError::internal("history lock is poisoned"))?;
        self.host
            .validate_history(&state)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        drop(state);
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

    #[napi(js_name = "viewState")]
    pub fn view_state(&self) -> Result<NativeViewState> {
        ensure_alive(&self.alive)?;
        let state = self
            .host
            .create_view_state()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeViewState::from_host(state))
    }

    #[napi(js_name = "contentPort")]
    pub fn content_port(&self, family: Option<String>) -> Result<NativeContentPort> {
        ensure_alive(&self.alive)?;
        let family = family.unwrap_or_else(|| "text".to_owned());
        if family != "text" {
            return Err(crate::NativeError::invalid_input(
                "unsupported ContentPort family",
            ));
        }
        let port = self
            .host
            .create_content_port(ContentFamily::Text)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(NativeContentPort::from_host(port))
    }

    #[napi(js_name = "textInput")]
    pub fn text_input(
        &self,
        multiline: Option<bool>,
        border: Option<Value>,
    ) -> Result<NativeTextInput> {
        ensure_alive(&self.alive)?;
        // Validate and lower the border before registering the component so a
        // malformed option cannot leave an unreachable host component behind.
        let border = border.map(theme_dto::build_border_spec).transpose()?;
        let input = self
            .host
            .create_text_input(multiline.unwrap_or(false))
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        if let Some(border) = border
            && let Err(error) = input.set_border(border)
        {
            input.retire();
            return Err(crate::NativeError::internal(error.to_string()));
        }
        Ok(NativeTextInput::from_host(input))
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

#[cfg(feature = "direct-ffi")]
#[napi]
impl NativeTuiHost {
    /// Qualification-only raw host address for the feature-gated direct FFI backend.
    #[napi(js_name = "tuiViewAbiHostPointer")]
    pub fn view_abi_host_pointer(&self) -> i64 {
        if !self.alive.load(Ordering::Acquire) {
            return 0;
        }
        self as *const Self as usize as i64
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
pub struct NativeTextSource {
    source: HostContentSource,
    alive: AtomicBool,
}

#[napi]
impl NativeTextSource {
    #[napi(constructor)]
    pub fn new(env: Env, kind: Option<String>, options: Option<Value>) -> Result<Self> {
        let retention = parse_text_source_options(options)?;
        let kind = match kind.as_deref().unwrap_or("stream") {
            "stream" => TextSourceKind::Stream,
            "block" => TextSourceKind::Block,
            other => {
                return Err(crate::NativeError::invalid_input(format!(
                    "unknown text Source kind `{other}`"
                )));
            }
        };
        let environment = host_environment_for_env(&env)?;
        let source = environment
            .create_content_source(kind)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        if let Some(retention) = retention
            && let Err(error) = source.configure_retention(
                retention.max_bytes,
                retention.max_lines,
                retention.drop_oldest,
            )
        {
            let diagnostic = crate::NativeError::content(error.to_string());
            return match source.dispose() {
                Ok(()) => Err(diagnostic),
                Err(cleanup) => Err(crate::NativeError::internal(format!(
                    "{diagnostic}; Source cleanup failed: {cleanup}"
                ))),
            };
        }
        Ok(Self {
            source,
            alive: AtomicBool::new(true),
        })
    }

    #[napi]
    pub fn dispose(&self) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        self.source.dispose().map_err(crate::NativeError::content)?;
        self.alive.store(false, Ordering::Release);
        Ok(())
    }

    #[napi(js_name = "sourceId")]
    pub fn source_id(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(self.source.id() as i64)
    }

    #[napi(js_name = "sourceGeneration")]
    pub fn source_generation(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(i64::from(self.source.generation()))
    }

    #[napi(js_name = "environmentSlot")]
    pub fn environment_slot(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(i64::from(self.source.environment_slot()))
    }

    #[napi(js_name = "environmentGeneration")]
    pub fn environment_generation(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(i64::from(self.source.environment_generation()))
    }

    #[napi(js_name = "contentGeneration")]
    pub fn content_generation(&self) -> Result<String> {
        ensure_alive(&self.alive)?;
        Ok(self
            .source
            .content_generation()
            .map_err(crate::NativeError::content)?
            .to_string())
    }

    #[napi]
    pub fn snapshot(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let snapshot = self
            .source
            .snapshot()
            .map_err(crate::NativeError::content)?;
        let annotations = snapshot
            .annotations()
            .into_iter()
            .map(|annotation| {
                serde_json::json!({
                    "kind": annotation.kind,
                    "flags": annotation.flags,
                    "startByte": annotation.start_byte.to_string(),
                    "endByte": annotation.end_byte.to_string(),
                    "payload": annotation.payload,
                    "aux0": annotation.aux0,
                    "aux1": annotation.aux1,
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sourceId": snapshot.source_id.to_string(),
            "sourceGeneration": snapshot.source_generation,
            "contentGeneration": snapshot.content_generation.to_string(),
            "revision": snapshot.revision.to_string(),
            "sourceBase": snapshot.source_base.to_string(),
            "sourceEnd": snapshot.source_end.to_string(),
            "sealed": snapshot.sealed,
            "headPartial": snapshot.head_partial,
            "text": snapshot.text(),
            "annotations": annotations,
        }))
    }

    #[napi]
    pub fn stats(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let stats = self.source.stats().map_err(crate::NativeError::content)?;
        Ok(serde_json::json!({
            "revision": stats.revision.to_string(),
            "sourceBase": stats.source_base.to_string(),
            "sourceEnd": stats.source_end.to_string(),
            "retainedBytes": stats.retained_bytes.to_string(),
            "retainedLines": stats.retained_lines.to_string(),
            "chunkCount": stats.chunk_count,
            "sealed": stats.sealed,
            "headPartial": stats.head_partial,
            "acceptedBytes": stats.accepted_bytes.to_string(),
            "copiedBytes": stats.copied_bytes.to_string(),
            "droppedHeadBytes": stats.dropped_head_bytes.to_string(),
        }))
    }

    #[napi]
    pub fn family(&self) -> Result<String> {
        ensure_alive(&self.alive)?;
        Ok("text".to_owned())
    }
}

#[napi]
pub struct NativeContentPort {
    port: HostContentPort,
    alive: AtomicBool,
}

#[napi]
impl NativeContentPort {
    #[napi]
    pub fn dispose(&self) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        self.port.dispose().map_err(crate::NativeError::content)?;
        self.alive.store(false, Ordering::Release);
        Ok(())
    }

    #[napi(js_name = "portId")]
    pub fn port_id(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(self.port.id() as i64)
    }

    #[napi(js_name = "attachmentId")]
    pub fn attachment_id(&self) -> Result<i64> {
        self.port_id()
    }

    #[napi(js_name = "portGeneration")]
    pub fn port_generation(&self) -> Result<i64> {
        ensure_alive(&self.alive)?;
        Ok(i64::from(self.port.generation()))
    }

    #[napi]
    pub fn family(&self) -> Result<String> {
        ensure_alive(&self.alive)?;
        Ok("text".to_owned())
    }

    #[napi]
    pub fn deactivate(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let wake = self
            .port
            .deactivate()
            .map_err(crate::NativeError::content)?;
        Ok(serde_json::json!({
            "schedule_environment_drain": wake.schedule_environment_drain,
        }))
    }

    #[napi]
    pub fn connect(
        &self,
        source: &NativeTextSource,
        kind: String,
        wrap: String,
        hyperlinks: bool,
        smooth: bool,
        tick_interval_ms: u32,
        spring: f64,
        min_units_per_second: f64,
        max_units_per_second: f64,
    ) -> Result<NativeContentConnector> {
        ensure_alive(&self.alive)?;
        ensure_alive(&source.alive)?;
        let funnel = parse_text_funnel_control(
            &kind,
            &wrap,
            hyperlinks,
            smooth,
            u64::from(tick_interval_ms),
            spring,
            min_units_per_second,
            max_units_per_second,
        )?;
        let connector = self
            .port
            .connect(&source.source, funnel)
            .map_err(crate::NativeError::content)?;
        Ok(NativeContentConnector {
            connector,
            alive: AtomicBool::new(true),
        })
    }

    #[napi]
    pub fn mounted(&self) -> Result<bool> {
        ensure_alive(&self.alive)?;
        self.port
            .is_mounted()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    fn from_host(port: HostContentPort) -> Self {
        Self {
            port,
            alive: AtomicBool::new(true),
        }
    }
}

#[napi]
pub struct NativeContentConnector {
    connector: HostContentConnector,
    alive: AtomicBool,
}

#[napi]
impl NativeContentConnector {
    #[napi]
    pub fn activate(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let wake = self
            .connector
            .activate()
            .map_err(crate::NativeError::content)?;
        Ok(serde_json::json!({
            "schedule_environment_drain": wake.schedule_environment_drain,
        }))
    }

    #[napi]
    pub fn deactivate(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let wake = self
            .connector
            .deactivate()
            .map_err(crate::NativeError::content)?;
        Ok(serde_json::json!({
            "schedule_environment_drain": wake.schedule_environment_drain,
        }))
    }

    #[napi]
    pub fn dispose(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        if self.connector.is_disposed() {
            return Ok(serde_json::json!({
                "schedule_environment_drain": false,
            }));
        }
        let wake = self
            .connector
            .dispose()
            .map_err(crate::NativeError::content)?;
        Ok(serde_json::json!({
            "schedule_environment_drain": wake.schedule_environment_drain,
        }))
    }

    /// Native/unit-only failure injection for validating switch rollback. The
    /// public TypeScript Connector intentionally does not expose this hook.
    #[napi(js_name = "failNextActivation")]
    pub fn fail_next_activation(&self, diagnostic: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.connector
            .fail_next_activation(diagnostic)
            .map_err(crate::NativeError::content)
    }

    #[napi]
    pub fn status(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let status = self
            .connector
            .status()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        let operating_error = status.error.map(|error| {
            serde_json::json!({
                "code": error.code,
                "diagnostic": error.diagnostic,
            })
        });
        let cleanup_error = status.cleanup_error.map(|error| {
            serde_json::json!({
                "code": error.code,
                "diagnostic": error.diagnostic,
            })
        });
        Ok(content_connector_status_value(
            status.phase,
            status.requested,
            status.visible,
            status.projected_source_revision,
            operating_error,
            cleanup_error,
        ))
    }
}

struct TextSourceRetentionConfig {
    max_bytes: Option<u64>,
    max_lines: Option<u64>,
    drop_oldest: bool,
}

fn parse_text_source_options(value: Option<Value>) -> Result<Option<TextSourceRetentionConfig>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let object = value.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("text Source options must be an object")
    })?;
    for key in object.keys() {
        if key != "retention" {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown text Source option `{key}`"
            )));
        }
    }
    let Some(retention) = object.get("retention") else {
        return Ok(None);
    };
    let retention = retention.as_object().ok_or_else(|| {
        crate::NativeError::invalid_input("text Source retention must be an object")
    })?;
    for key in retention.keys() {
        if !matches!(key.as_str(), "maxBytes" | "maxLines" | "overflow") {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown text Source retention option `{key}`"
            )));
        }
    }
    let max_bytes = optional_positive_safe_u64(retention, "maxBytes")?;
    let max_lines = optional_positive_safe_u64(retention, "maxLines")?;
    if max_bytes.is_none() && max_lines.is_none() {
        return Err(crate::NativeError::invalid_input(
            "text Source retention requires maxBytes or maxLines",
        ));
    }
    let drop_oldest = match retention.get("overflow").and_then(Value::as_str) {
        Some("drop-oldest") => true,
        Some("error") => false,
        Some(_) | None => {
            return Err(crate::NativeError::invalid_input(
                "text Source retention overflow must be drop-oldest or error",
            ));
        }
    };
    Ok(Some(TextSourceRetentionConfig {
        max_bytes,
        max_lines,
        drop_oldest,
    }))
}

fn optional_positive_safe_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    let value = value.as_u64().ok_or_else(|| {
        crate::NativeError::invalid_input(format!(
            "text Source retention {field} must be a positive safe integer"
        ))
    })?;
    if value == 0 || value > 9_007_199_254_740_991 {
        return Err(crate::NativeError::invalid_input(format!(
            "text Source retention {field} must be a positive safe integer"
        )));
    }
    Ok(Some(value))
}

fn parse_text_funnel_control(
    kind: &str,
    wrap: &str,
    hyperlinks: bool,
    smooth: bool,
    tick_interval_ms: u64,
    spring: f64,
    minimum: f64,
    maximum: f64,
) -> Result<HostContentFunnel> {
    let kind = match kind {
        "plain" => TextFunnelKind::Plain,
        "markdown" => TextFunnelKind::Markdown,
        "diff" => TextFunnelKind::Diff,
        "ansi" => TextFunnelKind::Ansi,
        _ => {
            return Err(crate::NativeError::invalid_input(
                "Content Funnel kind is invalid",
            ));
        }
    };
    let wrap = match wrap {
        "word" => TextWrapMode::Word,
        "grapheme" => TextWrapMode::Grapheme,
        "noWrap" => TextWrapMode::NoWrap,
        _ => {
            return Err(crate::NativeError::invalid_input(
                "Content Funnel wrap mode is invalid",
            ));
        }
    };
    let delivery = if !smooth {
        ContentDelivery::Immediate
    } else {
        if !spring.is_finite()
            || !minimum.is_finite()
            || !maximum.is_finite()
            || spring < f64::from(f32::MIN)
            || spring > f64::from(f32::MAX)
            || minimum < f64::from(f32::MIN)
            || minimum > f64::from(f32::MAX)
            || maximum < f64::from(f32::MIN)
            || maximum > f64::from(f32::MAX)
        {
            return Err(crate::NativeError::invalid_input(
                "Smooth values must be finite f32 values",
            ));
        }
        let config = SmoothConfig::try_from_parts(
            Duration::from_millis(tick_interval_ms),
            spring as f32,
            minimum as f32,
            maximum as f32,
        )
        .map_err(|error| crate::NativeError::invalid_input(error.to_string()))?;
        ContentDelivery::Smooth(config)
    };
    Ok(HostContentFunnel::new(kind, wrap, hyperlinks, delivery))
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
    #[napi]
    pub fn dispose(&self) {
        // PERF-12 T13.1 R8: disposal REQUESTS deferred retirement of the
        // registered component (idempotent); physical reclamation happens in
        // NativeRuntime::reap_retired_components after reconciliation proves the
        // component unmounted. The N-API surface is the durable public path.
        if self.alive.swap(false, Ordering::AcqRel) {
            self.pane.retire();
        }
    }

    #[napi(js_name = "componentId")]
    pub fn component_id(&self) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        Ok(self.pane.component_id().map(|id| id as i64))
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
    #[napi]
    pub fn dispose(&self) {
        // PERF-12 T13.1 R8: disposal REQUESTS deferred retirement of the
        // registered component (idempotent). Physical reclamation happens in
        // NativeRuntime::reap_retired_components after reconciliation proves the
        // component unmounted — committed roots may still reference it.
        if self.alive.swap(false, Ordering::AcqRel) {
            self.slot.retire();
        }
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

/// Canonical color-string decoder shared with the L1-05 state envelope
/// string lane. The TS packer normalizes `{type: "ansi", value}` objects to
/// `ansi:N` strings, so the envelope carries only strings; the theme DTO
/// handles object shapes separately and both terminate in identical values.
pub(super) fn color_spec_str(value: &str) -> Result<ColorSpec> {
    if let Some(value) = value.strip_prefix("theme:") {
        // Intern repeated theme keys once; identical strings share one
        // allocation instead of re-allocating per materialization.
        return Ok(ColorSpec::theme(iyon_tui::binding::intern_style_atom(
            value,
        )));
    }
    if let Some(value) = value.strip_prefix("ansi:") {
        return Ok(ColorSpec::ansi(value.parse::<u8>().map_err(|_| {
            crate::NativeError::invalid_input("ANSI color must fit in u8")
        })?));
    }
    match parse_rgb_hex(value) {
        Ok(Some((r, g, b))) => return Ok(ColorSpec::rgb(r, g, b)),
        Err(()) => {
            return Err(crate::NativeError::invalid_input(
                "RGB color must contain hexadecimal bytes",
            ));
        }
        Ok(None) => {}
    }
    let color = match value.to_ascii_lowercase().as_str() {
        "black" => AnsiColor::Black,
        "red" => AnsiColor::Red,
        "green" => AnsiColor::Green,
        "yellow" => AnsiColor::Yellow,
        "blue" => AnsiColor::Blue,
        "magenta" => AnsiColor::Magenta,
        "cyan" => AnsiColor::Cyan,
        "gray" => AnsiColor::Gray,
        "darkgray" => AnsiColor::DarkGray,
        "lightred" => AnsiColor::LightRed,
        "lightgreen" => AnsiColor::LightGreen,
        "lightyellow" => AnsiColor::LightYellow,
        "lightblue" => AnsiColor::LightBlue,
        "lightmagenta" => AnsiColor::LightMagenta,
        "lightcyan" => AnsiColor::LightCyan,
        "white" => AnsiColor::White,
        _ => {
            return Err(crate::NativeError::invalid_input(format!(
                "unknown color `{value}`"
            )));
        }
    };
    Ok(ColorSpec::named(color))
}

/// Decodes the six ASCII hexadecimal digits in an RGB color before any
/// byte-indexed slicing. A non-ASCII six-byte payload (for example
/// `#aé000`) is malformed input, not a valid UTF-8 boundary for a pair.
pub(super) fn parse_rgb_hex(value: &str) -> std::result::Result<Option<(u8, u8, u8)>, ()> {
    let Some(value) = value.strip_prefix('#') else {
        return Ok(None);
    };
    if value.len() != 6 {
        return Ok(None);
    }
    if !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    let r = u8::from_str_radix(&value[0..2], 16).map_err(|_| ())?;
    let g = u8::from_str_radix(&value[2..4], 16).map_err(|_| ())?;
    let b = u8::from_str_radix(&value[4..6], 16).map_err(|_| ())?;
    Ok(Some((r, g, b)))
}

pub(super) fn text_attribute(value: &str) -> Option<TextAttribute> {
    match value {
        "bold" => Some(TextAttribute::Bold),
        "dim" => Some(TextAttribute::Dim),
        "italic" => Some(TextAttribute::Italic),
        "underline" => Some(TextAttribute::Underline),
        "reversed" => Some(TextAttribute::Reversed),
        "strikethrough" => Some(TextAttribute::Strikethrough),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn content_connector_status_maps_cleanup_through_the_existing_error_lane() {
        let cleanup_error = serde_json::json!({
            "code": "SOURCE_CLEANUP_PENDING",
            "diagnostic": "Source membership is retained until retry",
        });
        let pending = content_connector_status_value(
            "active".to_owned(),
            false,
            false,
            Some(7),
            None,
            Some(cleanup_error.clone()),
        );
        let pending_object = pending.as_object().expect("status must be an object");
        assert_eq!(pending_object.get("error"), Some(&cleanup_error));
        assert!(!pending_object.contains_key("cleanupPending"));
        assert!(!pending_object.contains_key("cleanupError"));

        let operating_error = serde_json::json!({
            "code": "PROJECTION_FAILED",
            "diagnostic": "projection failed",
        });
        let operating = content_connector_status_value(
            "failed".to_owned(),
            true,
            false,
            None,
            Some(operating_error.clone()),
            None,
        );
        assert_eq!(
            operating
                .as_object()
                .expect("status must be an object")
                .get("error"),
            Some(&operating_error)
        );
    }
}
