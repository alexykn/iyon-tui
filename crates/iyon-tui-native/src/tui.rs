use napi::Env;
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use iyon_tui::binding::{
    AnsiColor, ColorSpec, ContentDelivery, ContentFamily, HostCellStyle, HostContentConnector,
    HostContentFunnel, HostContentPort, HostContentSource, Key, KeyStroke, Modifiers, SmoothConfig,
    TextAttribute, TextFunnelKind, TextSourceKind, TextWrapMode, TuiEnvironment, TuiHost,
};
use serde_json::{Map, Value};

mod theme_dto;
mod ui_commit;

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
    Ok("iyon-tui/t1".to_owned())
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

fn decode_ui_node_handle(words: &[u32]) -> Result<iyon_tui::binding::UiHandle> {
    if words.len() != iyon_tui::binding::UI_HANDLE_WORDS {
        return Err(crate::NativeError::invalid_input(
            "UI occurrence handle must contain four words",
        ));
    }
    if iyon_tui::binding::HandleKind::from_code(words[3])
        != Some(iyon_tui::binding::HandleKind::Node)
    {
        return Err(crate::NativeError::invalid_input(
            "UI handle must identify an occurrence",
        ));
    }
    let namespace = iyon_tui::binding::HostNamespace::new(words[0])
        .ok_or_else(|| crate::NativeError::invalid_input("invalid UI host namespace"))?;
    iyon_tui::binding::UiHandle::new(
        namespace,
        words[1],
        words[2],
        iyon_tui::binding::HandleKind::Node,
    )
    .ok_or_else(|| crate::NativeError::invalid_input("invalid UI occurrence handle"))
}

fn decode_ui_resource_handle(
    words: &[u32],
    kind: iyon_tui::binding::HandleKind,
) -> Result<iyon_tui::binding::UiHandle> {
    if words.len() != iyon_tui::binding::UI_HANDLE_WORDS {
        return Err(crate::NativeError::invalid_input(
            "UI resource handle must contain four words",
        ));
    }
    if iyon_tui::binding::HandleKind::from_code(words[3]) != Some(kind) {
        return Err(crate::NativeError::invalid_input(
            "UI resource handle has the wrong kind",
        ));
    }
    let namespace = iyon_tui::binding::HostNamespace::new(words[0])
        .ok_or_else(|| crate::NativeError::invalid_input("invalid UI host namespace"))?;
    iyon_tui::binding::UiHandle::new(namespace, words[1], words[2], kind)
        .ok_or_else(|| crate::NativeError::invalid_input("invalid UI resource handle"))
}

#[napi]
pub struct NativeTuiHost {
    host: Box<TuiHost>,
    alive: AtomicBool,
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
        let ui_namespace = iyon_tui::binding::HostNamespace::allocate()
            .ok_or_else(|| crate::NativeError::internal("UI host namespace exhausted"))?;
        let host = Box::new(
            TuiHost::open_in_environment_with_ui(
                width,
                height,
                headless.unwrap_or(false),
                environment,
                ui_namespace,
            )
            .map_err(|error| crate::NativeError::internal(error.to_string()))?,
        );
        Ok(Self {
            host,
            alive: AtomicBool::new(true),
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
        self.host
            .ui_namespace()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    /// Returns the qualified body occurrence used by the mutation renderer.
    /// The value is consumed only by the package's private React adapter; it
    /// is not a public tree-editing handle or a general native escape hatch.
    #[napi(js_name = "uiBodyHandle")]
    pub fn ui_body_handle(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let handle = self
            .host
            .ui_body_handle()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(serde_json::json!({
            "host_namespace": handle.host_namespace,
            "slot": handle.slot,
            "generation": handle.generation,
            "kind": handle.kind as u32,
        }))
    }

    #[napi(js_name = "uiHistoryUnitIdentity")]
    pub fn ui_history_unit_identity(&self, handle: Vec<u32>) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let identity = self
            .host
            .ui_history_unit_identity(decode_ui_node_handle(&handle)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(identity.map_or(serde_json::Value::Null, |value| {
            serde_json::Value::String(value.to_string())
        }))
    }

    #[napi(js_name = "uiPortMounted")]
    pub fn ui_port_mounted(&self, handle: Vec<u32>) -> Result<bool> {
        ensure_alive(&self.alive)?;
        self.host
            .ui_port_mounted(decode_ui_resource_handle(
                &handle,
                iyon_tui::binding::HandleKind::Port,
            )?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "uiConnectorStatus")]
    pub fn ui_connector_status(&self, handle: Vec<u32>) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let status = self
            .host
            .ui_connector_status(decode_ui_resource_handle(
                &handle,
                iyon_tui::binding::HandleKind::Connector,
            )?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(serde_json::json!({
            "phase": status.phase,
            "requested": status.requested,
            "visible": status.visible,
            "projectedSourceRevision": status
                .projected_source_revision
                .map(|revision| revision.to_string()),
            "error": status.error.map(|error| serde_json::json!({
                "code": error.code,
                "diagnostic": error.diagnostic,
            })),
            "cleanupPending": status.cleanup_pending,
            "cleanupError": status.cleanup_error.map(|error| serde_json::json!({
                "code": error.code,
                "diagnostic": error.diagnostic,
            })),
        }))
    }

    #[napi(js_name = "interceptPasteUi")]
    pub fn intercept_paste_ui(&self, handle: Vec<u32>, route_id: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .intercept_ui_paste(decode_ui_node_handle(&handle)?, route_id)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi(js_name = "waitForUiPresentation")]
    pub async fn wait_for_ui_presentation(
        &self,
        revision: i64,
        content_visible: bool,
    ) -> Result<()> {
        ensure_alive(&self.alive)?;
        let revision = u64::try_from(revision)
            .map_err(|_| crate::NativeError::invalid_input("UI revision must be non-negative"))?;
        self.host
            .wait_for_ui_presentation(revision, content_visible)
            .await
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    /// Waits for one owned asynchronous React event batch.  The native host
    /// owns queueing and stale-generation filtering; this boundary only
    /// converts the detached batch into JS values after the native await.
    #[napi(js_name = "waitForUiEvents")]
    pub async fn wait_for_ui_events(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let events = self
            .host
            .wait_for_ui_events()
            .await
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(match events {
            None => Value::Null,
            Some(events) => serde_json::json!(
                events
                    .into_iter()
                    .map(|event| serde_json::json!({
                        "host_namespace": event.handle.host_namespace,
                        "slot": event.handle.slot,
                        "generation": event.handle.generation,
                        "kind": event.handle.kind as u32,
                        "mask": event.mask,
                        "text": event.text,
                        "cursor_bytes": event.cursor_bytes,
                        "key": event.key,
                        "revision": event.revision,
                    }))
                    .collect::<Vec<_>>()
            ),
        })
    }

    #[napi(js_name = "focusUi")]
    pub fn focus_ui(&self, handle: Vec<u32>) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .focus_ui(decode_ui_node_handle(&handle)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "uiVisibleGeometry")]
    pub fn ui_visible_geometry(&self, handle: Vec<u32>) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let geometry = self
            .host
            .ui_visible_geometry(decode_ui_node_handle(&handle)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(geometry.map_or_else(
            || serde_json::Value::Null,
            |(x, y, width, height)| {
                serde_json::json!({
                    "x": x,
                    "y": y,
                    "width": width,
                    "height": height,
                })
            },
        ))
    }

    #[napi(js_name = "drainUiEvents")]
    pub fn drain_ui_events(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let events = self
            .host
            .drain_ui_events()
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(serde_json::json!(
            events
                .into_iter()
                .map(|event| serde_json::json!({
                    "host_namespace": event.handle.host_namespace,
                    "slot": event.handle.slot,
                    "generation": event.handle.generation,
                    "kind": event.handle.kind as u32,
                    "mask": event.mask,
                    "text": event.text,
                    "cursor_bytes": event.cursor_bytes,
                    "key": event.key,
                    "revision": event.revision,
                }))
                .collect::<Vec<_>>()
        ))
    }

    /// Private downward-only event queue configuration used by focused
    /// admission tests; normal callers use the normative default bounds.
    #[napi(js_name = "setUiEventQueueLimits")]
    pub fn set_ui_event_queue_limits(&self, max_records: i64, max_bytes: i64) -> Result<()> {
        ensure_alive(&self.alive)?;
        let max_records = usize::try_from(max_records)
            .map_err(|_| crate::NativeError::invalid_input("event max records must be positive"))?;
        let max_bytes = usize::try_from(max_bytes)
            .map_err(|_| crate::NativeError::invalid_input("event max bytes must be positive"))?;
        self.host
            .set_ui_event_limits(max_records, max_bytes)
            .map_err(|error| crate::NativeError::invalid_input(error.to_string()))
    }

    #[napi(js_name = "failUiConnectorForTest")]
    pub fn fail_ui_connector_for_test(&self, handle: Vec<u32>, diagnostic: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        if handle.len() != iyon_tui::binding::UI_HANDLE_WORDS {
            return Err(crate::NativeError::invalid_input(
                "UI Connector handle must contain four words",
            ));
        }
        if handle.iter().all(|word| *word == 0) {
            return self
                .host
                .fail_next_ui_connector_for_test(diagnostic)
                .map_err(|error| crate::NativeError::internal(error.to_string()));
        }
        let kind = iyon_tui::binding::HandleKind::from_code(handle[3])
            .ok_or_else(|| crate::NativeError::invalid_input("invalid UI handle kind"))?;
        let namespace = iyon_tui::binding::HostNamespace::new(handle[0])
            .ok_or_else(|| crate::NativeError::invalid_input("invalid UI host namespace"))?;
        let handle = iyon_tui::binding::UiHandle::new(namespace, handle[1], handle[2], kind)
            .ok_or_else(|| crate::NativeError::invalid_input("invalid UI Connector handle"))?;
        self.host
            .fail_ui_connector_for_test(handle, diagnostic)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    /// Explicitly releases the private occurrence document and its owned
    /// content/control resources without tearing down the terminal host.
    /// React uses this only for root-fault cleanup; normal roots unmount
    /// through the acknowledged mutation journal first.
    #[napi(js_name = "closeUiState")]
    pub fn close_ui_state(&self) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .close_ui_state()
            .map_err(|error| crate::NativeError::internal(error.to_string()))
    }

    #[napi(js_name = "waitForUiFailure")]
    pub async fn wait_for_ui_failure(&self) -> Result<Value> {
        ensure_alive(&self.alive)?;
        let failure = self
            .host
            .wait_for_ui_failure()
            .await
            .map_err(|error| crate::NativeError::internal(error.to_string()))?;
        Ok(match failure {
            None => Value::Null,
            Some(failure) => serde_json::json!({
                "phase": failure.phase,
                "code": failure.code,
                "attempted_ui_revision": failure.attempted_ui_revision.to_string(),
                "attempted_work_epoch": failure.attempted_work_epoch.to_string(),
                "diagnostic": failure.diagnostic,
                "retryable": failure.retryable,
            }),
        })
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
            let host_error = self.host.close().err();
            match host_error {
                None => self.alive.store(false, Ordering::Release),
                host_error => {
                    let mut diagnostics = Vec::new();
                    if let Some(error) = host_error {
                        diagnostics.push(format!("host close failed: {error:#}"));
                    }
                    return Err(crate::NativeError::internal(diagnostics.join("; ")));
                }
            }
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

    #[napi]
    pub fn set_theme(&self, value: Value) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .set_theme(theme_dto::decode_theme(value)?)
            .map_err(|error| crate::NativeError::internal(error.to_string()))
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
        Ok(self
            .host
            .style_at(row, column)
            .as_ref()
            .map(cell_style_value))
    }

    #[napi(js_name = "cellXOfText")]
    pub fn cell_x_of_text(&self, row: i64, text: String) -> Result<Option<i64>> {
        ensure_alive(&self.alive)?;
        let row = u16::try_from(row)
            .map_err(|_| crate::NativeError::invalid_input("row must fit in u16"))?;
        Ok(self.host.cell_x_of_text(row, &text).map(i64::from))
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

    #[napi(js_name = "dispatchKey")]
    pub fn dispatch_key(&self, key: String, modifiers: Option<Vec<String>>) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .dispatch_key(parse_key(&key, modifiers.as_deref())?)
            .map_err(native_input_error)
    }

    #[napi(js_name = "dispatchPaste")]
    pub fn dispatch_paste(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host.dispatch_paste(&text).map_err(native_input_error)
    }

    #[napi(js_name = "forwardPaste")]
    pub fn forward_paste(&self, text: String) -> Result<()> {
        ensure_alive(&self.alive)?;
        self.host
            .forward_paste(&text)
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

fn native_input_error(error: impl std::fmt::Display) -> napi::Error {
    let message = error.to_string();
    if let Some(detail) = message.strip_prefix("EVENT_BACKPRESSURE:") {
        return crate::NativeError::coded(
            napi::Status::GenericFailure,
            "ION_EVENT_BACKPRESSURE",
            detail.trim(),
        );
    }
    if let Some(detail) = message.strip_prefix("EVENT_TOO_LARGE:") {
        return crate::NativeError::coded(
            napi::Status::InvalidArg,
            "ION_EVENT_TOO_LARGE",
            detail.trim(),
        );
    }
    crate::NativeError::internal(message)
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

fn cell_style_value(style: &HostCellStyle) -> Value {
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

/// Canonical color-string decoder for the native theme DTO string lane. The
/// TypeScript packer normalizes `{type: "ansi", value}` objects to `ansi:N`
/// strings; the theme DTO handles object shapes separately and both terminate
/// in identical values.
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
