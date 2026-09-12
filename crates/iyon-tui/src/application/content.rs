//! Retained content-plane identities, Source storage, and inactive control state.
//!
//! Source storage is deliberately host-independent. This module owns the
//! PERF-13-E mutation boundary and the PERF-13-D lifecycle graph: environment-
//! owned Sources, host-owned Ports and Connectors, desired/visible mount state,
//! weak subscription bookkeeping, and the plain-text Connector projection.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::str;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel},
};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    geometry::Size,
    physical::{PhysicalCell, PhysicalRow, PhysicalStyle, Surface},
    presentation::{
        ContentDirty, ContentDirtyReason, ContentMeasurement, ContentMeasurementCapture,
        ContentProvider, ContentWindow, HistoryContentRows, HistoryMeasurementAdjustment,
        PreparedProjectionTicket,
    },
    projection::{Projection, ProjectionBuilder, Projector, Smooth, SmoothConfig},
    stream::{StreamOffset, StreamRange},
    text::{
        AnsiProjector, Block, DiffProjector, Inline, InlineContent, InlineKind, LiteralText,
        MarkdownOptions, MarkdownProjector, PlainTextProjector, RawText, TerminalRowWindow,
        TextContent, TextProjectionError, TextProvenance, TextRewriter, TextRun,
        walk_rewrite_block, walk_rewrite_inline,
    },
    {AnsiColor, ColorSpec, StyleRef, StyleSpec, TextAttribute, Theme},
};
use anyhow::{Result, anyhow};

use super::environment::{EnvironmentIdentity, WakeDisposition};
use super::host::HostInner;
use super::source_store::{
    CONTENT_ANNOTATION_KIND_ATOMIC, CONTENT_ANNOTATION_KIND_POINT, CONTENT_ANNOTATION_KIND_STYLE,
    CONTENT_ANNOTATION_KIND_TAG, ChunkView, MAX_ANNOTATION_PAYLOAD_BYTES, MAX_SOURCE_ANNOTATIONS,
    MAX_SOURCE_PAYLOAD_BYTES, SourceAnnotation, StoredSource, ValidatedAnnotation, ValidatedInput,
};
use super::ui_resources::UiResourceOwner;
use crate::occurrence::{HandleKind, ResourceKey, UiChangeSet};

/// The content executor is deliberately shared by every Source in one native
/// environment.  It owns the bounded handoff, while the registry remains the
/// authority for which Connector is selected and which product is visible.
///
/// A single worker is intentional for this first cutover: it gives semantic
/// parser state a strict Source order without introducing one thread per
/// Source, and the bounded queue still keeps content work away from the host
/// acceptance lock. A large projection therefore serializes other content
/// jobs in this tranche; no preemption or incremental parser checkpoint API is
/// claimed. The command/result contract is typed so a future fixed worker set
/// can preserve the same ownership boundary.
#[derive(Debug)]
struct ContentExecutor {
    commands: SyncSender<ContentExecutorCommand>,
    accounting: Arc<Mutex<ContentExecutorAccounting>>,
    semantic_cache: Arc<Mutex<SemanticProjectionCache>>,
    parser_states: Arc<Mutex<VecDeque<(ParserExecutionKey, ParserExecution)>>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    startup_error: Option<Arc<str>>,
    join: Mutex<Option<JoinHandle<()>>>,
    #[cfg(test)]
    projection_latch: Arc<Mutex<Option<TestContentLatch>>>,
}

/// Accounting and waiter registration share one mutex. A capacity probe and
/// a waiter registration therefore form one transition: a worker cannot
/// return capacity between the probe and the registration and strand an
/// owner.
struct ContentExecutorAccounting {
    queued_jobs: usize,
    queued_bytes: usize,
    waiters: VecDeque<(u64, Arc<dyn Fn() + Send + Sync>)>,
    next_waiter_id: u64,
}

impl Default for ContentExecutorAccounting {
    fn default() -> Self {
        Self {
            queued_jobs: 0,
            queued_bytes: 0,
            waiters: VecDeque::new(),
            next_waiter_id: 0,
        }
    }
}

impl std::fmt::Debug for ContentExecutorAccounting {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContentExecutorAccounting")
            .field("queued_jobs", &self.queued_jobs)
            .field("queued_bytes", &self.queued_bytes)
            .field("waiter_count", &self.waiters.len())
            .finish()
    }
}

#[cfg(test)]
#[derive(Debug)]
struct TestContentLatch {
    entered: std::sync::mpsc::Sender<()>,
    release: Arc<Mutex<std::sync::mpsc::Receiver<()>>>,
}

enum ContentExecutorCommand {
    Run(ContentExecutorJob),
}

const CONTENT_EXECUTOR_MAX_JOBS: usize = 32;
const CONTENT_EXECUTOR_MAX_BYTES: usize = 64 * 1024 * 1024;
const CONTENT_EXECUTOR_PARSER_STATE_CAPACITY: usize = 16;

struct ContentExecutorJob {
    bytes: usize,
    run: Box<dyn FnOnce() + Send + 'static>,
}

impl std::fmt::Debug for ContentExecutorJob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContentExecutorJob")
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
enum ContentExecutorRejectKind {
    Startup,
    Oversized,
    Unavailable,
}

#[derive(Debug)]
struct ContentExecutorRejected {
    kind: ContentExecutorRejectKind,
    diagnostic: String,
}

impl ContentExecutorRejected {
    fn new(kind: ContentExecutorRejectKind, diagnostic: impl Into<String>) -> Self {
        Self {
            kind,
            diagnostic: diagnostic.into(),
        }
    }
}

impl std::fmt::Display for ContentExecutorRejected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ContentExecutorRejected {}

/// An admitted queue slot remains owned until the command is actually sent.
/// This closes the fallible gap between capacity reservation and Connector
/// execution capture: poisoned state or any other early return cannot leak a
/// slot from the shared executor budget.
struct ContentExecutorPermit {
    accounting: Arc<Mutex<ContentExecutorAccounting>>,
    bytes: usize,
    committed: bool,
}

impl ContentExecutorPermit {
    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for ContentExecutorPermit {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut accounting = self
            .accounting
            .lock()
            .expect("content executor accounting lock must remain usable");
        accounting.queued_jobs = accounting
            .queued_jobs
            .checked_sub(1)
            .expect("content executor queued job accounting underflow");
        accounting.queued_bytes = accounting
            .queued_bytes
            .checked_sub(self.bytes)
            .expect("content executor queued byte accounting underflow");
        drop(accounting);
        wake_content_executor_waiters(&self.accounting);
    }
}

impl ContentExecutor {
    fn new() -> Result<Arc<Self>> {
        let (commands, receive) = sync_channel(CONTENT_EXECUTOR_MAX_JOBS);
        let accounting = Arc::new(Mutex::new(ContentExecutorAccounting::default()));
        let accounting_for_worker = Arc::clone(&accounting);
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_for_worker = Arc::clone(&shutdown);
        let join = thread::Builder::new()
            .name("iyon-tui-content".to_owned())
            .spawn(move || {
                content_executor_loop(receive, accounting_for_worker, shutdown_for_worker)
            })
            .map_err(|error| anyhow!("CONTENT_EXECUTOR_STARTUP_FAILED: {error}"))?;
        Ok(Arc::new(Self {
            commands,
            accounting,
            semantic_cache: Arc::new(Mutex::new(VecDeque::new())),
            parser_states: Arc::new(Mutex::new(VecDeque::new())),
            shutdown,
            startup_error: None,
            join: Mutex::new(Some(join)),
            #[cfg(test)]
            projection_latch: Arc::new(Mutex::new(None)),
        }))
    }

    fn failed(diagnostic: String) -> Arc<Self> {
        let (commands, _receive) = sync_channel(CONTENT_EXECUTOR_MAX_JOBS);
        Arc::new(Self {
            commands,
            accounting: Arc::new(Mutex::new(ContentExecutorAccounting::default())),
            semantic_cache: Arc::new(Mutex::new(VecDeque::new())),
            parser_states: Arc::new(Mutex::new(VecDeque::new())),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            startup_error: Some(Arc::from(diagnostic)),
            join: Mutex::new(None),
            #[cfg(test)]
            projection_latch: Arc::new(Mutex::new(None)),
        })
    }

    fn reserve_projection(
        &self,
        bytes: usize,
        waiter: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<(Option<ContentExecutorPermit>, Option<u64>), ContentExecutorRejected> {
        if let Some(error) = self.startup_error.as_ref() {
            return Err(ContentExecutorRejected::new(
                ContentExecutorRejectKind::Startup,
                error.to_string(),
            ));
        }
        if bytes > CONTENT_EXECUTOR_MAX_BYTES {
            return Err(ContentExecutorRejected::new(
                ContentExecutorRejectKind::Oversized,
                format!(
                    "CONTENT_LIMIT_EXCEEDED: content projection requires {bytes} bytes, exceeding the executor limit of {CONTENT_EXECUTOR_MAX_BYTES}"
                ),
            ));
        }
        let mut accounting = self
            .accounting
            .lock()
            .expect("content executor accounting lock must remain usable");
        let next_jobs = accounting.queued_jobs.saturating_add(1);
        let next_bytes = accounting.queued_bytes.saturating_add(bytes);
        if next_jobs <= CONTENT_EXECUTOR_MAX_JOBS && next_bytes <= CONTENT_EXECUTOR_MAX_BYTES {
            accounting.queued_jobs = next_jobs;
            accounting.queued_bytes = next_bytes;
            return Ok((
                Some(ContentExecutorPermit {
                    accounting: Arc::clone(&self.accounting),
                    bytes,
                    committed: false,
                }),
                None,
            ));
        }
        let waiter_id = match waiter {
            Some(waiter) => {
                let waiter_id = accounting
                    .next_waiter_id
                    .checked_add(1)
                    .expect("content executor waiter identity exhausted");
                accounting.next_waiter_id = waiter_id;
                accounting.waiters.push_back((waiter_id, waiter));
                Some(waiter_id)
            }
            None => None,
        };
        Ok((None, waiter_id))
    }

    fn submit_projection(
        &self,
        mut permit: ContentExecutorPermit,
        job: ContentProjectionTask,
    ) -> Result<(), ContentExecutorRejected> {
        debug_assert_eq!(permit.bytes, job.bytes);
        let semantic_cache = Arc::clone(&self.semantic_cache);
        let parser_states = Arc::clone(&self.parser_states);
        let wake = job.wake.clone();
        #[cfg(test)]
        let projection_latch = Arc::clone(&self.projection_latch);
        let task = ContentExecutorJob {
            bytes: job.bytes,
            run: Box::new(move || {
                let mut job = job;
                let parser_key = ParserExecutionKey::for_snapshot(&job.snapshot, job.funnel);
                let parser = take_parser_execution(&parser_states, parser_key, &mut job.execution);
                parser.install_into(&mut job.execution);
                #[cfg(test)]
                if let Some(latch) = projection_latch
                    .lock()
                    .expect("content test latch lock must remain usable")
                    .as_ref()
                {
                    let _ = latch.entered.send(());
                    let _ = latch
                        .release
                        .lock()
                        .expect("content test latch release lock must remain usable")
                        .recv();
                }
                if job.cancelled.load(Ordering::Acquire) {
                    wake();
                    return;
                }
                let projection = {
                    let mut semantic_cache = semantic_cache
                        .lock()
                        .expect("content semantic cache lock must remain usable");
                    project_text_snapshot(
                        &job.snapshot,
                        job.funnel,
                        job.offered_width,
                        job.needs_finalized_prefix,
                        &job.theme,
                        job.theme_revision,
                        &mut job.execution,
                        job.delivery_revision,
                        &mut semantic_cache,
                        &mut job.prefix_proof_cache,
                    )
                };
                let parser = ParserExecution::from_execution(&mut job.execution);
                let mut parser_states = parser_states
                    .lock()
                    .expect("content parser state lock must remain usable");
                parser_states.retain(|(candidate, _)| *candidate != parser_key);
                parser_states.push_front((parser_key, parser));
                while parser_states.len() > CONTENT_EXECUTOR_PARSER_STATE_CAPACITY {
                    parser_states.pop_back();
                }
                drop(parser_states);
                let _ = job.result.send(ContentProjectionResult {
                    connector_id: job.connector_id,
                    key: job.key,
                    projection,
                    execution: job.execution,
                    prefix_proof_cache: job.prefix_proof_cache,
                });
                wake();
            }),
        };
        match self.commands.try_send(ContentExecutorCommand::Run(task)) {
            Ok(()) => {
                // The permit remains armed through the fallible send. A
                // disconnected worker therefore returns the exact slot via
                // its Drop implementation instead of relying on a second
                // rollback path.
                permit.commit();
                Ok(())
            }
            Err(error) => {
                let diagnostic = format!(
                    "CONTENT_EXECUTOR_UNAVAILABLE: content executor queue is unavailable: {error}"
                );
                match error {
                    TrySendError::Full(_) | TrySendError::Disconnected(_) => {}
                }
                Err(ContentExecutorRejected {
                    kind: ContentExecutorRejectKind::Unavailable,
                    diagnostic,
                })
            }
        }
    }

    #[cfg(test)]
    fn install_projection_latch(
        &self,
    ) -> (std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>) {
        let (entered, entered_receive) = std::sync::mpsc::channel();
        let (release, release_receive) = std::sync::mpsc::channel();
        *self
            .projection_latch
            .lock()
            .expect("content test latch lock must remain usable") = Some(TestContentLatch {
            entered,
            release: Arc::new(Mutex::new(release_receive)),
        });
        (entered_receive, release)
    }

    #[cfg(test)]
    fn clear_projection_latch(&self) {
        *self
            .projection_latch
            .lock()
            .expect("content test latch lock must remain usable") = None;
    }

    fn unregister_waiter(&self, waiter_id: u64) {
        self.accounting
            .lock()
            .expect("content executor accounting lock must remain usable")
            .waiters
            .retain(|(candidate, _)| *candidate != waiter_id);
    }
}

fn wake_content_executor_waiters(accounting: &Arc<Mutex<ContentExecutorAccounting>>) {
    let waiters = accounting
        .lock()
        .expect("content executor accounting lock must remain usable")
        .waiters
        .iter()
        .map(|(_, waiter)| waiter)
        .cloned()
        .collect::<Vec<_>>();
    for waiter in waiters {
        waiter();
    }
}

/// Immutable input captured by a Connector owner before a projection enters
/// the executor.  In particular, `snapshot` owns the Source storage Arc, so
/// Source and HostInner locks are both released before this task runs.
struct ContentProjectionTask {
    bytes: usize,
    connector_id: u64,
    key: TextProjectionKey,
    result: std::sync::mpsc::SyncSender<ContentProjectionResult>,
    snapshot: HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    offered_width: u16,
    needs_finalized_prefix: bool,
    theme: Arc<Theme>,
    theme_revision: u64,
    delivery_revision: u64,
    execution: ConnectorExecution,
    prefix_proof_cache: PrefixProofCache,
    wake: Arc<dyn Fn() + Send + Sync>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

struct ContentProjectionAdmission<'a> {
    connector: &'a Arc<Mutex<ConnectorRecord>>,
    connector_id: u64,
    key: TextProjectionKey,
    snapshot: &'a HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    offered_width: u16,
    delivery_revision: u64,
    needs_finalized_prefix: bool,
    task_bytes: usize,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ParserExecutionKey {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_base: u64,
    funnel_kind: TextFunnelKind,
    hyperlinks: bool,
}

impl ParserExecutionKey {
    fn for_snapshot(snapshot: &HostContentSourceSnapshot, funnel: HostContentFunnel) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_base: snapshot.source_base,
            funnel_kind: funnel.kind,
            hyperlinks: funnel.hyperlinks,
        }
    }
}

#[derive(Debug, Default)]
struct ParserExecution {
    markdown: Option<MarkdownProjector>,
    diff: Option<DiffProjector>,
    ansi: Option<AnsiProjector>,
    parser_lineage: Option<ContentLineage>,
}

impl ParserExecution {
    fn from_execution(execution: &mut ConnectorExecution) -> Self {
        Self {
            markdown: execution.markdown.take(),
            diff: execution.diff.take(),
            ansi: execution.ansi.take(),
            parser_lineage: execution.parser_lineage,
        }
    }

    fn install_into(self, execution: &mut ConnectorExecution) {
        execution.markdown = self.markdown;
        execution.diff = self.diff;
        execution.ansi = self.ansi;
        execution.parser_lineage = self.parser_lineage;
    }
}

fn take_parser_execution(
    states: &Mutex<VecDeque<(ParserExecutionKey, ParserExecution)>>,
    key: ParserExecutionKey,
    fallback: &mut ConnectorExecution,
) -> ParserExecution {
    let mut states = states
        .lock()
        .expect("content parser state lock must remain usable");
    states
        .iter()
        .position(|(candidate, _)| *candidate == key)
        .and_then(|index| states.remove(index).map(|(_, parser)| parser))
        .unwrap_or_else(|| ParserExecution::from_execution(fallback))
}

struct ContentProjectionResult {
    connector_id: u64,
    key: TextProjectionKey,
    projection: Result<HostContentProjection>,
    execution: ConnectorExecution,
    prefix_proof_cache: PrefixProofCache,
}

#[derive(Debug)]
struct PendingContentProjection {
    key: TextProjectionKey,
    /// `None` means this Connector is waiting for executor admission. The
    /// pending marker retains no Source snapshot or parser execution; the
    /// next owner turn captures fresh input after capacity is reserved.
    result: Option<Receiver<ContentProjectionResult>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
struct ContentWorkerWake(Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for ContentWorkerWake {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContentWorkerWake(..)")
    }
}

fn content_executor_loop(
    receive: Receiver<ContentExecutorCommand>,
    accounting: Arc<Mutex<ContentExecutorAccounting>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
) {
    while !shutdown.load(Ordering::Acquire) {
        let command = match receive.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match command {
            ContentExecutorCommand::Run(job) => {
                {
                    let mut accounting_guard = accounting
                        .lock()
                        .expect("content executor accounting lock must remain usable");
                    accounting_guard.queued_jobs = accounting_guard
                        .queued_jobs
                        .checked_sub(1)
                        .expect("content executor queued job accounting underflow");
                    accounting_guard.queued_bytes = accounting_guard
                        .queued_bytes
                        .checked_sub(job.bytes)
                        .expect("content executor queued byte accounting underflow");
                }
                // Capacity returns when the command leaves the bounded
                // handoff, before projection work runs. Wake every live
                // owner; the environment queue supplies the fair one-turn
                // ordering and each owner coalesces its own Connector work.
                wake_content_executor_waiters(&accounting);
                (job.run)();
            }
        }
    }
}

impl Drop for ContentExecutor {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(join) = self
            .join
            .lock()
            .expect("content executor join lock must remain usable")
            .take()
        {
            let _ = join.join();
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContentFamily {
    Text,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextSourceKind {
    Block,
    Stream,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextFunnelKind {
    Plain,
    Markdown,
    Diff,
    Ansi,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContentDelivery {
    Immediate,
    Smooth(SmoothConfig),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TextWrapMode {
    Word,
    Grapheme,
    NoWrap,
}

/// Fixed-width annotation envelope shared by the direct data ABI and the
/// native Source store. Offsets are operation-local UTF-8 byte coordinates;
/// the Source converts them to absolute coordinates while holding its mutex.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentAnnotationRecord {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub payload_offset: u32,
    pub payload_length: u32,
    pub aux0: u32,
    pub aux1: u32,
}

const _: () = assert!(std::mem::size_of::<ContentAnnotationRecord>() == 32);
const _: () = assert!(std::mem::align_of::<ContentAnnotationRecord>() == 4);

/// Read-only annotation data exposed by a diagnostic Source snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentAnnotationSnapshot {
    pub kind: u32,
    pub flags: u32,
    pub start_byte: u64,
    pub end_byte: u64,
    pub payload: Vec<u8>,
    pub aux0: u32,
    pub aux1: u32,
}

/// Immutable, cheap-to-clone Source snapshot. The text/chunk storage is
/// shared by Arc; `text()` is an explicit diagnostic/materialization query and
/// is not used by the frame path.
#[derive(Clone, Debug)]
pub struct HostContentSourceSnapshot {
    pub source_id: u64,
    pub source_generation: u32,
    pub content_generation: u64,
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub sealed: bool,
    pub head_partial: bool,
    storage: Arc<StoredSource>,
}

impl HostContentSourceSnapshot {
    #[must_use]
    pub fn text(&self) -> String {
        self.storage.text()
    }

    #[must_use]
    pub fn annotations(&self) -> Vec<ContentAnnotationSnapshot> {
        self.storage
            .annotations_in_order()
            .iter()
            .map(|annotation| ContentAnnotationSnapshot {
                kind: annotation.kind,
                flags: annotation.flags,
                start_byte: annotation.start_byte,
                end_byte: annotation.end_byte,
                payload: annotation.payload.to_vec(),
                aux0: annotation.aux0,
                aux1: annotation.aux1,
            })
            .collect()
    }

    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        self.source_end.saturating_sub(self.source_base)
    }

    #[must_use]
    pub fn retained_lines(&self) -> u64 {
        self.storage.line_count() as u64
    }

    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.storage.chunk_count()
    }

    pub(crate) fn chunk_views(&self) -> Vec<ChunkView> {
        self.storage.chunk_views()
    }

    fn chunks(&self) -> impl Iterator<Item = (&[u8], u64)> {
        self.storage.iter_chunks()
    }

    fn annotations_for_projection(&self) -> &[SourceAnnotation] {
        self.storage.annotations_in_order()
    }

    fn stable_prefix(&self) -> Option<Self> {
        let storage = self.storage.stable_prefix()?;
        Some(Self {
            source_id: self.source_id,
            source_generation: self.source_generation,
            content_generation: self.content_generation,
            revision: self.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            storage: Arc::new(storage),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContentLineage {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_base: u64,
}

impl ContentLineage {
    fn from_snapshot(snapshot: &HostContentSourceSnapshot) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_base: snapshot.source_base,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TextProjectionKey {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_revision: u64,
    source_base: u64,
    source_end: u64,
    head_partial: bool,
    width: u16,
    wrap: TextWrapMode,
    funnel_kind: TextFunnelKind,
    delivery_revision: u64,
    theme_revision: u64,
    needs_finalized_prefix: bool,
    needs_physical_rows: bool,
}

impl TextProjectionKey {
    fn revision(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn metric_revision(self, size: Size, complete: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.width.hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.funnel_kind.hash(&mut hasher);
        self.needs_finalized_prefix.hash(&mut hasher);
        size.width.hash(&mut hasher);
        size.height.hash(&mut hasher);
        complete.hash(&mut hasher);
        hasher.finish()
    }

    fn layout_input_revision(self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.source_id.hash(&mut hasher);
        self.source_generation.hash(&mut hasher);
        self.source_revision.hash(&mut hasher);
        self.width.hash(&mut hasher);
        self.wrap.hash(&mut hasher);
        self.funnel_kind.hash(&mut hasher);
        // Delivery can change the visible intrinsic height even when Source
        // bytes and width are unchanged. It is a layout input, while the
        // metric revision below still records whether geometry actually
        // changed after evaluation.
        self.delivery_revision.hash(&mut hasher);
        hasher.finish()
    }
}

// Retain a small current/prior working set. The previous product covers an
// in-flight/rollback retry and the newest matching append prefix is preferred;
// older, otherwise-valid keys may be recomputed after eviction. Replacement,
// truncation, width, and theme changes carry distinct keys, so a large
// historical set increases live layout memory for limited reuse benefit.
const CONTENT_CACHE_CAPACITY: usize = 2;
const CONTENT_PREFIX_CACHE_CAPACITY: usize = 2;

/// Key identifying a cached semantic IR projection. The semantic IR is
/// independent of theme, width, delivery tick, and viewport.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SemanticProjectionKey {
    source_id: u64,
    source_generation: u32,
    content_generation: u64,
    source_revision: u64,
    source_base: u64,
    source_end: u64,
    sealed: bool,
    head_partial: bool,
    funnel_kind: TextFunnelKind,
    hyperlinks: bool,
}

impl SemanticProjectionKey {
    fn for_snapshot(snapshot: &HostContentSourceSnapshot, funnel: HostContentFunnel) -> Self {
        Self {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            sealed: snapshot.sealed,
            head_partial: snapshot.head_partial,
            funnel_kind: funnel.kind,
            hyperlinks: funnel.hyperlinks,
        }
    }
}

type SemanticProjectionCache = VecDeque<(SemanticProjectionKey, Arc<Projection<TextContent>>)>;

/// Resolves one semantic projection through the Connector cache, building
/// and retaining it only on a miss. Theme-only changes always hit: parsers
/// never re-run for a recolor.
fn resolve_cached_semantic(
    cache: &mut SemanticProjectionCache,
    key: SemanticProjectionKey,
    build: impl FnOnce() -> Result<Projection<TextContent>>,
) -> Result<Arc<Projection<TextContent>>> {
    if let Some(hit) = cache
        .iter()
        .find(|(candidate, _)| candidate == &key)
        .map(|(_, projection)| Arc::clone(projection))
    {
        return Ok(hit);
    }
    crate::perf::inc(crate::perf::Counter::SemanticProjectionRebuilds);
    let built = Arc::new(build()?);
    cache.retain(|(candidate, _)| candidate != &key);
    cache.push_front((key, Arc::clone(&built)));
    while cache.len() > CONTENT_CACHE_CAPACITY {
        cache.pop_back();
    }
    Ok(built)
}

#[derive(Clone, Debug)]
struct HostContentProjection {
    /// Monotonic product identity used by prepared tickets.  This is not an
    /// allocator address and therefore cannot suffer pointer ABA after cache
    /// eviction/reuse.
    identity: u64,
    key: TextProjectionKey,
    source_snapshot: HostContentSourceSnapshot,
    /// Width-specific terminal realization. The producer owns semantic
    /// layout, row boundaries, styles, and provenance; this registry only
    /// selects and retains the immutable product for its Connector.
    product: Arc<crate::text::TerminalTextProduct>,
    /// The selected semantic values and policy are retained with the product
    /// so the renderer driver can request another pure definite-width
    /// realization without consulting Source/Connector state.
    semantic_contents: Arc<[TextContent]>,
    terminal_policy: crate::text::TextRenderPolicy,
    intrinsic_size: Size,
    physically_complete: bool,
    min_content: Size,
    max_content: Size,
    /// Immediate non-History projections may defer physical row lowering to
    /// the prepared-ticket window. Smooth/History products retain rows for
    /// reveal and scrollback semantics.
    rows: Option<Arc<Vec<PhysicalRow>>>,
    /// Immutable palette captured with this projection. Deferred row-window
    /// painting must not consult the Connector's newer host theme.
    theme: Arc<Theme>,
    /// Physical rows produced by the established finalized-prefix proof.
    /// This is a distinct product from the open document rows: Markdown may
    /// render a prefix differently while its trailing block remains open.
    finalized_prefix: Option<Arc<FinalizedPrefixProduct>>,
    stable_rows: usize,
    visible_row_count: usize,
    cut: Option<(u16, u16)>,
}

/// Mutable delivery state that belongs to one smoothed Connector binding.
/// Delivery tracks time advancement, grapheme indexing, and candidate frontiers
/// without reconstructing raw Source projections or parsing syntax on pure ticks.
#[derive(Debug)]
struct ConnectorDelivery {
    smoother: Smooth,
    units: Projection<TextContent>,
    indexed_generation: u32,
    indexed_revision: u64,
    indexed_sealed: bool,
    candidate_frontier: StreamOffset,
}

impl ConnectorDelivery {
    fn new(config: SmoothConfig) -> Self {
        Self {
            smoother: Smooth::new(config),
            units: ProjectionBuilder::new(
                StreamOffset::ZERO,
                StreamOffset::ZERO,
                StreamOffset::ZERO,
                false,
            )
            .finish()
            .expect("empty grapheme projection is valid"),
            indexed_generation: u32::MAX,
            indexed_revision: u64::MAX,
            indexed_sealed: false,
            candidate_frontier: StreamOffset::ZERO,
        }
    }

    fn accept_input(&mut self, snapshot: &HostContentSourceSnapshot) -> Result<()> {
        let changed = self.indexed_generation != snapshot.source_generation
            || self.indexed_revision != snapshot.revision
            || self.indexed_sealed != snapshot.sealed;
        if !changed {
            return Ok(());
        }
        let units = source_grapheme_projection(snapshot)
            .map_err(|error| anyhow!("content smoothing input failed: {error}"))?;
        let _ = self.smoother.project(&units);
        self.units = units;
        self.indexed_generation = snapshot.source_generation;
        self.indexed_revision = snapshot.revision;
        self.indexed_sealed = snapshot.sealed;
        self.candidate_frontier = self.smoother.published_through();
        Ok(())
    }

    fn advance(&mut self, now: Instant) -> bool {
        let progressed = self.smoother.advance(now);
        if progressed {
            self.candidate_frontier = self.smoother.published_through();
        }
        progressed
    }

    fn published_through(&self) -> StreamOffset {
        self.smoother.published_through()
    }

    fn reveal_units(&self) -> usize {
        let published = self.published_through();
        let spans = self.units.spans();
        spans.partition_point(|span| span.source().end() <= published)
    }
}

/// Mutable execution state that belongs to one Connector binding. Funnels
/// remain immutable specifications; inactive Connectors drop this value so
/// inactive membership retains no parser, delivery, or projection work.
#[derive(Debug)]
struct ConnectorExecution {
    markdown: Option<MarkdownProjector>,
    diff: Option<DiffProjector>,
    ansi: Option<AnsiProjector>,
    parser_lineage: Option<ContentLineage>,
    delivery: Option<ConnectorDelivery>,
}

impl ConnectorExecution {
    fn new(funnel: &HostContentFunnel) -> Self {
        Self {
            markdown: matches!(funnel.kind, TextFunnelKind::Markdown).then(|| {
                MarkdownProjector::new(MarkdownOptions::gfm().with_live_table_stabilization(true))
            }),
            diff: matches!(funnel.kind, TextFunnelKind::Diff).then(DiffProjector::new),
            ansi: matches!(funnel.kind, TextFunnelKind::Ansi).then(|| {
                AnsiProjector::new(crate::text::AnsiOptions {
                    hyperlinks: funnel.hyperlinks,
                })
            }),
            parser_lineage: None,
            delivery: funnel.smooth_config().map(ConnectorDelivery::new),
        }
    }

    fn prepare_for_snapshot(&mut self, snapshot: &HostContentSourceSnapshot) {
        let lineage = ContentLineage::from_snapshot(snapshot);
        if self.parser_lineage == Some(lineage) {
            return;
        }
        if self.parser_lineage.is_some() {
            // Replacement/clear starts a new logical document even when the
            // retained byte range happens to have the same coordinates. Only
            // parser state is lineage-bound; Smooth keeps its existing
            // replacement policy and terminal products are rebuilt from the
            // new immutable semantic lineage.
            self.markdown = None;
            self.diff = None;
            self.ansi = None;
        }
        self.parser_lineage = Some(lineage);
    }
}

impl HostContentProjection {
    fn measurement(&self, connector_id: u64) -> ContentMeasurement {
        ContentMeasurement {
            intrinsic_size: self.intrinsic_size,
            physically_complete: self.physically_complete,
            projection_revision: self.key.revision(),
            metric_revision: self
                .key
                .metric_revision(self.intrinsic_size, self.physically_complete),
            paint_revision: self.key.revision(),
            connector_id: Some(connector_id),
            projection_identity: self.identity,
            source_id: self.source_snapshot.source_id,
            source_generation: self.source_snapshot.source_generation,
            content_generation: self.source_snapshot.content_generation,
            source_base: self.source_snapshot.source_base,
            source_end: self.source_snapshot.source_end,
            sealed: self.source_snapshot.sealed,
            head_partial: self.source_snapshot.head_partial,
        }
    }
}

#[derive(Clone, Debug)]
struct CapturedProjection {
    connector_id: u64,
    source_snapshot: HostContentSourceSnapshot,
    product: Arc<HostContentProjection>,
}

#[derive(Clone, Debug)]
enum CapturedCandidate {
    None,
    Prepared(CapturedProjection),
    Failed {
        connector_id: u64,
        source_snapshot: HostContentSourceSnapshot,
    },
}

#[derive(Clone, Debug)]
struct CandidateContentCapture {
    port_id: u64,
    candidate: CapturedCandidate,
    confirmed: Option<CapturedProjection>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostContentSourceStats {
    pub revision: u64,
    pub source_base: u64,
    pub source_end: u64,
    pub retained_bytes: u64,
    pub retained_lines: u64,
    pub chunk_count: usize,
    pub sealed: bool,
    pub head_partial: bool,
    pub accepted_bytes: u64,
    pub copied_bytes: u64,
    pub dropped_head_bytes: u64,
}

/// Result returned by every successful Source data mutation. The wake bit is
/// only a scheduler hint; native host epochs remain authoritative. A
/// post-acceptance host wake failure is reported by the environment's next
/// drain report without changing this successful result.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentMutationResult {
    pub revision: u64,
    pub environment_wake_epoch: u64,
    pub schedule_environment_drain: bool,
}

const MAX_CONTENT_PROJECTION_ROWS: u64 = u16::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentProjectionFailureKind {
    LimitExceeded,
    RetentionIncompatible,
    Projection,
    ExecutorUnavailable,
}

impl ContentProjectionFailureKind {
    const fn code(self) -> &'static str {
        match self {
            Self::LimitExceeded => "LIMIT_EXCEEDED",
            Self::RetentionIncompatible => "RETENTION_INCOMPATIBLE",
            Self::Projection => "PROJECTION_FAILED",
            Self::ExecutorUnavailable => "CONTENT_EXECUTOR_UNAVAILABLE",
        }
    }
}

#[derive(Debug)]
struct ContentProjectionFailure {
    kind: ContentProjectionFailureKind,
    diagnostic: String,
}

#[derive(Debug)]
pub(super) struct ContentProjectionPending;

impl std::fmt::Display for ContentProjectionPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CONTENT_PENDING: content projection is running")
    }
}

impl std::error::Error for ContentProjectionPending {}

impl std::fmt::Display for ContentProjectionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ContentProjectionFailure {}

static NEXT_CONTENT_PROJECTION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CONTENT_CAPTURE_ID: AtomicU64 = AtomicU64::new(1);

fn next_content_projection_id() -> u64 {
    NEXT_CONTENT_PROJECTION_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("content projection identity exhausted")
}

fn next_content_capture_id() -> u64 {
    NEXT_CONTENT_CAPTURE_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("content capture identity exhausted")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnnotationTruncationPolicy {
    Clip,
    Drop,
    Point,
}

fn projected_bounds(
    snapshot: &HostContentSourceSnapshot,
    wrap: TextWrapMode,
    offered_width: u16,
) -> (u64, u64) {
    // Zero-width content is a real terminal constraint. It preserves hard
    // line boundaries instead of being silently widened to one cell.
    let width = u64::from(offered_width);
    let mut rows = 0u64;
    let mut line_bytes = 0u64;
    let mut max_line_bytes = 0u64;
    for (bytes, _) in snapshot.chunks() {
        for byte in bytes {
            if *byte == b'\n' {
                max_line_bytes = max_line_bytes.max(line_bytes);
                rows = if wrap == TextWrapMode::NoWrap || offered_width == 0 {
                    rows.saturating_add(1)
                } else {
                    rows.saturating_add(line_bytes.div_ceil(width).max(1))
                };
                line_bytes = 0;
            } else {
                line_bytes = line_bytes.saturating_add(1);
            }
        }
    }
    max_line_bytes = max_line_bytes.max(line_bytes);
    rows = if wrap == TextWrapMode::NoWrap || offered_width == 0 {
        rows.saturating_add(1)
    } else {
        rows.saturating_add(line_bytes.div_ceil(width).max(1))
    };
    (rows, max_line_bytes)
}

fn annotation_policy(kind: u32) -> AnnotationTruncationPolicy {
    match kind {
        CONTENT_ANNOTATION_KIND_TAG | CONTENT_ANNOTATION_KIND_STYLE => {
            AnnotationTruncationPolicy::Clip
        }
        CONTENT_ANNOTATION_KIND_ATOMIC => AnnotationTruncationPolicy::Drop,
        CONTENT_ANNOTATION_KIND_POINT => AnnotationTruncationPolicy::Point,
        _ => AnnotationTruncationPolicy::Drop,
    }
}

fn source_projection(
    snapshot: &HostContentSourceSnapshot,
) -> Result<Projection<TextContent>, TextProjectionError> {
    let mut builder = ProjectionBuilder::new(
        StreamOffset::new(snapshot.source_base),
        StreamOffset::new(snapshot.source_end),
        StreamOffset::new(snapshot.source_end),
        snapshot.sealed,
    );
    for view in snapshot.chunk_views() {
        let raw = RawText::from_page_slice(view.page, view.page_start, view.len);
        let end = view.abs_start.saturating_add(u64::from(view.len));
        builder = builder.emit(
            StreamRange::new(StreamOffset::new(view.abs_start), StreamOffset::new(end)),
            TextContent::Raw(raw),
        );
    }
    builder.finish().map_err(TextProjectionError::Projection)
}

fn source_grapheme_projection(
    snapshot: &HostContentSourceSnapshot,
) -> Result<Projection<TextContent>, TextProjectionError> {
    let mut builder = ProjectionBuilder::new(
        StreamOffset::new(snapshot.source_base),
        StreamOffset::new(snapshot.source_end),
        StreamOffset::new(snapshot.source_end),
        snapshot.sealed,
    );
    let mut carry: Option<(u64, String)> = None;
    for (bytes, start) in snapshot.chunks() {
        let text = str::from_utf8(bytes).expect("Source snapshot chunks are valid UTF-8");
        let (combined_start, mut combined) = match carry.take() {
            Some((carry_start, carry_text)) => {
                let mut combined = carry_text;
                combined.push_str(text);
                (carry_start, combined)
            }
            None => (start, text.to_owned()),
        };
        let boundaries = combined
            .grapheme_indices(true)
            .map(|(offset, grapheme)| (offset, grapheme.len()))
            .collect::<Vec<_>>();
        let keep = boundaries.last().copied();
        for (offset, length) in boundaries
            .iter()
            .copied()
            .take(boundaries.len().saturating_sub(1))
        {
            let grapheme_start = combined_start.saturating_add(offset as u64);
            let grapheme_end = grapheme_start.saturating_add(length as u64);
            builder = builder.emit(
                StreamRange::new(
                    StreamOffset::new(grapheme_start),
                    StreamOffset::new(grapheme_end),
                ),
                TextContent::raw(combined[offset..offset + length].to_owned()),
            );
        }
        if let Some((offset, length)) = keep {
            let carry_start = combined_start.saturating_add(offset as u64);
            carry = Some((carry_start, combined[offset..offset + length].to_owned()));
        }
        // Drop the temporary combined buffer after preserving only the final
        // grapheme. This keeps cross-append EGC handling correct without
        // materializing the complete Source.
        combined.clear();
    }
    if let Some((start, grapheme)) = carry {
        let end = start.saturating_add(grapheme.len() as u64);
        builder = builder.emit(
            StreamRange::new(StreamOffset::new(start), StreamOffset::new(end)),
            TextContent::raw(grapheme),
        );
    }
    builder.finish().map_err(TextProjectionError::Projection)
}

fn project_semantic_snapshot(
    snapshot: &HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    execution: &mut ConnectorExecution,
) -> Result<Projection<TextContent>> {
    execution.prepare_for_snapshot(snapshot);
    let raw = source_projection(snapshot).map_err(|error| anyhow!(error.to_string()))?;
    let semantic = match funnel.kind {
        TextFunnelKind::Plain => PlainTextProjector::new()
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Markdown => execution
            .markdown
            .get_or_insert_with(|| {
                MarkdownProjector::new(MarkdownOptions::gfm().with_live_table_stabilization(true))
            })
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Diff => execution
            .diff
            .get_or_insert_with(DiffProjector::new)
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
        TextFunnelKind::Ansi => execution
            .ansi
            .get_or_insert_with(|| {
                AnsiProjector::new(crate::text::AnsiOptions {
                    hyperlinks: funnel.hyperlinks,
                })
            })
            .project(&raw)
            .map_err(|error| anyhow!(error.to_string()))?,
    };
    if snapshot.annotations_for_projection().is_empty() {
        return Ok(semantic);
    }
    SourceAnnotationRewriter::new(&snapshot.storage)
        .into_projector()
        .project(&semantic)
        .map_err(|error| anyhow!(error.to_string()))
}

fn content_text_policy(wrap: TextWrapMode) -> crate::text::TextRenderPolicy {
    let text_wrap = match wrap {
        TextWrapMode::Word => crate::WrapMode::WordThenGrapheme,
        TextWrapMode::Grapheme => crate::WrapMode::Grapheme,
        TextWrapMode::NoWrap => crate::WrapMode::NoWrap,
    };
    crate::TextRenderPolicy::new()
        .with_block_gap(1)
        .with_soft_break(crate::SoftBreakPolicy::LineBreak)
        .with_table_column_sizing(crate::TableColumnSizing::Content)
        .with_table_column_gap(1)
        .with_table_row_gap(0)
        .with_task_list_marker(crate::TaskListMarkerPolicy::TaskOnly)
        .with_code_block_label(crate::CodeBlockLabelPolicy::Language)
        .with_code_block_gap(0)
        .with_code_wrap(crate::WrapMode::NoWrap)
        .with_text_wrap(text_wrap)
}

fn semantic_values(semantic: &Projection<TextContent>) -> Arc<[TextContent]> {
    semantic
        .spans()
        .iter()
        .flat_map(|span| span.values().iter().cloned())
        .collect::<Vec<_>>()
        .into()
}

fn project_terminal_contents(
    contents: &[TextContent],
    policy: &crate::text::TextRenderPolicy,
    constraints: crate::text::TerminalConstraints,
) -> Result<Arc<crate::text::TerminalTextProduct>> {
    let projector = crate::text::TerminalTextProjector::new(policy.clone());
    projector
        .project_contents(contents, constraints)
        .map(Arc::new)
        .map_err(|error| anyhow!(error.to_string()))
}

fn paint_terminal_rows(
    product: &crate::text::TerminalTextProduct,
    theme: &Theme,
    width: u16,
) -> Result<Vec<PhysicalRow>> {
    let size = product.size();
    let mut surface = Surface::new(width, size.height());
    product
        .paint_window(
            theme,
            PhysicalStyle::default(),
            &mut surface,
            (0, 0),
            crate::geometry::Rect::new(0, 0, width, size.height()),
            TerminalRowWindow::new(0, usize::from(size.height())),
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok((0..size.height())
        .map(|row| PhysicalRow::from_cells(surface.row_cells(row).to_vec()))
        .collect())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RevealBoundary {
    pub(crate) revealed_height: u16,
    pub(crate) fully_revealed_rows: usize,
    pub(crate) cut: Option<(u16, u16)>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityIndex {
    pub(crate) row_glyphs: Vec<Vec<u16>>,
    pub(crate) total_glyphs: usize,
}

impl VisibilityIndex {
    pub(crate) fn from_rows(rows: &[PhysicalRow]) -> Self {
        let mut row_glyphs = Vec::with_capacity(rows.len());
        let mut total_glyphs = 0;
        for row in rows {
            let mut cols = Vec::new();
            for glyph in row.glyphs() {
                if glyph.leader.painted {
                    cols.push(glyph.start as u16);
                    total_glyphs += 1;
                }
            }
            row_glyphs.push(cols);
        }
        Self {
            row_glyphs,
            total_glyphs,
        }
    }

    pub(crate) fn reveal_bounds(&self, units: usize, width: u16, height: u16) -> RevealBoundary {
        if units == 0 || width == 0 || height == 0 || self.total_glyphs == 0 {
            return RevealBoundary {
                revealed_height: 0,
                fully_revealed_rows: 0,
                cut: None,
            };
        }
        if units >= self.total_glyphs {
            return RevealBoundary {
                revealed_height: height,
                fully_revealed_rows: self.row_glyphs.len(),
                cut: None,
            };
        }
        let mut remaining = units;
        let mut last_row = 0u16;
        let mut saw_glyph = false;
        let mut fully_revealed = 0usize;
        let mut cut: Option<(u16, u16)> = None;

        for (row_idx, cols) in self.row_glyphs.iter().enumerate() {
            let row = row_idx as u16;
            if cols.is_empty() {
                fully_revealed = row_idx + 1;
                continue;
            }
            if remaining < cols.len() {
                let cut_col = cols[remaining];
                cut = Some((row, cut_col));
                if remaining > 0 {
                    saw_glyph = true;
                    last_row = row;
                }
                break;
            }
            remaining -= cols.len();
            saw_glyph = true;
            last_row = row;
            fully_revealed = row_idx + 1;
        }

        if !saw_glyph {
            return RevealBoundary {
                revealed_height: 0,
                fully_revealed_rows: 0,
                cut: None,
            };
        }

        let target_height = if let Some((cut_row, cut_col)) = cut {
            if cut_col > 0 {
                last_row.max(cut_row).saturating_add(1)
            } else {
                last_row.saturating_add(1)
            }
        } else {
            last_row.saturating_add(1)
        };

        RevealBoundary {
            revealed_height: target_height.min(height),
            fully_revealed_rows: fully_revealed,
            cut,
        }
    }
}

#[derive(Clone, Debug)]
struct FinalizedPrefixProduct {
    rows: Arc<Vec<PhysicalRow>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PrefixProofKey {
    semantic: SemanticProjectionKey,
    source_end: u64,
    width: u16,
}

#[derive(Clone, Debug)]
struct PrefixProof {
    source_end: u64,
    product: Arc<crate::text::TerminalTextProduct>,
}

type PrefixProofCache = VecDeque<(PrefixProofKey, Arc<PrefixProof>)>;

fn prove_finalized_prefix(
    snapshot: &HostContentSourceSnapshot,
    semantic_key: &SemanticProjectionKey,
    funnel: HostContentFunnel,
    theme: &Theme,
    offered_width: u16,
    semantic_cache: &mut SemanticProjectionCache,
    prefix_proof_cache: &mut PrefixProofCache,
) -> Option<Arc<FinalizedPrefixProduct>> {
    // This is the pre-L11 finalized-prefix policy. It deliberately renders
    // an immutable sealed range rather than slicing the open projection; the
    // existing parser/restart behavior remains authoritative.
    let prefix = snapshot.stable_prefix()?;
    let stable_end = prefix.source_end;
    let key = PrefixProofKey {
        semantic: semantic_key.clone(),
        source_end: stable_end,
        width: offered_width,
    };
    let proof = if let Some(proof) = prefix_proof_cache
        .iter()
        .find(|(candidate, _)| candidate == &key)
        .map(|(_, proof)| Arc::clone(proof))
    {
        proof
    } else {
        let mut prefix_execution = ConnectorExecution::new(&funnel);
        let prefix_key = SemanticProjectionKey::for_snapshot(&prefix, funnel);
        let prefix_semantic = resolve_cached_semantic(semantic_cache, prefix_key, || {
            project_semantic_snapshot(&prefix, funnel, &mut prefix_execution)
        })
        .ok()?;
        let contents = semantic_values(&prefix_semantic);
        let policy = content_text_policy(funnel.wrap);
        let product = project_terminal_contents(
            &contents,
            &policy,
            crate::text::TerminalConstraints::definite(offered_width),
        )
        .ok()?;
        let proof = Arc::new(PrefixProof {
            source_end: stable_end,
            product,
        });
        if prefix_proof_cache.len() >= CONTENT_PREFIX_CACHE_CAPACITY {
            prefix_proof_cache.pop_back();
        }
        prefix_proof_cache.push_front((key, Arc::clone(&proof)));
        proof
    };
    debug_assert_eq!(proof.source_end, stable_end);
    let rows = paint_terminal_rows(&proof.product, theme, offered_width).ok()?;
    Some(Arc::new(FinalizedPrefixProduct {
        rows: Arc::new(rows),
    }))
}

fn project_text_snapshot(
    snapshot: &HostContentSourceSnapshot,
    funnel: HostContentFunnel,
    offered_width: u16,
    needs_finalized_prefix: bool,
    theme: &Arc<Theme>,
    theme_revision: u64,
    execution: &mut ConnectorExecution,
    delivery_revision: u64,
    semantic_cache: &mut SemanticProjectionCache,
    prefix_proof_cache: &mut PrefixProofCache,
) -> Result<HostContentProjection> {
    let key = TextProjectionKey {
        source_id: snapshot.source_id,
        source_generation: snapshot.source_generation,
        content_generation: snapshot.content_generation,
        source_revision: snapshot.revision,
        source_base: snapshot.source_base,
        source_end: snapshot.source_end,
        head_partial: snapshot.head_partial,
        width: offered_width,
        wrap: funnel.wrap,
        funnel_kind: funnel.kind,
        delivery_revision,
        theme_revision,
        needs_finalized_prefix,
        needs_physical_rows: needs_finalized_prefix || execution.delivery.is_some(),
    };
    let semantic_key = SemanticProjectionKey::for_snapshot(snapshot, funnel);
    let (row_bound, max_line_bytes) = projected_bounds(snapshot, funnel.wrap, offered_width);
    if row_bound > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow::Error::new(ContentProjectionFailure {
            kind: ContentProjectionFailureKind::LimitExceeded,
            diagnostic: format!(
                "LIMIT_EXCEEDED: content projection requires {row_bound} rows, exceeding the terminal row limit"
            ),
        }));
    }
    if max_line_bytes > MAX_CONTENT_PROJECTION_ROWS {
        return Err(anyhow::Error::new(ContentProjectionFailure {
            kind: ContentProjectionFailureKind::LimitExceeded,
            diagnostic: format!(
                "LIMIT_EXCEEDED: content projection has a {max_line_bytes}-byte logical line, exceeding the terminal line limit"
            ),
        }));
    }
    let semantic = resolve_cached_semantic(semantic_cache, semantic_key.clone(), || {
        project_semantic_snapshot(snapshot, funnel, execution)
    })?;
    let semantic_contents = semantic_values(&semantic);
    let terminal_policy = content_text_policy(funnel.wrap);
    let product = project_terminal_contents(
        &semantic_contents,
        &terminal_policy,
        crate::text::TerminalConstraints::definite(offered_width),
    )?;
    let min_product = project_terminal_contents(
        &semantic_contents,
        &terminal_policy,
        crate::text::TerminalConstraints::min_content(),
    )?;
    let max_product = project_terminal_contents(
        &semantic_contents,
        &terminal_policy,
        crate::text::TerminalConstraints::max_content(),
    )?;
    let terminal_size = product.size();
    let size = Size::new(terminal_size.width(), terminal_size.height());
    let terminal_intrinsic_size = product.intrinsic_size();
    // Keep the natural width from the producer for content-fit requests while the
    // measured height remains tied to this exact offered-width product.
    let intrinsic_size = Size::new(terminal_intrinsic_size.width(), size.height);
    let retain_rows = key.needs_physical_rows;
    let rows = retain_rows
        .then(|| paint_terminal_rows(&product, theme, offered_width))
        .transpose()?
        .map(Arc::new);
    let visibility = rows.as_ref().map(|rows| VisibilityIndex::from_rows(rows));
    let finalized_prefix = if !needs_finalized_prefix {
        None
    } else if snapshot.sealed {
        Some(Arc::new(FinalizedPrefixProduct {
            rows: Arc::clone(
                rows.as_ref()
                    .expect("History products retain physical rows"),
            ),
        }))
    } else {
        prove_finalized_prefix(
            snapshot,
            &semantic_key,
            funnel,
            theme,
            offered_width,
            semantic_cache,
            prefix_proof_cache,
        )
    };

    let (intrinsic_size, visible_row_count, cut, _fully_revealed_rows) =
        if let Some(delivery) = execution.delivery.as_mut() {
            delivery.accept_input(snapshot)?;
            let reveal_units = delivery.reveal_units();
            let bounds = visibility
                .as_ref()
                .expect("delivery products retain visibility rows")
                .reveal_bounds(reveal_units, size.width, size.height);
            (
                Size::new(intrinsic_size.width, bounds.revealed_height),
                usize::from(bounds.revealed_height),
                bounds.cut,
                bounds.fully_revealed_rows,
            )
        } else {
            (
                intrinsic_size,
                usize::from(size.height),
                None,
                usize::from(size.height),
            )
        };

    let stable_rows = finalized_prefix.as_ref().map_or(0, |prefix| {
        if execution.delivery.is_some() {
            // Preserve the established row-granular Smooth policy: History
            // may transfer the finalized product incrementally as the same
            // number of open rows become fully revealed.  In particular, a
            // sealed source can still have a partial sink-visible backlog.
            prefix.rows.len().min(_fully_revealed_rows)
        } else {
            prefix.rows.len()
        }
    });
    let physically_complete = product.physically_complete();

    Ok(HostContentProjection {
        identity: next_content_projection_id(),
        key,
        source_snapshot: snapshot.clone(),
        product,
        semantic_contents,
        terminal_policy,
        intrinsic_size,
        physically_complete,
        min_content: {
            let size = min_product.size();
            Size::new(size.width(), size.height())
        },
        max_content: {
            let size = max_product.size();
            Size::new(size.width(), size.height())
        },
        rows,
        theme: Arc::clone(theme),
        finalized_prefix,
        stable_rows,
        visible_row_count,
        cut,
    })
}

/// Applies Source annotations to semantic runs without resolving them to a
/// host-native style. Exact runs preserve source coordinates; transformed
/// runs use a deterministic proportional split when a range crosses them.
struct SourceAnnotationRewriter<'a> {
    storage: &'a StoredSource,
}

impl<'a> SourceAnnotationRewriter<'a> {
    fn new(storage: &'a StoredSource) -> Self {
        Self { storage }
    }

    fn annotate_run(&self, run: TextRun) -> Result<Vec<TextRun>, TextProjectionError> {
        let Some(source) = (match run.provenance() {
            TextProvenance::Exact(range) | TextProvenance::Derived(range) => Some(*range),
            TextProvenance::Synthetic => None,
        }) else {
            return Ok(vec![run]);
        };
        if source.is_empty() || run.text().is_empty() {
            return Ok(vec![run]);
        }
        let overlapping = self
            .storage
            .overlapping(source.start().as_u64(), source.end().as_u64());
        if overlapping.is_empty() {
            return Ok(vec![run]);
        }
        let mut cuts = vec![0usize, run.text().len()];
        for overlap in &overlapping {
            for offset in [overlap.start(), overlap.end()] {
                if offset <= source.start().as_u64() || offset >= source.end().as_u64() {
                    continue;
                }
                let local = source_local_offset(&run, source, offset);
                if run.text().is_char_boundary(local) {
                    cuts.push(local);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        if cuts.len() == 2 {
            let mut piece = run;
            piece = piece.map_annotations(|current| {
                overlapping.iter().fold(current, |current, overlap| {
                    if let Some(tag) = overlap.tag() {
                        current.with_tag(tag)
                    } else {
                        current
                    }
                })
            });
            if let Some(style) = overlapping.iter().rev().find_map(|overlap| overlap.style()) {
                piece = piece.with_style(style);
            }
            return Ok(vec![piece]);
        }
        let mut output = Vec::with_capacity(cuts.len().saturating_sub(1));
        for pair in cuts.windows(2) {
            let local_start = pair[0];
            let local_end = pair[1];
            if local_start == local_end {
                continue;
            }
            let (piece, _) = run.split_at(local_end)?;
            let (_, piece) = piece.split_at(local_start)?;
            let piece_source = source_range_for_piece(&run, source, local_start, local_end);
            let active = overlapping
                .iter()
                .filter(|overlap| {
                    overlap.end() > piece_source.start().as_u64()
                        && overlap.start() < piece_source.end().as_u64()
                })
                .collect::<Vec<_>>();
            let mut piece = piece;
            if !active.is_empty() {
                piece = piece.map_annotations(|current| {
                    active.iter().fold(current, |current, overlap| {
                        if let Some(tag) = overlap.tag() {
                            current.with_tag(tag)
                        } else {
                            current
                        }
                    })
                });
                if let Some(style) = active.iter().rev().find_map(|overlap| overlap.style()) {
                    piece = piece.with_style(style);
                }
            }
            output.push(piece);
        }
        Ok(output)
    }
}

impl TextRewriter for SourceAnnotationRewriter<'_> {
    type Error = TextProjectionError;

    fn rewrite_block(&mut self, block: Block) -> Result<Block, Self::Error> {
        walk_rewrite_block(self, block)
    }

    fn rewrite_inline(&mut self, inline: Inline) -> Result<Inline, Self::Error> {
        let InlineKind::Text(run) = inline.kind() else {
            return walk_rewrite_inline(self, inline);
        };
        let pieces = self.annotate_run(run.clone())?;
        if pieces.len() != 1 {
            // Inline content owns the vector boundary. This branch is only
            // used by rewrite_inline callers outside our inline-content hook;
            // preserve the first piece rather than duplicating an Inline.
            let Some(first) = pieces.into_iter().next() else {
                return Ok(inline);
            };
            return Ok(Inline::from_parts(
                InlineKind::Text(first),
                inline.marks().clone(),
                inline.annotations().clone(),
            ));
        }
        Ok(Inline::from_parts(
            InlineKind::Text(pieces.into_iter().next().expect("one piece")),
            inline.marks().clone(),
            inline.annotations().clone(),
        ))
    }

    fn rewrite_inline_content(
        &mut self,
        content: InlineContent,
    ) -> Result<InlineContent, Self::Error> {
        let mut output = Vec::new();
        for inline in content.items() {
            let InlineKind::Text(run) = inline.kind() else {
                output.push(self.rewrite_inline(inline.clone())?);
                continue;
            };
            for piece in self.annotate_run(run.clone())? {
                output.push(Inline::from_parts(
                    InlineKind::Text(piece),
                    inline.marks().clone(),
                    inline.annotations().clone(),
                ));
            }
        }
        Ok(InlineContent::new(output))
    }

    fn rewrite_literal(&mut self, literal: LiteralText) -> Result<LiteralText, Self::Error> {
        let mut runs = Vec::new();
        for run in literal.runs() {
            runs.extend(self.annotate_run(run.clone())?);
        }
        Ok(LiteralText::new(runs))
    }
}

fn source_local_offset(run: &TextRun, source: StreamRange, offset: u64) -> usize {
    let source_delta = offset.saturating_sub(source.start().as_u64());
    let local = match run.provenance() {
        TextProvenance::Exact(_) => source_delta,
        TextProvenance::Derived(_) => source_delta
            .saturating_mul(run.text().len() as u64)
            .checked_div(source.len().max(1))
            .unwrap_or_default(),
        TextProvenance::Synthetic => 0,
    };
    usize::try_from(local)
        .unwrap_or(run.text().len())
        .min(run.text().len())
}

fn source_range_for_piece(
    run: &TextRun,
    source: StreamRange,
    local_start: usize,
    local_end: usize,
) -> StreamRange {
    match run.provenance() {
        TextProvenance::Exact(_) => StreamRange::new(
            source.start().saturating_add(local_start as u64),
            source.start().saturating_add(local_end as u64),
        ),
        TextProvenance::Derived(_) => {
            let start = source.start().as_u64().saturating_add(
                (local_start as u64)
                    .saturating_mul(source.len())
                    .checked_div(run.text().len().max(1) as u64)
                    .unwrap_or_default(),
            );
            let end = source.start().as_u64().saturating_add(
                (local_end as u64)
                    .saturating_mul(source.len())
                    .checked_div(run.text().len().max(1) as u64)
                    .unwrap_or_default(),
            );
            StreamRange::new(
                StreamOffset::new(start.min(source.end().as_u64())),
                StreamOffset::new(end.min(source.end().as_u64())),
            )
        }
        TextProvenance::Synthetic => StreamRange::new(source.start(), source.start()),
    }
}

// ContentPort IDs cross the structural/native boundary, so they must not be
// host-local: a product built for host A must not accidentally resolve to host B's
// port with the same local slot. IDs are monotonic and never reused.
static NEXT_CONTENT_PORT_ID: AtomicU64 = AtomicU64::new(1);

/// Immutable, Source-neutral Funnel configuration supplied by the control
/// transport. It has no active state, host, viewport, or projection cache.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostContentFunnel {
    pub family: ContentFamily,
    pub kind: TextFunnelKind,
    pub wrap: TextWrapMode,
    pub hyperlinks: bool,
    pub delivery: ContentDelivery,
}

impl HostContentFunnel {
    #[must_use]
    pub const fn plain(wrap: TextWrapMode) -> Self {
        Self {
            family: ContentFamily::Text,
            kind: TextFunnelKind::Plain,
            wrap,
            hyperlinks: true,
            delivery: ContentDelivery::Immediate,
        }
    }

    #[must_use]
    pub const fn new(
        kind: TextFunnelKind,
        wrap: TextWrapMode,
        hyperlinks: bool,
        delivery: ContentDelivery,
    ) -> Self {
        Self {
            family: ContentFamily::Text,
            kind,
            wrap,
            hyperlinks,
            delivery,
        }
    }

    fn smooth_config(&self) -> Option<SmoothConfig> {
        match self.delivery {
            ContentDelivery::Immediate => None,
            ContentDelivery::Smooth(config) => Some(config),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRetentionPolicy {
    max_bytes: Option<u64>,
    max_lines: Option<u64>,
    drop_oldest: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortLifecycle {
    Live,
    Disposed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectorLifecycle {
    Live,
    Disposing,
    Disposed,
}

#[derive(Clone, Debug)]
struct SourceSubscriptionGroup {
    host: Weak<Mutex<HostInner>>,
    tokens: Vec<(u64, u32)>,
    wake_tokens: Vec<(u64, u32)>,
}

struct CapturedSubscriberGroup {
    host_key: usize,
    host: Weak<Mutex<HostInner>>,
    tokens: Vec<(u64, u32)>,
}

impl std::fmt::Debug for CapturedSubscriberGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapturedSubscriberGroup")
            .field("host_key", &self.host_key)
            .field("tokens", &self.tokens)
            .finish()
    }
}

#[derive(Debug)]
struct ContentSourceRecord {
    id: u64,
    generation: u32,
    content_generation: u64,
    revision: u64,
    family: ContentFamily,
    kind: TextSourceKind,
    lifecycle: SourceLifecycle,
    retention: Option<SourceRetentionPolicy>,
    storage: Arc<StoredSource>,
    copied_bytes: u64,
    dropped_head_bytes: u64,
    accepted_bytes: u64,
    connector_count: usize,
    /// Host-grouped wake subscriptions.  The host allocation pointer is only
    /// an in-process map key; each value retains a Weak host and generation-
    /// checked Connector tokens for validation when a mutation is drained.
    subscribers: HashMap<usize, SourceSubscriptionGroup>,
    /// Reused outer wake batch storage. The Source lock is released before
    /// hosts are touched, so the batch is recycled only after every eligible
    /// host has been attempted.
    subscriber_wake_scratch: Vec<CapturedSubscriberGroup>,
}

/// A Source mutation can finish after releasing the Source lock, so a host
/// wake failure cannot be returned as the mutation's ordinary `Result`:
/// doing so would make an already-installed revision look rejected to the
/// caller.  Keep one latest failure per host in an environment-owned side
/// channel instead.  The weak host reference is retained to resolve the
/// current environment host ID without taking the possibly poisoned host
/// lock, and also prevents an allocator-address reuse from misattributing a
/// stale failure to a new host.
#[derive(Debug)]
pub(crate) struct PendingSourceWakeFailure {
    pub(super) host_key: usize,
    pub(super) host: Weak<Mutex<HostInner>>,
    pub(super) revision: u64,
    pub(super) diagnostic: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SourceWakeFailureChannel {
    failures: Arc<Mutex<HashMap<usize, PendingSourceWakeFailure>>>,
}

impl SourceWakeFailureChannel {
    pub(super) fn record(
        &self,
        host: &Weak<Mutex<HostInner>>,
        revision: u64,
        diagnostic: impl Into<String>,
    ) {
        let host_key = host.as_ptr() as usize;
        let mut failures = self
            .failures
            .lock()
            // A poisoned diagnostic channel must not turn accepted Source
            // bytes into an apparent mutation rejection.  The queue contains
            // only replaceable diagnostics, so recovering its guard is safe;
            // the failure remains explicit when the environment drains it.
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failures
            .get(&host_key)
            .is_some_and(|previous| previous.revision > revision)
        {
            return;
        }
        failures.insert(
            host_key,
            PendingSourceWakeFailure {
                host_key,
                host: host.clone(),
                revision,
                diagnostic: diagnostic.into(),
            },
        );
    }

    pub(super) fn take(&self) -> Vec<PendingSourceWakeFailure> {
        let mut failures = self
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut pending = failures
            .drain()
            .map(|(_, failure)| failure)
            .collect::<Vec<_>>();
        // HashMap iteration order is deliberately unspecified. Stable error
        // ordering keeps the environment report deterministic while retaining
        // O(number of failed hosts) extraction.
        pending.sort_unstable_by_key(|failure| (failure.host_key, failure.revision));
        pending
    }
}

#[derive(Debug, Default)]
struct ContentSourceRegistryInner {
    next_id: u64,
    next_generation: u32,
    sources: HashMap<u64, Arc<Mutex<ContentSourceRecord>>>,
}

/// Environment-owned Source registry. The registry holds the authoritative
/// record strongly so a Source can outlive any host and can be reused by later
/// hosts in the same environment.
#[derive(Clone, Debug)]
pub(crate) struct ContentSourceRegistry {
    inner: Arc<Mutex<ContentSourceRegistryInner>>,
    identity: EnvironmentIdentity,
    wake_failures: SourceWakeFailureChannel,
    executor: Arc<ContentExecutor>,
    executor_startup_error: Option<String>,
}

impl Default for ContentSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentSourceRegistry {
    pub(crate) fn new() -> Self {
        Self::with_identity(EnvironmentIdentity::allocate())
    }

    pub(crate) fn with_identity(identity: EnvironmentIdentity) -> Self {
        let (executor, executor_startup_error) = match ContentExecutor::new() {
            Ok(executor) => (executor, None),
            Err(error) => {
                let diagnostic = error.to_string();
                (
                    ContentExecutor::failed(diagnostic.clone()),
                    Some(diagnostic),
                )
            }
        };
        Self {
            inner: Arc::new(Mutex::new(ContentSourceRegistryInner::default())),
            identity,
            wake_failures: SourceWakeFailureChannel::default(),
            executor,
            executor_startup_error,
        }
    }

    pub(super) fn executor_startup_error(&self) -> Option<String> {
        self.executor_startup_error.clone()
    }

    pub(super) fn record_wake_failure(
        &self,
        host: &Weak<Mutex<HostInner>>,
        revision: u64,
        diagnostic: impl Into<String>,
    ) {
        self.wake_failures.record(host, revision, diagnostic);
    }

    pub(super) fn take_wake_failures(&self) -> Vec<PendingSourceWakeFailure> {
        self.wake_failures.take()
    }

    pub(crate) fn create(&self, kind: TextSourceKind) -> Result<HostContentSource> {
        if let Some(error) = self.executor_startup_error.as_ref() {
            return Err(anyhow!(error.clone()));
        }
        let mut registry = self
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        let id = registry
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source identity exhausted"))?;
        if id > u64::from(u32::MAX) {
            return Err(anyhow!("content Source identity exhausted"));
        }
        let generation = registry
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("content Source generation exhausted"))?;
        registry.next_id = id;
        registry.next_generation = generation;
        let record = Arc::new(Mutex::new(ContentSourceRecord {
            id,
            generation,
            content_generation: 1,
            revision: 0,
            family: ContentFamily::Text,
            kind,
            lifecycle: SourceLifecycle::Live,
            retention: None,
            storage: Arc::new(StoredSource::empty()),
            copied_bytes: 0,
            dropped_head_bytes: 0,
            accepted_bytes: 0,
            connector_count: 0,
            subscribers: HashMap::new(),
            subscriber_wake_scratch: Vec::new(),
        }));
        registry.sources.insert(id, Arc::clone(&record));
        Ok(HostContentSource {
            registry: self.clone(),
            record,
        })
    }

    fn contains(&self, id: u64, record: &Arc<Mutex<ContentSourceRecord>>) -> bool {
        self.inner
            .lock()
            .ok()
            .and_then(|registry| registry.sources.get(&id).cloned())
            .is_some_and(|candidate| Arc::ptr_eq(&candidate, record))
    }

    pub(crate) fn lookup(&self, id: u64, generation: u32) -> Result<HostContentSource> {
        let record = self
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?
            .sources
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_SOURCE: Source {id} is unavailable"))?;
        let matches = record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?
            .generation
            == generation;
        if !matches {
            return Err(anyhow!("STALE_SOURCE: Source {id} generation is stale"));
        }
        Ok(HostContentSource {
            registry: self.clone(),
            record,
        })
    }
}

/// A native environment-owned Source identity and retained UTF-8 store.
#[derive(Clone, Debug)]
pub struct HostContentSource {
    registry: ContentSourceRegistry,
    record: Arc<Mutex<ContentSourceRecord>>,
}

pub struct PreparedSourceMutation {
    source: HostContentSource,
    record: Arc<Mutex<ContentSourceRecord>>,
    source_id: u64,
    expected_generation: u32,
    expected_revision: u64,
    next_revision: u64,
    next_content_generation: Option<u64>,
    next_storage: Option<Arc<StoredSource>>,
    copied_bytes: u64,
    dropped_head_bytes: u64,
    accepted_bytes: u64,
    expected_connector_count: usize,
    next_connector_count: usize,
    dispose_after_install: bool,
}

/// Owned replacement input validated by the same content-store decoder used by
/// direct Source mutation. Keeping the parsed annotations with the bytes lets
/// a UI batch validate every literal operation before coalescing final writes.
#[derive(Clone)]
pub(crate) struct PreparedSourceReplacement {
    text: String,
    parsed: Vec<ValidatedAnnotation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceInstallDisposition {
    Keep,
    DisposeWhenEmpty,
}

#[derive(Debug)]
pub struct PreparedSourceWakes {
    wakes: Vec<(HostContentSource, u64, Vec<CapturedSubscriberGroup>)>,
}

impl Drop for PreparedSourceWakes {
    fn drop(&mut self) {
        // If the owning UI transaction faults after Source installation but
        // before it can schedule the wake, return captured tokens to the
        // Source's reusable scratch storage. The accepted Source state stays
        // authoritative; a later mutation can capture the restored tokens.
        for (source, _revision, mut groups) in self.wakes.drain(..) {
            match source.record.lock() {
                Ok(mut record) => {
                    for group in &mut groups {
                        if let Some(subscriber) = record.subscribers.get_mut(&group.host_key) {
                            subscriber.wake_tokens = std::mem::take(&mut group.tokens);
                        }
                    }
                    record.subscriber_wake_scratch = groups;
                }
                Err(_) => eprintln!(
                    "prepared Source wake cleanup failed after accepted storage installation"
                ),
            }
        }
    }
}

fn ensure_source_live(record: &ContentSourceRecord) -> Result<()> {
    if record.lifecycle != SourceLifecycle::Live {
        return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
    }
    Ok(())
}

fn next_revision(revision: u64) -> Result<u64> {
    revision
        .checked_add(1)
        .ok_or_else(|| anyhow!("Source revision exhausted"))
}

fn validate_payload_size(length: usize) -> Result<()> {
    if length > MAX_SOURCE_PAYLOAD_BYTES {
        return Err(anyhow!(
            "PAYLOAD_TOO_LARGE: Source payload exceeds the configured limit"
        ));
    }
    Ok(())
}

fn decode_annotations(
    input: &ValidatedInput<'_>,
    absolute_base: u64,
    records: &[ContentAnnotationRecord],
    payload: &[u8],
) -> Result<Vec<ValidatedAnnotation>> {
    if records.len() > MAX_SOURCE_ANNOTATIONS {
        return Err(anyhow!(
            "LIMIT_EXCEEDED: annotation count exceeds the configured limit"
        ));
    }
    if payload.len() > MAX_ANNOTATION_PAYLOAD_BYTES {
        return Err(anyhow!(
            "PAYLOAD_TOO_LARGE: annotation payload exceeds the configured limit"
        ));
    }
    let text = input.text();
    let bytes = input.bytes();
    records
        .iter()
        .map(|record| {
            let policy = match record.kind {
                CONTENT_ANNOTATION_KIND_TAG
                | CONTENT_ANNOTATION_KIND_STYLE
                | CONTENT_ANNOTATION_KIND_ATOMIC
                | CONTENT_ANNOTATION_KIND_POINT => annotation_policy(record.kind),
                _ => {
                    return Err(anyhow!(
                        "UNKNOWN_ANNOTATION_KIND: annotation kind {} is unsupported",
                        record.kind
                    ));
                }
            };
            if record.flags != 0 || record.aux0 != 0 || record.aux1 != 0 {
                return Err(anyhow!(
                    "INVALID_ANNOTATION_PAYLOAD: annotation flags or auxiliary lanes are reserved"
                ));
            }
            let start = usize::try_from(record.start_byte)
                .map_err(|_| anyhow!("INVALID_RANGE: annotation start does not fit usize"))?;
            let end = usize::try_from(record.end_byte)
                .map_err(|_| anyhow!("INVALID_RANGE: annotation end does not fit usize"))?;
            if start > end
                || end > bytes.len()
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
            {
                return Err(anyhow!(
                    "INVALID_RANGE: annotation range is not an ordered UTF-8 range"
                ));
            }
            if policy == AnnotationTruncationPolicy::Point && start != end {
                return Err(anyhow!(
                    "INVALID_RANGE: point annotations must have an empty range"
                ));
            }
            if policy != AnnotationTruncationPolicy::Point && start == end {
                return Err(anyhow!(
                    "INVALID_RANGE: non-point annotations must cover text"
                ));
            }
            let payload_end = record
                .payload_offset
                .checked_add(record.payload_length)
                .ok_or_else(|| anyhow!("INVALID_ANNOTATION_PAYLOAD: payload range overflow"))?;
            if payload_end as usize > payload.len() {
                return Err(anyhow!(
                    "INVALID_ANNOTATION_PAYLOAD: annotation payload range is outside the sidecar"
                ));
            }
            let annotation_payload = &payload[record.payload_offset as usize..payload_end as usize];
            let (tag, style) = match record.kind {
                CONTENT_ANNOTATION_KIND_TAG => (Some(decode_tag(annotation_payload)?), None),
                CONTENT_ANNOTATION_KIND_STYLE => {
                    (None, Some(decode_semantic_style(annotation_payload)?))
                }
                CONTENT_ANNOTATION_KIND_ATOMIC | CONTENT_ANNOTATION_KIND_POINT => (None, None),
                _ => unreachable!("annotation kind was validated above"),
            };
            let absolute_start = absolute_base
                .checked_add(record.start_byte as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: annotation coordinate exhausted"))?;
            let absolute_end = absolute_base
                .checked_add(record.end_byte as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: annotation coordinate exhausted"))?;
            Ok(ValidatedAnnotation {
                kind: record.kind,
                flags: record.flags,
                start_byte: absolute_start,
                end_byte: absolute_end,
                payload: payload[record.payload_offset as usize..payload_end as usize].to_vec(),
                aux0: record.aux0,
                aux1: record.aux1,
                tag,
                style,
            })
        })
        .collect()
}

fn decode_tag(payload: &[u8]) -> Result<crate::text::SemanticTag> {
    let separator = payload.iter().position(|byte| *byte == 0).ok_or_else(|| {
        anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: tag annotations require a NUL-separated namespace and name"
        )
    })?;
    if payload[separator + 1..].contains(&0) {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: tag annotation names must not contain NUL"
        ));
    }
    let namespace = str::from_utf8(&payload[..separator])
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: tag namespace is not valid UTF-8"))?;
    let name = str::from_utf8(&payload[separator + 1..])
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: tag name is not valid UTF-8"))?;
    crate::text::SemanticTag::new(namespace, name)
        .map_err(|error| anyhow!("INVALID_ANNOTATION_PAYLOAD: {error}"))
}

const STYLE_PAYLOAD_VERSION: u8 = 1;
const STYLE_FLAG_ROLE: u8 = 1 << 0;
const STYLE_FLAG_FOREGROUND: u8 = 1 << 1;
const STYLE_FLAG_BACKGROUND: u8 = 1 << 2;
const STYLE_FLAG_ATTRIBUTES: u8 = 1 << 3;

fn decode_semantic_style(payload: &[u8]) -> Result<StyleRef> {
    if payload.len() < 4 || payload[0] != STYLE_PAYLOAD_VERSION {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload version is unsupported"
        ));
    }
    let flags = payload[1];
    if flags & !0x0f != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload has reserved flags"
        ));
    }
    let presence = payload[2];
    let values = payload[3];
    if presence & !0x3f != 0 || values & !presence != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style attributes are malformed"
        ));
    }
    let mut cursor = 4usize;
    let role = if flags & STYLE_FLAG_ROLE != 0 {
        Some(read_style_string(payload, &mut cursor, "role")?)
    } else {
        None
    };
    let foreground = if flags & STYLE_FLAG_FOREGROUND != 0 {
        Some(read_style_color(payload, &mut cursor, "foreground")?)
    } else {
        None
    };
    let background = if flags & STYLE_FLAG_BACKGROUND != 0 {
        Some(read_style_color(payload, &mut cursor, "background")?)
    } else {
        None
    };
    if cursor != payload.len() {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style payload has trailing bytes"
        ));
    }
    let mut style = StyleSpec::new();
    if let Some(color) = foreground {
        style.set_foreground(color);
    }
    if let Some(color) = background {
        style.set_background(color);
    }
    if flags & STYLE_FLAG_ATTRIBUTES != 0 {
        for (bit, attribute) in [
            (1, TextAttribute::Bold),
            (2, TextAttribute::Dim),
            (4, TextAttribute::Italic),
            (8, TextAttribute::Underline),
            (16, TextAttribute::Reversed),
            (32, TextAttribute::Strikethrough),
        ] {
            if presence & bit != 0 {
                style.set_attribute(attribute, values & bit != 0);
            }
        }
    } else if presence != 0 || values != 0 {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style attributes flag is missing"
        ));
    }
    Ok(match role {
        Some(role) => StyleRef::themed(role, style),
        None => StyleRef::themed(crate::content::text::TEXT_THEME_KEY, style),
    })
}

fn read_style_string(payload: &[u8], cursor: &mut usize, field: &str) -> Result<String> {
    let length = read_style_u16(payload, cursor, field)? as usize;
    let end = cursor.checked_add(length).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} length overflow")
    })?;
    let value = payload.get(*cursor..end).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} is truncated")
    })?;
    *cursor = end;
    let value = str::from_utf8(value)
        .map_err(|_| anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} is not UTF-8"))?;
    if value.is_empty() || value.contains('\0') || value.chars().any(char::is_whitespace) {
        return Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style {field} is not a valid name"
        ));
    }
    Ok(value.to_owned())
}

fn read_style_u16(payload: &[u8], cursor: &mut usize, field: &str) -> Result<u16> {
    let end = (*cursor).saturating_add(2);
    let bytes = payload.get(*cursor..end).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} length is truncated")
    })?;
    *cursor = end;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_style_color(payload: &[u8], cursor: &mut usize, field: &str) -> Result<ColorSpec> {
    let kind = *payload.get(*cursor).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} color is truncated")
    })?;
    *cursor += 1;
    match kind {
        1 => Ok(ColorSpec::named(read_ansi_color(payload, cursor, field)?)),
        2 => Ok(ColorSpec::ansi(read_style_byte(payload, cursor, field)?)),
        3 => Ok(ColorSpec::rgb(
            read_style_byte(payload, cursor, field)?,
            read_style_byte(payload, cursor, field)?,
            read_style_byte(payload, cursor, field)?,
        )),
        4 => Ok(ColorSpec::theme(read_style_string(payload, cursor, field)?)),
        _ => Err(anyhow!(
            "INVALID_ANNOTATION_PAYLOAD: semantic style {field} color kind is unknown"
        )),
    }
}

fn read_style_byte(payload: &[u8], cursor: &mut usize, field: &str) -> Result<u8> {
    let value = *payload.get(*cursor).ok_or_else(|| {
        anyhow!("INVALID_ANNOTATION_PAYLOAD: semantic style {field} color is truncated")
    })?;
    *cursor += 1;
    Ok(value)
}

fn read_ansi_color(payload: &[u8], cursor: &mut usize, field: &str) -> Result<AnsiColor> {
    let value = read_style_byte(payload, cursor, field)?;
    let color = match value {
        0 => AnsiColor::Black,
        1 => AnsiColor::Red,
        2 => AnsiColor::Green,
        3 => AnsiColor::Yellow,
        4 => AnsiColor::Blue,
        5 => AnsiColor::Magenta,
        6 => AnsiColor::Cyan,
        7 => AnsiColor::Gray,
        8 => AnsiColor::DarkGray,
        9 => AnsiColor::LightRed,
        10 => AnsiColor::LightGreen,
        11 => AnsiColor::LightYellow,
        12 => AnsiColor::LightBlue,
        13 => AnsiColor::LightMagenta,
        14 => AnsiColor::LightCyan,
        15 => AnsiColor::White,
        _ => {
            return Err(anyhow!(
                "INVALID_ANNOTATION_PAYLOAD: semantic style {field} ANSI color is unknown"
            ));
        }
    };
    Ok(color)
}

fn retention_head(storage: &StoredSource, retention: Option<SourceRetentionPolicy>) -> u64 {
    let Some(retention) = retention else {
        return storage.base();
    };
    let mut head = storage.base();
    if let Some(max_bytes) = retention.max_bytes
        && storage.end().saturating_sub(storage.base()) > max_bytes
    {
        head = head.max(storage.offset_for_max_bytes(max_bytes));
    }
    if let Some(max_lines) = retention.max_lines
        && storage.line_count() as u64 > max_lines
    {
        let keep = usize::try_from(max_lines).unwrap_or(usize::MAX);
        let index = storage.line_count().saturating_sub(keep);
        if let Some(line_start) = storage.line_entry(storage.base(), index as u64) {
            head = head.max(line_start);
        }
    }
    head
}

fn retention_would_overflow(
    storage: &StoredSource,
    retention: Option<SourceRetentionPolicy>,
    appended_bytes: usize,
    appended_newlines: usize,
) -> bool {
    let Some(policy) = retention else {
        return false;
    };
    (!policy.drop_oldest
        && policy.max_bytes.is_some_and(|limit| {
            storage
                .end()
                .saturating_sub(storage.base())
                .saturating_add(appended_bytes as u64)
                > limit
        }))
        || (!policy.drop_oldest
            && policy.max_lines.is_some_and(|limit| {
                (storage.line_count() as u64).saturating_add(appended_newlines as u64) > limit
            }))
}

fn retention_requires_truncation(
    storage: &StoredSource,
    retention: Option<SourceRetentionPolicy>,
    appended_bytes: usize,
    appended_newlines: usize,
) -> bool {
    let Some(policy) = retention else {
        return false;
    };
    policy.drop_oldest
        && (policy.max_bytes.is_some_and(|limit| {
            storage
                .retained_bytes()
                .saturating_add(appended_bytes as u64)
                > limit
        }) || policy.max_lines.is_some_and(|limit| {
            storage.line_count().saturating_add(appended_newlines) as u64 > limit
        }))
}

fn apply_retention(
    storage: StoredSource,
    retention: Option<SourceRetentionPolicy>,
) -> Result<(StoredSource, u64)> {
    let head = retention_head(&storage, retention);
    if head == storage.base() {
        return Ok((storage, 0));
    }
    let policy = retention.expect("retention head requires a policy");
    if !policy.drop_oldest {
        return Err(anyhow!(
            "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
        ));
    }
    let (next, dropped) = storage
        .apply_truncate(storage.base(), head, storage.revision())
        .map_err(|err| anyhow!("{err}"))?;
    Ok((next, dropped))
}

fn capture_subscribers(record: &mut ContentSourceRecord) -> Vec<CapturedSubscriberGroup> {
    let mut captured = std::mem::take(&mut record.subscriber_wake_scratch);
    captured.clear();
    record
        .subscribers
        .retain(|_, group| group.host.strong_count() != 0 && !group.tokens.is_empty());
    for (&host_key, group) in record.subscribers.iter_mut() {
        let mut tokens = std::mem::take(&mut group.wake_tokens);
        tokens.clear();
        tokens.extend(group.tokens.iter().copied());
        captured.push(CapturedSubscriberGroup {
            host_key,
            host: group.host.clone(),
            tokens,
        });
    }
    captured
}

impl HostContentSource {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.record.lock().map_or(0, |record| record.id)
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.record.lock().map_or(0, |record| record.generation)
    }

    #[must_use]
    pub fn environment_slot(&self) -> u32 {
        self.registry.identity.slot
    }

    #[must_use]
    pub fn environment_generation(&self) -> u32 {
        self.registry.identity.generation
    }

    #[must_use]
    pub fn family(&self) -> ContentFamily {
        self.record
            .lock()
            .map_or(ContentFamily::Text, |record| record.family)
    }

    #[must_use]
    pub fn kind(&self) -> TextSourceKind {
        self.record
            .lock()
            .map_or(TextSourceKind::Stream, |record| record.kind)
    }

    pub fn retention_compatible(&self, funnel: TextFunnelKind) -> Result<bool> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if funnel != TextFunnelKind::Markdown {
            return Ok(true);
        }
        let truncated =
            record.retention.is_some_and(|policy| policy.drop_oldest) || record.storage.base() != 0;
        Ok(!truncated)
    }

    pub fn content_generation(&self) -> Result<u64> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        Ok(record.content_generation)
    }

    pub fn snapshot(&self) -> Result<HostContentSourceSnapshot> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        crate::perf::inc(crate::perf::Counter::SourceSnapshotsAcquired);
        let storage = Arc::clone(&record.storage);
        Ok(HostContentSourceSnapshot {
            source_id: record.id,
            source_generation: record.generation,
            content_generation: record.content_generation,
            revision: record.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            storage,
        })
    }

    pub fn stats(&self) -> Result<HostContentSourceStats> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        ensure_source_live(&record)?;
        let storage = &record.storage;
        Ok(HostContentSourceStats {
            revision: record.revision,
            source_base: storage.base(),
            source_end: storage.end(),
            retained_bytes: storage.retained_bytes(),
            retained_lines: storage.line_count() as u64,
            chunk_count: storage.chunk_count(),
            sealed: storage.sealed(),
            head_partial: storage.head_partial(),
            accepted_bytes: record.accepted_bytes,
            copied_bytes: record.copied_bytes,
            dropped_head_bytes: record.dropped_head_bytes,
        })
    }

    /// Appends one validated UTF-8 payload to a Stream Source. The payload is
    /// copied into immutable chunks before the Source lock is released.
    pub fn append_utf8(
        &self,
        bytes: &[u8],
        annotations: &[ContentAnnotationRecord],
        annotation_payload: &[u8],
    ) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind != TextSourceKind::Stream {
                return Err(anyhow!("INVALID_ARGUMENT: append requires a stream Source"));
            }
            if record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            validate_payload_size(bytes.len())?;
            let input = ValidatedInput::from_bytes(bytes)?;
            let base = record.storage.end();
            base.checked_add(input.len() as u64)
                .ok_or_else(|| anyhow!("INVALID_RANGE: Source coordinate exhausted"))?;
            let parsed = decode_annotations(&input, base, annotations, annotation_payload)?;
            if input.is_empty() && parsed.is_empty() {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            if retention_would_overflow(
                &record.storage,
                record.retention,
                input.len(),
                input.newlines(),
            ) {
                return Err(anyhow!(
                    "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
                ));
            }
            let retention = record.retention;
            // Preflight the fallible revision arithmetic before installing
            // candidate storage: a rejection must leave bytes, annotations,
            // revision and accounting exactly as they were (§9.6).
            let revision = next_revision(record.revision)?;
            let can_append_in_place = parsed.is_empty()
                && !retention_requires_truncation(
                    &record.storage,
                    retention,
                    input.len(),
                    input.newlines(),
                );
            let dropped = if can_append_in_place {
                if let Some(storage) = Arc::get_mut(&mut record.storage) {
                    storage.append_in_place(input.text(), revision);
                    0
                } else {
                    let next = record
                        .storage
                        .apply_append(input.text(), revision, parsed)
                        .map_err(|err| anyhow!("{err}"))?;
                    let (next, dropped) = apply_retention(next, retention)?;
                    record.storage = Arc::new(next);
                    dropped
                }
            } else {
                let next = record
                    .storage
                    .apply_append(input.text(), revision, parsed)
                    .map_err(|err| anyhow!("{err}"))?;
                let (next, dropped) = apply_retention(next, retention)?;
                record.storage = Arc::new(next);
                dropped
            };
            record.revision = revision;
            record.copied_bytes = record.copied_bytes.saturating_add(input.len() as u64);
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            record.accepted_bytes = record.accepted_bytes.saturating_add(input.len() as u64);
            (revision, capture_subscribers(&mut record))
        };
        Ok(self.finish_mutation(revision, subscribers))
    }

    pub fn prepare_membership_delta(
        &self,
        delta: i64,
        disposition: SourceInstallDisposition,
    ) -> Result<PreparedSourceMutation> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        ensure_source_live(&record)?;
        let next_connector_count = if delta >= 0 {
            record
                .connector_count
                .checked_add(delta as usize)
                .ok_or_else(|| anyhow!("Source Connector membership count exhausted"))?
        } else {
            record
                .connector_count
                .checked_sub(delta.unsigned_abs() as usize)
                .ok_or_else(|| {
                    anyhow!("SOURCE_MEMBERSHIP_MISMATCH: Connector membership underflow")
                })?
        };
        Ok(PreparedSourceMutation {
            source: self.clone(),
            record: Arc::clone(&self.record),
            source_id: record.id,
            expected_generation: record.generation,
            expected_revision: record.revision,
            next_revision: record.revision,
            next_content_generation: None,
            next_storage: None,
            copied_bytes: 0,
            dropped_head_bytes: 0,
            accepted_bytes: 0,
            expected_connector_count: record.connector_count,
            next_connector_count,
            dispose_after_install: disposition == SourceInstallDisposition::DisposeWhenEmpty,
        })
    }

    pub(crate) fn validate_replacement(
        bytes: Vec<u8>,
        annotations: &[ContentAnnotationRecord],
        annotation_payload: &[u8],
    ) -> Result<PreparedSourceReplacement> {
        validate_payload_size(bytes.len())?;
        let input = ValidatedInput::from_bytes(&bytes)?;
        let parsed = decode_annotations(&input, 0, annotations, annotation_payload)?;
        Ok(PreparedSourceReplacement {
            text: input.text().to_owned(),
            parsed,
        })
    }

    pub(crate) fn prepare_validated_replacement_with_membership(
        &self,
        replacement: PreparedSourceReplacement,
        membership_delta: i64,
        disposition: SourceInstallDisposition,
    ) -> Result<PreparedSourceMutation> {
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        ensure_source_live(&record)?;
        if record.kind == TextSourceKind::Stream && record.storage.sealed() {
            return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
        }
        let next_revision = next_revision(record.revision)?;
        let next_content_generation = record
            .content_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("Source content generation exhausted"))?;
        let next = StoredSource::empty()
            .apply_append(&replacement.text, next_revision, replacement.parsed)
            .map_err(|err| anyhow!("{err}"))?;
        let (next, dropped_head_bytes) = apply_retention(next, record.retention)?;
        let source_id = record.id;
        let expected_generation = record.generation;
        let expected_revision = record.revision;
        let expected_connector_count = record.connector_count;
        let next_connector_count = if membership_delta >= 0 {
            record
                .connector_count
                .checked_add(membership_delta as usize)
                .ok_or_else(|| anyhow!("Source Connector membership count exhausted"))?
        } else {
            record
                .connector_count
                .checked_sub(membership_delta.unsigned_abs() as usize)
                .ok_or_else(|| {
                    anyhow!("SOURCE_MEMBERSHIP_MISMATCH: Connector membership underflow")
                })?
        };
        drop(record);
        Ok(PreparedSourceMutation {
            source: self.clone(),
            record: Arc::clone(&self.record),
            source_id,
            expected_generation,
            expected_revision,
            next_revision,
            next_content_generation: Some(next_content_generation),
            next_storage: Some(Arc::new(next)),
            copied_bytes: replacement.text.len() as u64,
            dropped_head_bytes,
            accepted_bytes: replacement.text.len() as u64,
            expected_connector_count,
            next_connector_count,
            dispose_after_install: disposition == SourceInstallDisposition::DisposeWhenEmpty,
        })
    }

    pub fn install_prepared_mutations(
        mut mutations: Vec<PreparedSourceMutation>,
    ) -> Result<PreparedSourceWakes> {
        mutations.sort_unstable_by_key(|mutation| {
            (
                mutation.source.environment_slot(),
                mutation.source.environment_generation(),
                mutation.source_id,
            )
        });
        if let Some(first) = mutations.first() {
            let environment = (
                first.source.environment_slot(),
                first.source.environment_generation(),
            );
            if mutations.iter().any(|mutation| {
                (
                    mutation.source.environment_slot(),
                    mutation.source.environment_generation(),
                ) != environment
            }) {
                return Err(anyhow!(
                    "SOURCE_TRANSACTION_ENVIRONMENT: Sources belong to different environments"
                ));
            }
        }
        if mutations.windows(2).any(|pair| {
            pair[0].source_id == pair[1].source_id
                && pair[0].source.environment_slot() == pair[1].source.environment_slot()
                && pair[0].source.environment_generation()
                    == pair[1].source.environment_generation()
        }) {
            return Err(anyhow!(
                "SOURCE_TRANSACTION_DUPLICATE: duplicate Source mutation"
            ));
        }
        let mut guards = Vec::new();
        guards
            .try_reserve(mutations.len())
            .map_err(|_| anyhow!("Source transaction guard capacity exhausted"))?;
        for mutation in &mutations {
            guards.push(
                mutation
                    .record
                    .lock()
                    .map_err(|_| anyhow!("content Source lock is poisoned"))?,
            );
        }
        for (mutation, record) in mutations.iter().zip(guards.iter()) {
            ensure_source_live(record)?;
            if record.id != mutation.source_id
                || record.generation != mutation.expected_generation
                || record.revision != mutation.expected_revision
                || record.connector_count != mutation.expected_connector_count
            {
                return Err(anyhow!(
                    "SOURCE_CHANGED: Source changed while its transaction was prepared"
                ));
            }
            if mutation.next_storage.is_some() != mutation.next_content_generation.is_some() {
                return Err(anyhow!(
                    "SOURCE_TRANSACTION_INVALID: incomplete storage replacement"
                ));
            }
            if mutation.dispose_after_install && mutation.next_connector_count != 0 {
                return Err(anyhow!(
                    "SOURCE_TRANSACTION_INVALID: disposed Source retains Connector membership"
                ));
            }
        }
        let mut wakes = Vec::new();
        wakes
            .try_reserve(mutations.len())
            .map_err(|_| anyhow!("Source transaction wake capacity exhausted"))?;
        // Capture wake groups before the first storage write.  The capture
        // helper may grow a per-subscriber token buffer on its first use;
        // doing that here keeps the write phase non-fallible and preserves the
        // post-acceptance wake contract.
        for (mutation, record) in mutations.iter().zip(guards.iter_mut()) {
            if mutation.next_storage.is_some() && mutation.next_connector_count != 0 {
                let subscribers = capture_subscribers(record);
                if !subscribers.is_empty() {
                    wakes.push((mutation.source.clone(), mutation.next_revision, subscribers));
                }
            }
        }
        for (mutation, record) in mutations.iter().zip(guards.iter_mut()) {
            if let Some(storage) = mutation.next_storage.as_ref() {
                record.storage = Arc::clone(storage);
                if let Some(content_generation) = mutation.next_content_generation {
                    record.content_generation = content_generation;
                }
                record.revision = mutation.next_revision;
                record.copied_bytes = record.copied_bytes.saturating_add(mutation.copied_bytes);
                record.dropped_head_bytes = record
                    .dropped_head_bytes
                    .saturating_add(mutation.dropped_head_bytes);
                record.accepted_bytes = record
                    .accepted_bytes
                    .saturating_add(mutation.accepted_bytes);
            }
            record.connector_count = mutation.next_connector_count;
            if record.connector_count == 0 {
                record.subscribers.clear();
            }
            if mutation.dispose_after_install {
                record.lifecycle = SourceLifecycle::Disposed;
                record.subscribers.clear();
            }
        }
        // Keep every Source guard alive until every Source has been written.
        // Releasing one guard in the write loop would expose a partially
        // installed multi-Source replacement to a concurrent mutation.
        drop(guards);
        for mutation in &mutations {
            if mutation.dispose_after_install {
                mutation.source.remove_disposed_from_registry();
            }
        }
        Ok(PreparedSourceWakes { wakes })
    }

    pub fn finish_prepared_wakes(mut wakes: PreparedSourceWakes) {
        for (source, revision, subscribers) in wakes.wakes.drain(..) {
            source.finish_mutation(revision, subscribers);
        }
    }

    /// Atomically replaces a Block or Stream Source with a fresh content
    /// generation. Existing snapshots retain their old immutable storage.
    pub fn replace_utf8(
        &self,
        bytes: &[u8],
        annotations: &[ContentAnnotationRecord],
        annotation_payload: &[u8],
    ) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind == TextSourceKind::Stream && record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            validate_payload_size(bytes.len())?;
            let input = ValidatedInput::from_bytes(bytes)?;
            let parsed = decode_annotations(&input, 0, annotations, annotation_payload)?;
            let revision = next_revision(record.revision)?;
            let content_generation = record
                .content_generation
                .checked_add(1)
                .ok_or_else(|| anyhow!("Source content generation exhausted"))?;
            let next = StoredSource::empty()
                .apply_append(input.text(), revision, parsed)
                .map_err(|err| anyhow!("{err}"))?;
            let (next, dropped) = apply_retention(next, record.retention)?;
            record.storage = Arc::new(next);
            record.content_generation = content_generation;
            record.revision = revision;
            record.copied_bytes = record.copied_bytes.saturating_add(input.len() as u64);
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            record.accepted_bytes = record.accepted_bytes.saturating_add(input.len() as u64);
            (revision, capture_subscribers(&mut record))
        };
        Ok(self.finish_mutation(revision, subscribers))
    }

    pub fn clear(&self) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind == TextSourceKind::Stream && record.storage.sealed() {
                return Err(anyhow!("SOURCE_SEALED: Source is sealed"));
            }
            if record.storage.base() == 0
                && record.storage.end() == 0
                && record.storage.annotation_count() == 0
            {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            // Preflight both fallible counters before swapping in the empty
            // root: a rejection must leave the retained bytes in place (§9.6).
            let revision = next_revision(record.revision)?;
            let content_generation = record
                .content_generation
                .checked_add(1)
                .ok_or_else(|| anyhow!("Source content generation exhausted"))?;
            record.storage = Arc::new(StoredSource::empty());
            record.content_generation = content_generation;
            record.revision = revision;
            (revision, capture_subscribers(&mut record))
        };
        Ok(self.finish_mutation(revision, subscribers))
    }

    pub fn seal(&self) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if record.kind != TextSourceKind::Stream {
                return Err(anyhow!("INVALID_ARGUMENT: seal requires a stream Source"));
            }
            if record.storage.sealed() {
                return Err(anyhow!("SOURCE_ALREADY_SEALED: Source is already sealed"));
            }
            // Preflight the revision before flipping the flag: a rejection
            // must not report a sealed Source at a stale revision (§9.6).
            let revision = next_revision(record.revision)?;
            let next = record
                .storage
                .apply_seal(record.storage.base(), record.storage.end(), revision, None)
                .map_err(|err| anyhow!("{err}"))?;
            record.storage = Arc::new(next);
            record.revision = revision;
            (revision, capture_subscribers(&mut record))
        };
        Ok(self.finish_mutation(revision, subscribers))
    }

    /// Advances the retained head without renumbering absolute coordinates.
    pub fn truncate_head(&self, offset: u64) -> Result<ContentMutationResult> {
        let (revision, subscribers) = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            ensure_source_live(&record)?;
            if offset < record.storage.base() || offset > record.storage.end() {
                return Err(anyhow!(
                    "INVALID_RANGE: Source head is outside the retained range"
                ));
            }
            if !record.storage.is_boundary(offset) {
                return Err(anyhow!(
                    "INVALID_RANGE: Source head must be a UTF-8 scalar boundary"
                ));
            }
            if offset == record.storage.base() {
                return Ok(ContentMutationResult {
                    revision: record.revision,
                    ..ContentMutationResult::default()
                });
            }
            // Preflight the revision before dropping the head: a rejection
            // must not move the retained range at a stale revision (§9.6).
            let revision = next_revision(record.revision)?;
            let (next, dropped) = record
                .storage
                .apply_truncate(record.storage.base(), offset, revision)
                .map_err(|err| anyhow!("{err}"))?;
            record.storage = Arc::new(next);
            record.revision = revision;
            record.dropped_head_bytes = record.dropped_head_bytes.saturating_add(dropped);
            (revision, capture_subscribers(&mut record))
        };
        Ok(self.finish_mutation(revision, subscribers))
    }

    fn finish_mutation(
        &self,
        revision: u64,
        mut groups: Vec<CapturedSubscriberGroup>,
    ) -> ContentMutationResult {
        if groups.is_empty() {
            return ContentMutationResult {
                revision,
                ..ContentMutationResult::default()
            };
        }
        let mut schedule_environment_drain = false;
        let mut environment_wake_epoch = 0;
        // A failed subscriber must not cancel the remaining wakes (§9.6):
        // every eligible host is attempted, and a failure is reported through
        // the environment's per-host error channel.  In particular, do not
        // return an ordinary mutation error after the Source revision is
        // installed; that would make a successful append look retryable to a
        // direct-FFI caller and could duplicate its bytes.
        crate::perf::add(crate::perf::Counter::ContentWakeGroups, groups.len() as u64);
        for group in &groups {
            let Some(host) = group.host.upgrade() else {
                continue;
            };
            let tokens = &group.tokens;
            let wake_result: Result<()> = (|| {
                let mut host = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
                let mut dirty = std::mem::take(&mut host.content_dirty_scratch);
                dirty.clear();
                for (id, generation) in tokens.iter().copied() {
                    let Some(dirty_item) = host
                        .content
                        .source_subscription_is_live(id, generation, revision)?
                    else {
                        continue;
                    };
                    dirty.push(dirty_item);
                }
                if !dirty.is_empty() {
                    let wake = host.mark_content_pending_batch(&dirty)?;
                    schedule_environment_drain |= wake.schedule_environment_drain;
                    environment_wake_epoch = host.environment_wake_epoch();
                }
                host.content_dirty_scratch = dirty;
                Ok(())
            })();
            if let Err(error) = wake_result {
                schedule_environment_drain = true;
                self.registry.record_wake_failure(
                    &group.host,
                    revision,
                    format!(
                        "SOURCE_WAKE_FAILED: Source revision {revision} was accepted; \
                         subscriber host wake failed: {error:#}"
                    ),
                );
            }
        }
        // Reuse the per-Source outer batch allocation on the next mutation;
        // the Source mutex is reacquired only after all host locks have been
        // released, preserving the no-Source-lock→Host-lock ordering.
        match self.record.lock() {
            Ok(mut record) => {
                for group in &mut groups {
                    if let Some(subscriber) = record.subscribers.get_mut(&group.host_key) {
                        subscriber.wake_tokens = std::mem::take(&mut group.tokens);
                    }
                }
                record.subscriber_wake_scratch = groups;
            }
            Err(_) => {
                // The Source was already accepted, so a poisoned record at
                // this cleanup point is also an environment-visible wake
                // failure, not a mutation rejection.  The captured weak
                // references identify the affected host channels without
                // touching those hosts again.  If all subscribers raced away,
                // there is no remaining host channel to report to; the next
                // Source operation will explicitly surface the poisoned
                // Source lock.
                for group in &groups {
                    schedule_environment_drain = true;
                    self.registry.record_wake_failure(
                        &group.host,
                        revision,
                        format!(
                            "SOURCE_WAKE_FAILED: Source revision {revision} was accepted; \
                             Source lock is poisoned while recycling wake storage"
                        ),
                    );
                }
            }
        }
        ContentMutationResult {
            revision,
            environment_wake_epoch,
            schedule_environment_drain,
        }
    }

    pub(crate) fn same_environment(&self, registry: &ContentSourceRegistry) -> bool {
        Arc::ptr_eq(&self.registry.inner, &registry.inner)
    }

    #[must_use]
    pub fn is_live(&self) -> bool {
        self.record
            .lock()
            .is_ok_and(|record| record.lifecycle == SourceLifecycle::Live)
            && self.registry.contains(self.id(), &self.record)
    }

    pub fn dispose(&self) -> Result<()> {
        let source_id = {
            let mut record = self
                .record
                .lock()
                .map_err(|_| anyhow!("content Source lock is poisoned"))?;
            if record.lifecycle == SourceLifecycle::Disposed {
                return Ok(());
            }
            if record.connector_count != 0 {
                return Err(anyhow!(
                    "SOURCE_IN_USE: Source has {} Connector membership(s)",
                    record.connector_count
                ));
            }
            record.lifecycle = SourceLifecycle::Disposed;
            record.subscribers.clear();
            record.id
        };
        let mut registry = self
            .registry
            .inner
            .lock()
            .map_err(|_| anyhow!("content Source registry lock is poisoned"))?;
        if registry
            .sources
            .get(&source_id)
            .is_some_and(|candidate| Arc::ptr_eq(candidate, &self.record))
        {
            registry.sources.remove(&source_id);
        }
        Ok(())
    }

    fn remove_disposed_from_registry(&self) {
        let source_id = self
            .record
            .lock()
            .expect("disposed Source record must remain lockable")
            .id;
        let mut registry = self
            .registry
            .inner
            .lock()
            .expect("disposed Source registry must remain lockable");
        if registry
            .sources
            .get(&source_id)
            .is_some_and(|candidate| Arc::ptr_eq(candidate, &self.record))
        {
            registry.sources.remove(&source_id);
        }
    }

    /// Stores the creation-time retention policy used by Source mutations.
    #[doc(hidden)]
    pub fn configure_retention(
        &self,
        max_bytes: Option<u64>,
        max_lines: Option<u64>,
        drop_oldest: bool,
    ) -> Result<()> {
        if max_bytes.is_none() && max_lines.is_none()
            || max_bytes.is_some_and(|value| value == 0)
            || max_lines.is_some_and(|value| value == 0)
        {
            return Err(anyhow!(
                "INVALID_ARGUMENT: Source retention limits must be positive"
            ));
        }
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if record.connector_count != 0 {
            return Err(anyhow!(
                "SOURCE_IN_USE: Source retention cannot change while Connectors exist"
            ));
        }
        let retention = SourceRetentionPolicy {
            max_bytes,
            max_lines,
            drop_oldest,
        };
        if !drop_oldest && retention_head(&record.storage, Some(retention)) > record.storage.base()
        {
            return Err(anyhow!(
                "SOURCE_RETENTION_OVERFLOW: Source retention limit would be exceeded"
            ));
        }
        record.retention = Some(retention);
        Ok(())
    }

    pub fn acquire_connector(&self) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record.connector_count = record
            .connector_count
            .checked_add(1)
            .ok_or_else(|| anyhow!("Source Connector membership count exhausted"))?;
        Ok(())
    }

    pub fn release_connector(&self) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        record.connector_count = record.connector_count.saturating_sub(1);
        if record.connector_count == 0 {
            record.subscribers.clear();
        }
        Ok(())
    }

    fn subscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        if record.lifecycle != SourceLifecycle::Live {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        record
            .subscribers
            .retain(|_, subscriber| subscriber.host.strong_count() != 0);
        let host_key = host.as_ptr() as usize;
        let group = record
            .subscribers
            .entry(host_key)
            .or_insert_with(|| SourceSubscriptionGroup {
                host: host.clone(),
                tokens: Vec::new(),
                wake_tokens: Vec::new(),
            });
        group
            .tokens
            .retain(|token| *token != (connector_id, connector_generation));
        group.tokens.push((connector_id, connector_generation));
        Ok(())
    }

    fn unsubscribe(
        &self,
        host: &Weak<Mutex<HostInner>>,
        connector_id: u64,
        connector_generation: u32,
    ) -> Result<()> {
        let mut record = self
            .record
            .lock()
            .map_err(|_| anyhow!("content Source lock is poisoned"))?;
        let host_key = host.as_ptr() as usize;
        let remove_group = record.subscribers.get_mut(&host_key).is_some_and(|group| {
            group
                .tokens
                .retain(|token| *token != (connector_id, connector_generation));
            group.tokens.is_empty()
        });
        if remove_group {
            record.subscribers.remove(&host_key);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn subscriber_count(&self) -> usize {
        self.record.lock().map_or(0, |record| {
            record
                .subscribers
                .values()
                .map(|group| group.tokens.len())
                .sum()
        })
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryPortState {
    pub(crate) history_unit: Option<u64>,
    pub(crate) history_insets: crate::presentation::Insets,
    pub(crate) history_committed_rows: usize,
    pub(crate) history_committed_content_rows: usize,
    pub(crate) history_leading_padding_rows: usize,
    pub(crate) history_trailing_padding_rows: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HistoryTerminalAdapter {
    ports: HashMap<u64, HistoryPortState>,
    unit_ports: HashMap<u64, HashSet<u64>>,
}

impl HistoryTerminalAdapter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn bind_unit(
        &mut self,
        port_id: u64,
        unit_id: u64,
        insets: crate::presentation::Insets,
    ) {
        if let Some(Some(old_unit)) = self.ports.get(&port_id).map(|state| state.history_unit)
            && old_unit != unit_id
            && let Some(ports) = self.unit_ports.get_mut(&old_unit)
        {
            ports.remove(&port_id);
        }
        self.ports.insert(
            port_id,
            HistoryPortState {
                history_unit: Some(unit_id),
                history_insets: insets,
                history_committed_rows: 0,
                history_committed_content_rows: 0,
                history_leading_padding_rows: 0,
                history_trailing_padding_rows: 0,
            },
        );
        self.unit_ports.entry(unit_id).or_default().insert(port_id);
    }

    pub(crate) fn unit_id(&self, port_id: u64) -> Option<u64> {
        self.ports.get(&port_id).and_then(|s| s.history_unit)
    }

    pub(crate) fn committed_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_committed_rows)
    }

    pub(crate) fn committed_content_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_committed_content_rows)
    }

    pub(crate) fn leading_padding_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_leading_padding_rows)
    }

    pub(crate) fn trailing_padding_rows(&self, port_id: u64) -> usize {
        self.ports
            .get(&port_id)
            .map_or(0, |s| s.history_trailing_padding_rows)
    }

    pub(crate) fn insets(&self, port_id: u64) -> crate::presentation::Insets {
        self.ports
            .get(&port_id)
            .map_or(crate::presentation::Insets::ZERO, |s| s.history_insets)
    }

    pub(crate) fn record_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        if let Some(state) = self.ports.get_mut(&port_id) {
            state.history_committed_rows = state.history_committed_rows.saturating_add(rows);
            state.history_committed_content_rows = state
                .history_committed_content_rows
                .saturating_add(content_rows.min(rows));
            state.history_leading_padding_rows = state
                .history_leading_padding_rows
                .saturating_add(leading_padding.min(rows));
            state.history_trailing_padding_rows = state
                .history_trailing_padding_rows
                .saturating_add(trailing_padding.min(rows));
        }
    }

    pub(crate) fn clear_unit(&mut self, unit_id: u64) {
        if let Some(port_ids) = self.unit_ports.remove(&unit_id) {
            for port_id in port_ids {
                if let Some(state) = self.ports.get_mut(&port_id) {
                    if state.history_unit != Some(unit_id) {
                        continue;
                    }
                    state.history_unit = None;
                    state.history_committed_rows = 0;
                    state.history_committed_content_rows = 0;
                    state.history_leading_padding_rows = 0;
                    state.history_trailing_padding_rows = 0;
                }
            }
        }
    }

    pub(crate) fn retire_unit(&mut self, unit_id: u64) -> Vec<u64> {
        let matching = self
            .unit_ports
            .remove(&unit_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        for port_id in &matching {
            self.ports.remove(port_id);
        }
        matching
    }

    pub(crate) fn hash_state<H: std::hash::Hasher>(&self, port_id: u64, hasher: &mut H) {
        use std::hash::Hash;
        if let Some(state) = self.ports.get(&port_id) {
            state.history_committed_rows.hash(hasher);
            state.history_committed_content_rows.hash(hasher);
            state.history_leading_padding_rows.hash(hasher);
            state.history_trailing_padding_rows.hash(hasher);
        }
    }
}

#[derive(Clone, Debug)]
struct PortRecord {
    id: u64,
    generation: u32,
    family: ContentFamily,
    /// Qualified occurrence Port identity for the derived UI adapter, when
    /// this Port was created by `sync_ui_resources`. Generic host-owned
    /// ContentPorts leave this unset. Capturing it on the Port record lets a
    /// receipt promote/retire only the touched adapter without reverse-
    /// scanning the whole UI registry.
    ui_key: Option<ResourceKey>,
    /// A retired occurrence Port remains in the native execution registry
    /// until the receipt that confirms its unmount has removed its derived
    /// Connector. This keeps stale-resource cleanup receipt-safe.
    ui_retire_after_receipt: bool,
    lifecycle: PortLifecycle,
    host: Weak<Mutex<HostInner>>,
    connector_ids: HashSet<u64>,
    desired_mounted: bool,
    visible_mounted: bool,
    desired_connector: Option<u64>,
    visible_connector: Option<u64>,
}

#[derive(Debug)]
struct ConnectorRecord {
    id: u64,
    generation: u32,
    lifecycle: ConnectorLifecycle,
    port: Weak<Mutex<PortRecord>>,
    port_id: u64,
    source: HostContentSource,
    funnel: HostContentFunnel,
    requested: bool,
    visible: bool,
    subscribed: bool,
    /// Source membership is independent from wake subscription. This receipt
    /// bit makes disposal/retry release the Source membership exactly once.
    membership_released: bool,
    /// False for the terminal projection adapter's derived wrapper.  The
    /// canonical occurrence owner owns that Source membership.
    membership_owned: bool,
    /// A post-promotion Source cleanup that could not acquire its Source lock.
    /// This status is distinct from projection/activation failure: the
    /// logical frame is already visible and the Source membership remains
    /// retained until the cleanup succeeds.
    cleanup_error: Option<Arc<ContentConnectorError>>,
    phase: &'static str,
    error: Option<ContentConnectorError>,
    /// Source revision observed at the start of the last failed candidate.
    /// A later Source revision clears the retryable error exactly once.
    failed_source_revision: Option<u64>,
    /// Deterministic synthetic operational failure used by native/unit
    /// fixtures to exercise transactional switch rollback.
    activation_failure: Option<String>,
    /// Connector-local width-dependent derived projections. Inactive connectors
    /// clear this cache; the Source remains the authoritative store.
    projection_cache: VecDeque<(TextProjectionKey, Arc<HostContentProjection>)>,
    prefix_proof_cache: PrefixProofCache,
    committed_projection: Option<Arc<HostContentProjection>>,
    candidate_projection: Option<Arc<HostContentProjection>>,
    projected_source_revision: Option<u64>,
    projection_failure_key: Option<TextProjectionKey>,
    /// Monotonic control revision used to keep newer requested selection
    /// changes independent from an older in-flight candidate cleanup.
    control_revision: u64,
    delivery_revision: u64,
    candidate_delivery_frontier: StreamOffset,
    committed_delivery_frontier: StreamOffset,
    execution: Option<ConnectorExecution>,
}

/// A prevalidated visible-association change.  The Arc records are captured
/// while the candidate is prepared, so receipt-time promotion never resolves
/// a handle through the live registries or builds a replacement collection.
#[derive(Debug)]
struct PreparedContentPort {
    id: u64,
    record: Arc<Mutex<PortRecord>>,
    ui_key: Option<ResourceKey>,
    ui_retire_after_receipt: bool,
    mounted: bool,
    old_connector_id: Option<u64>,
    old_connector_index: Option<usize>,
    old_control_revision: Option<u64>,
    next_connector_id: Option<u64>,
    next_connector_index: Option<usize>,
    retry_selection: bool,
}

#[derive(Debug)]
struct PreparedContentConnector {
    id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    source: HostContentSource,
    source_id: u64,
    generation: u32,
    requested: bool,
    subscribed: bool,
    control_revision: u64,
    deadline: Option<Instant>,
    visible: bool,
    /// Immutable projection captured with this candidate. Receipt promotion
    /// must use this Arc rather than consuming whatever candidate happens to
    /// be installed after a newer delivery tick or Source revision.
    candidate_projection: Option<Arc<HostContentProjection>>,
    delivery_frontier: StreamOffset,
    delivery_input: Option<(u32, u64, bool)>,
    delivery_revision: u64,
}

#[derive(Debug)]
struct PreparedContentSource {
    id: u64,
    source: HostContentSource,
}

#[derive(Clone, Copy, Debug)]
struct PreparedContentBindingChange {
    port_index: usize,
    revision: u64,
}

#[derive(Clone, Debug)]
struct PreparedSourceCleanup {
    source: HostContentSource,
    source_id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    connector_id: u64,
    connector_generation: u32,
    unsubscribe: bool,
    error: Arc<ContentConnectorError>,
}

/// Candidate-owned content commit data.  It is deliberately separate from
/// the mutable desired Port/Connector tables: operations accepted while a
/// backend receipt is outstanding cannot consume or overwrite this plan.
#[derive(Debug)]
pub(crate) struct PreparedContentCommit {
    ports: Vec<PreparedContentPort>,
    connectors: Vec<PreparedContentConnector>,
    sources: Vec<PreparedContentSource>,
    source_cleanups: Vec<PreparedSourceCleanup>,
    binding_changes: Vec<PreparedContentBindingChange>,
}

fn remove_source_subscription_locked(
    source: &mut ContentSourceRecord,
    host: &Weak<Mutex<HostInner>>,
    connector_id: u64,
    connector_generation: u32,
) {
    let host_key = host.as_ptr() as usize;
    let remove_group = source.subscribers.get_mut(&host_key).is_some_and(|group| {
        group
            .tokens
            .retain(|token| *token != (connector_id, connector_generation));
        group.tokens.is_empty()
    });
    if remove_group {
        source.subscribers.remove(&host_key);
    }
}

fn release_source_membership_locked(source: &mut ContentSourceRecord) {
    source.connector_count = source.connector_count.saturating_sub(1);
    if source.connector_count == 0 {
        source.subscribers.clear();
    }
}

fn set_connector_visible_committed(
    active_deadlines: &mut HashMap<u64, Instant>,
    active_connectors: &mut HashSet<u64>,
    connector: &PreparedContentConnector,
    visible: bool,
    preserve_newer_control: bool,
) {
    let connector_id = connector.id;
    let mut state = connector
        .record
        .lock()
        .expect("prepared Connector lock must remain usable during visible commit");
    state.visible = visible;
    if visible {
        state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
            "disposing"
        } else {
            "active"
        };
        return;
    }
    if !preserve_newer_control {
        state.committed_projection = None;
        state.candidate_projection = None;
        state.projection_cache.clear();
        state.prefix_proof_cache.clear();
        state.projected_source_revision = None;
        state.projection_failure_key = None;
        state.execution = None;
        state.delivery_revision = 0;
        state.candidate_delivery_frontier = StreamOffset::ZERO;
        state.committed_delivery_frontier = StreamOffset::ZERO;
    }
    if state.lifecycle != ConnectorLifecycle::Disposing {
        state.phase = if state.error.is_some() && state.requested {
            "failed"
        } else if state.requested {
            "activation-pending"
        } else {
            "idle"
        };
    }
    active_deadlines.remove(&connector_id);
    active_connectors.remove(&connector_id);
}

fn remove_prepared_connector_committed(
    connectors: &mut HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
    connector: &PreparedContentConnector,
) {
    let mut state = connector
        .record
        .lock()
        .expect("prepared Connector lock must remain usable before removal");
    if state.lifecycle != ConnectorLifecycle::Disposing || state.visible {
        return;
    }
    let _owned = connectors
        .remove(&connector.id)
        .expect("prepared Connector must still be owned at commit");
    let port = state.port.upgrade();
    state.lifecycle = ConnectorLifecycle::Disposed;
    state.phase = "disposed";
    state.visible = false;
    state.requested = false;
    state.subscribed = false;
    state.cleanup_error = None;
    if let Some(port) = port {
        let mut port_guard = port
            .lock()
            .expect("prepared ContentPort lock must remain usable during removal");
        port_guard.connector_ids.remove(&connector.id);
        if port_guard.desired_connector == Some(connector.id) {
            port_guard.desired_connector = None;
        }
        if port_guard.visible_connector == Some(connector.id) {
            port_guard.visible_connector = None;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorError {
    pub code: String,
    pub diagnostic: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentConnectorStatus {
    pub phase: String,
    pub requested: bool,
    pub visible: bool,
    pub projected_source_revision: Option<u64>,
    pub error: Option<ContentConnectorError>,
    pub cleanup_pending: bool,
    pub cleanup_error: Option<ContentConnectorError>,
}

/// Host-owned Port/Connector registries. The registry is intentionally
/// separate from Source storage: Sources are environment-owned, while ports
/// and connectors depend on one host's activation and visible frame.
#[derive(Debug)]
pub(crate) struct ContentHostRegistry {
    source_registry: ContentSourceRegistry,
    /// Shared by every host that belongs to this environment. Projection
    /// tasks retain immutable Source snapshots, never a HostInner guard.
    executor: Arc<ContentExecutor>,
    worker_wake: ContentWorkerWake,
    owner_host: Weak<Mutex<HostInner>>,
    theme: Arc<Theme>,
    theme_revision: u64,
    next_connector_id: u64,
    next_generation: u32,
    ports: HashMap<u64, Arc<Mutex<PortRecord>>>,
    connectors: HashMap<u64, Arc<Mutex<ConnectorRecord>>>,
    in_flight_connectors: HashSet<u64>,
    /// Candidate selection/projection is discarded on frame abort and
    /// promoted only after the backend receipt commits.
    candidate_selections: HashMap<u64, Option<u64>>,
    /// Candidate-local immutable product captures. Each entry retains the
    /// exact candidate/confirmed Arc products and Source frontier used by a
    /// bounded final-width refinement.
    candidate_content_captures: HashMap<u64, CandidateContentCapture>,
    /// Desired association changes accepted outside a candidate. They are
    /// moved into `candidate_binding_changes` at attempt start and remain
    /// independent from newer changes while a receipt is in flight.
    pending_binding_changes: HashSet<u64>,
    pending_binding_revisions: HashMap<u64, u64>,
    next_binding_revision: u64,
    candidate_binding_changes: HashSet<u64>,
    candidate_binding_revisions: HashMap<u64, u64>,
    /// Active due deadlines for smoothed connectors. Native ticks and wake
    /// queries inspect this structure without scanning inactive registries.
    active_deadlines: HashMap<u64, Instant>,
    /// Connector IDs eligible for deadline synchronization.  This recovery
    /// index is maintained with lifecycle transitions, so an empty deadline
    /// map never requires walking every inactive Connector.
    active_connectors: HashSet<u64>,
    /// Reused worklists for the active/deadline indexes. They contain only
    /// currently eligible Connector IDs and never require a registry scan.
    active_sync_scratch: Vec<u64>,
    due_connector_scratch: Vec<u64>,
    /// Last host-supplied clock. Source/control wakes may arrive without an
    /// explicit `now`; once a host has advanced, those wakes must stay on the
    /// same timeline rather than falling back to wall time.
    authoritative_clock: Option<Instant>,
    /// Connector/Port records touched while preparing the current candidate.
    /// Candidate cleanup and visible promotion consume these sets instead of
    /// scanning unrelated inactive registry entries.
    candidate_touched_connectors: HashSet<u64>,
    candidate_touched_ports: HashSet<u64>,
    /// Source association cleanup that could not acquire its Source lock
    /// after a successful logical frame promotion.  Membership/subscription
    /// stays retained until a later candidate can release it safely.
    pending_source_cleanups: Vec<PreparedSourceCleanup>,
    pending_source_cleanup_ids: HashSet<u64>,
    #[cfg(test)]
    test_poison_source_after_first_cleanup: Option<u64>,
    /// Attempt-local immutable Source captures.  The map is populated lazily
    /// by the first demanded Connector and shared by all later key/history
    /// lookups in that candidate.  Interior mutability keeps the read-only
    /// provider revision queries on the existing seam without making the map a
    /// second Source authority.
    candidate_source_snapshots:
        RefCell<HashMap<super::ui_resources::SourceIdentity, HostContentSourceSnapshot>>,
    candidate_capture_active: bool,
    candidate_commit_prepared: bool,
    preserve_captures_for_async: bool,
    history_adapter: HistoryTerminalAdapter,
    /// Derived terminal resources for the canonical occurrence document.
    /// The occurrence owner remains authoritative; these handles are only
    /// the existing `ContentProvider` execution objects used by the M1 adapter.
    ui_ports: HashMap<ResourceKey, u64>,
    /// A physically exported History unit may retain its occurrence root until
    /// React removes it. Keep its derived Port retired rather than recreating
    /// the content product on a later sparse sync.
    retired_ui_ports: HashSet<ResourceKey>,
    ui_connectors: HashMap<ResourceKey, u64>,
    ui_connector_keys: HashMap<ResourceKey, ResourceKey>,
    /// Every derived adapter retains its qualified occurrence Connector key
    /// until the receipt that supersedes it permits retirement. This is an
    /// identity-to-execution-owner index, not a second Source-membership or
    /// status authority.
    ui_connector_keys_by_id: HashMap<u64, ResourceKey>,
    /// Qualified Connector identity and native execution owner confirmed by
    /// the last successful receipt for each UI Port. The occurrence owner
    /// remains authoritative for identity and Source membership.
    ui_confirmed_connectors: HashMap<ResourceKey, (ResourceKey, u64)>,
    ui_failure_injections: HashMap<ResourceKey, String>,
    ui_next_failure_injection: Option<String>,
    pending_content_projections: HashMap<u64, PendingContentProjection>,
    /// One wake registration covers all Connectors waiting on this host's
    /// shared executor. The registration is removed once no Connector still
    /// needs admission, so it cannot retain a closed host or registry.
    deferred_projection_waiter: Option<u64>,
    projection_results_ready: bool,
    #[cfg(test)]
    ui_demand_nodes_visited: usize,
    #[cfg(test)]
    ui_owner_nodes_visited: usize,
}

impl ContentHostRegistry {
    pub(crate) fn new(source_registry: ContentSourceRegistry) -> Self {
        let executor = Arc::clone(&source_registry.executor);
        Self {
            source_registry,
            executor,
            worker_wake: ContentWorkerWake(Arc::new(|| {})),
            owner_host: Weak::new(),
            theme: Arc::new(Theme::new()),
            theme_revision: 0,
            next_connector_id: 0,
            next_generation: 0,
            ports: HashMap::new(),
            connectors: HashMap::new(),
            in_flight_connectors: HashSet::new(),
            candidate_selections: HashMap::new(),
            candidate_content_captures: HashMap::new(),
            pending_binding_changes: HashSet::new(),
            pending_binding_revisions: HashMap::new(),
            next_binding_revision: 0,
            candidate_binding_changes: HashSet::new(),
            candidate_binding_revisions: HashMap::new(),
            active_deadlines: HashMap::new(),
            active_connectors: HashSet::new(),
            active_sync_scratch: Vec::new(),
            due_connector_scratch: Vec::new(),
            authoritative_clock: None,
            candidate_touched_connectors: HashSet::new(),
            candidate_touched_ports: HashSet::new(),
            pending_source_cleanups: Vec::new(),
            pending_source_cleanup_ids: HashSet::new(),
            #[cfg(test)]
            test_poison_source_after_first_cleanup: None,
            candidate_source_snapshots: RefCell::new(HashMap::new()),
            candidate_capture_active: false,
            candidate_commit_prepared: false,
            preserve_captures_for_async: false,
            history_adapter: HistoryTerminalAdapter::new(),
            ui_ports: HashMap::new(),
            retired_ui_ports: HashSet::new(),
            ui_connectors: HashMap::new(),
            ui_connector_keys: HashMap::new(),
            ui_connector_keys_by_id: HashMap::new(),
            ui_confirmed_connectors: HashMap::new(),
            ui_failure_injections: HashMap::new(),
            ui_next_failure_injection: None,
            pending_content_projections: HashMap::new(),
            deferred_projection_waiter: None,
            projection_results_ready: false,
            #[cfg(test)]
            ui_demand_nodes_visited: 0,
            #[cfg(test)]
            ui_owner_nodes_visited: 0,
        }
    }

    pub(crate) fn sync_ui_resources(
        &mut self,
        owner: &UiResourceOwner,
        changes: Option<&UiChangeSet>,
    ) -> Result<Vec<u64>> {
        let host = self.owner_host.clone();
        if host.strong_count() == 0 {
            return Err(anyhow!("UI content adapter has no owning host"));
        }
        let initial = changes.is_none();
        self.retired_ui_ports
            .retain(|key| owner.ports.contains(key));
        let (mut port_keys, mut stale_keys, owner_visited) =
            Self::ui_sync_port_keys(&self.ui_ports, owner, changes)?;
        crate::perf::add(
            crate::perf::Counter::ContentOwnerNodesVisited,
            u64::try_from(owner_visited).unwrap_or(u64::MAX),
        );
        #[cfg(test)]
        {
            self.ui_owner_nodes_visited = self.ui_owner_nodes_visited.saturating_add(owner_visited);
        }
        port_keys.sort_unstable_by_key(|key| (key.slot, key.generation));
        port_keys.dedup();
        let synced_keys = port_keys.clone();
        stale_keys.sort_unstable_by_key(|key| (key.slot, key.generation));
        stale_keys.dedup();
        self.remove_stale_ui_ports(stale_keys)?;

        let demanded_keys = self.demanded_ui_ports(owner, &port_keys, initial)?;

        for key in port_keys {
            if !owner.ports.contains(&key) || self.retired_ui_ports.contains(&key) {
                continue;
            }
            self.sync_ui_port(owner, &host, key)?;
        }
        let desired_changes = if initial {
            self.ui_ports
                .iter()
                .map(|(key, port_id)| (*port_id, demanded_keys.contains(key)))
                .collect::<Vec<_>>()
        } else {
            synced_keys
                .into_iter()
                .filter_map(|key| {
                    self.ui_ports
                        .get(&key)
                        .map(|port_id| (*port_id, demanded_keys.contains(&key)))
                })
                .collect::<Vec<_>>()
        };
        self.set_desired_sparse(&desired_changes)?;
        Ok(desired_changes
            .into_iter()
            .filter_map(|(port_id, mounted)| mounted.then_some(port_id))
            .collect())
    }

    fn ui_sync_port_keys(
        ui_ports: &HashMap<ResourceKey, u64>,
        owner: &UiResourceOwner,
        changes: Option<&UiChangeSet>,
    ) -> Result<(Vec<ResourceKey>, Vec<ResourceKey>, usize)> {
        if changes.is_none() {
            return Ok((
                owner.ports.iter().copied().collect(),
                ui_ports
                    .keys()
                    .copied()
                    .filter(|key| !owner.ports.contains(key))
                    .collect(),
                0,
            ));
        }
        let mut touched_ports = HashSet::new();
        let mut owner_visited = 0usize;
        let changes = changes.expect("non-initial UI sync has changes");
        for key in &changes.changed_resources {
            if key.kind == HandleKind::Port {
                touched_ports.insert(*key);
            } else if key.kind == HandleKind::Connector {
                if let Some(port) = changes
                    .resource_ports
                    .iter()
                    .find_map(|(resource, port)| (*resource == *key).then_some(*port))
                    .flatten()
                {
                    touched_ports.insert(port);
                }
            }
        }
        if changes.physical_work {
            let (membership_ports, visited) = owner.ports_under_nodes(&changes.membership_nodes)?;
            owner_visited = owner_visited.saturating_add(visited);
            touched_ports.extend(membership_ports);
            for key in &changes.changed_resources {
                if key.kind != HandleKind::Control
                    || !owner.control_state(*key).is_some_and(|state| {
                        state.kind() == crate::occurrence::ControlKind::Animation
                    })
                {
                    continue;
                }
                let Some(node) = owner
                    .document
                    .as_ref()
                    .and_then(|document| document.control_owner(*key))
                else {
                    continue;
                };
                let (animation_ports, visited) = owner.ports_under_nodes(&[node])?;
                owner_visited = owner_visited.saturating_add(visited);
                touched_ports.extend(animation_ports);
            }
        }
        let stale = touched_ports
            .iter()
            .copied()
            .filter(|key| !owner.ports.contains(key))
            .collect();
        Ok((touched_ports.into_iter().collect(), stale, owner_visited))
    }

    fn demanded_ui_ports(
        &mut self,
        owner: &UiResourceOwner,
        port_keys: &[ResourceKey],
        initial: bool,
    ) -> Result<HashSet<ResourceKey>> {
        if initial {
            return owner
                .demanded_ports()
                .map(|ports| ports.into_iter().collect());
        }
        let mut demanded = HashSet::new();
        for key in port_keys {
            let (is_demanded, visited) = owner.demanded_port(*key)?;
            crate::perf::add(
                crate::perf::Counter::ContentDemandNodesVisited,
                visited as u64,
            );
            #[cfg(test)]
            {
                self.ui_demand_nodes_visited = self.ui_demand_nodes_visited.saturating_add(visited);
            }
            if is_demanded {
                demanded.insert(*key);
            }
        }
        Ok(demanded)
    }

    #[cfg(test)]
    pub(crate) fn test_ui_demand_nodes_visited(&self) -> usize {
        self.ui_demand_nodes_visited
    }

    #[cfg(test)]
    pub(crate) fn test_ui_owner_nodes_visited(&self) -> usize {
        self.ui_owner_nodes_visited
    }

    #[cfg(test)]
    pub(crate) fn test_ui_adapter_count(&self) -> usize {
        self.ui_connector_keys_by_id.len()
    }

    #[cfg(test)]
    pub(crate) fn test_ui_confirmed_connector(
        &self,
        port: ResourceKey,
    ) -> Option<(ResourceKey, u64)> {
        self.ui_confirmed_connectors.get(&port).copied()
    }

    #[cfg(test)]
    pub(crate) fn test_port_count(&self) -> usize {
        self.ports.len()
    }

    #[cfg(test)]
    pub(crate) fn test_connector_count(&self) -> usize {
        self.connectors.len()
    }

    fn remove_stale_ui_ports(&mut self, keys: Vec<ResourceKey>) -> Result<()> {
        for key in keys {
            if let Some(connector_id) = self.ui_connectors.remove(&key) {
                let visible = self.connector_is_visible(connector_id)?;
                self.ui_connector_keys.remove(&key);
                if visible {
                    self.request_deactivation(connector_id)?;
                } else {
                    self.ui_connector_keys_by_id.remove(&connector_id);
                    self.remove_connector(connector_id);
                }
            }
            if let Some(port_id) = self.ui_ports.remove(&key)
                && let Some(port) = self.ports.get(&port_id).cloned()
            {
                self.set_desired_sparse(&[(port_id, false)])?;
                port.lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned during stale cleanup"))?
                    .ui_retire_after_receipt = true;
            }
            self.ui_confirmed_connectors.remove(&key);
        }
        Ok(())
    }

    fn sync_ui_port(
        &mut self,
        owner: &UiResourceOwner,
        host: &Weak<Mutex<HostInner>>,
        key: ResourceKey,
    ) -> Result<()> {
        let port_id = if let Some(port_id) = self.ui_ports.get(&key).copied() {
            port_id
        } else {
            let port = self.create_port(host.clone(), ContentFamily::Text)?;
            let port_id = port.id();
            port.record
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned during UI sync"))?
                .ui_key = Some(key);
            self.ui_ports.insert(key, port_id);
            port_id
        };
        let binding = owner.content_binding(key)?;
        let current_connector = self.ui_connectors.get(&key).copied();
        let Some((connector_key, source, funnel)) = binding else {
            if let Some(connector_id) = self.ui_connectors.remove(&key) {
                let visible = self.connector_is_visible(connector_id)?;
                if let Some(old_key) = self.ui_connector_keys.get(&key).copied()
                    && visible
                {
                    self.ui_confirmed_connectors
                        .insert(key, (old_key, connector_id));
                }
                if visible {
                    self.request_deactivation(connector_id)?;
                } else {
                    self.ui_connector_keys_by_id.remove(&connector_id);
                    self.remove_connector(connector_id);
                }
            }
            self.ui_connector_keys.remove(&key);
            return Ok(());
        };
        let needs_new = current_connector
            .and_then(|id| self.connectors.get(&id))
            .is_none()
            || self.ui_connector_keys.get(&key).copied() != Some(connector_key);
        if current_connector.is_some() && !needs_new {
            return Ok(());
        }
        if let Some(connector_id) = self.ui_connectors.remove(&key) {
            let visible = self.connector_is_visible(connector_id)?;
            if let Some(old_key) = self.ui_connector_keys.get(&key).copied()
                && visible
            {
                self.ui_confirmed_connectors
                    .insert(key, (old_key, connector_id));
            }
            if visible {
                self.request_deactivation(connector_id)?;
            } else {
                self.ui_connector_keys_by_id.remove(&connector_id);
                self.remove_connector(connector_id);
            }
        }
        let port = self
            .ports
            .get(&port_id)
            .cloned()
            .ok_or_else(|| anyhow!("UI ContentPort disappeared during sync"))?;
        let funnel = HostContentFunnel::new(
            match funnel.kind {
                0 => TextFunnelKind::Plain,
                1 => TextFunnelKind::Markdown,
                2 => TextFunnelKind::Diff,
                3 => TextFunnelKind::Ansi,
                _ => return Err(anyhow!("invalid UI Funnel kind")),
            },
            match funnel.wrap {
                0 => TextWrapMode::Word,
                1 => TextWrapMode::Grapheme,
                2 => TextWrapMode::NoWrap,
                _ => return Err(anyhow!("invalid UI Funnel wrap")),
            },
            funnel.hyperlinks,
            if funnel.smooth {
                ContentDelivery::Smooth(SmoothConfig::default())
            } else {
                ContentDelivery::Immediate
            },
        );
        let connector = self.connect_derived(&port, &source, funnel)?;
        let connector_id = connector.id();
        // UiResourceOwner already owns the accepted Source membership. The
        // derived ContentProvider wrapper has no independent lease.
        self.ui_connectors.insert(key, connector_id);
        self.ui_connector_keys.insert(key, connector_key);
        self.ui_connector_keys_by_id
            .insert(connector_id, connector_key);
        if let Some(diagnostic) = self
            .ui_failure_injections
            .remove(&key)
            .or_else(|| self.ui_next_failure_injection.take())
        {
            self.fail_next_activation(connector_id, diagnostic)?;
        }
        self.request_activation(connector_id, host).map(|_| ())
    }

    fn touch_connector(&mut self, connector_id: u64) {
        if self.candidate_commit_prepared {
            return;
        }
        self.candidate_touched_connectors.insert(connector_id);
    }

    pub(crate) fn set_owner_host(&mut self, host: Weak<Mutex<HostInner>>) {
        self.owner_host = host;
    }

    pub(crate) fn ui_port_id(&self, key: ResourceKey) -> Option<u64> {
        self.ui_ports
            .get(&key)
            .copied()
            .filter(|port_id| self.ports.contains_key(port_id))
    }

    fn completion_wake(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::clone(&self.worker_wake.0)
    }

    pub(crate) fn set_worker_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.worker_wake = ContentWorkerWake(wake);
    }

    fn cancel_projection(&mut self, connector_id: u64) {
        if let Some(pending) = self.pending_content_projections.remove(&connector_id) {
            pending.cancelled.store(true, Ordering::Release);
        }
        self.release_deferred_projection_waiter_if_idle();
    }

    fn release_deferred_projection_waiter_if_idle(&mut self) {
        if self
            .pending_content_projections
            .values()
            .any(|pending| pending.result.is_none())
        {
            return;
        }
        if let Some(waiter_id) = self.deferred_projection_waiter.take() {
            self.executor.unregister_waiter(waiter_id);
        }
    }

    fn projection_admission_failure(
        error: &ContentExecutorRejected,
    ) -> ContentProjectionFailureKind {
        match error.kind {
            ContentExecutorRejectKind::Oversized => ContentProjectionFailureKind::LimitExceeded,
            ContentExecutorRejectKind::Startup | ContentExecutorRejectKind::Unavailable => {
                ContentProjectionFailureKind::ExecutorUnavailable
            }
        }
    }

    /// Installs completed immutable products into the Connector cache. This
    /// is a short registry transition performed on the environment queue;
    /// projection itself has already finished on the shared executor.
    fn drain_projection_results(&mut self) -> Result<()> {
        let mut changed = false;
        let connector_ids = self
            .pending_content_projections
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for connector_id in connector_ids {
            let result = match self.pending_content_projections.get_mut(&connector_id) {
                Some(pending) => {
                    let Some(result) = pending.result.as_mut() else {
                        continue;
                    };
                    match result.try_recv() {
                        Ok(result) => Some(Ok(result)),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => Some(Err(())),
                    }
                }
                None => None,
            };
            let Some(result) = result else {
                continue;
            };
            changed = true;
            let pending = self
                .pending_content_projections
                .remove(&connector_id)
                .expect("completed content task must remain registered");
            let Ok(result) = result else {
                if self.connectors.contains_key(&connector_id) {
                    self.record_connector_operating_failure(
                        connector_id,
                        anyhow!("content executor dropped a projection result"),
                    )?;
                }
                continue;
            };
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                continue;
            };
            debug_assert_eq!(result.connector_id, connector_id);
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned after projection"))?;
            state.execution = Some(result.execution);
            state.prefix_proof_cache = result.prefix_proof_cache;
            match result.projection {
                Ok(projection) => {
                    state
                        .projection_cache
                        .retain(|(candidate, _)| candidate != &result.key);
                    state
                        .projection_cache
                        .push_front((result.key, Arc::new(projection)));
                    while state.projection_cache.len() > CONTENT_CACHE_CAPACITY {
                        state.projection_cache.pop_back();
                    }
                    if state.projection_failure_key == Some(result.key) {
                        state.error = None;
                        state.failed_source_revision = None;
                        state.projection_failure_key = None;
                    }
                }
                Err(error) => {
                    let code = error
                        .downcast_ref::<ContentProjectionFailure>()
                        .map_or(ContentProjectionFailureKind::Projection.code(), |failure| {
                            failure.kind.code()
                        });
                    state.error = Some(ContentConnectorError {
                        code: code.to_owned(),
                        diagnostic: error.to_string(),
                    });
                    state.failed_source_revision = Some(result.key.source_revision);
                    state.projection_failure_key = Some(result.key);
                    state.phase = if state.visible { "active" } else { "failed" };
                }
            }
            drop(state);
            // A result may be older than the current Source revision. It is
            // retained as a compatible intermediate product, but the next
            // candidate will schedule the exact newer key before claiming
            // the content barrier complete.
            let _ = pending.key;
        }
        self.projection_results_ready |= changed;
        Ok(())
    }

    pub(crate) fn take_projection_results_ready(&mut self) -> bool {
        std::mem::take(&mut self.projection_results_ready)
    }

    pub(crate) fn has_pending_projections(&self) -> bool {
        !self.pending_content_projections.is_empty()
    }

    #[cfg(test)]
    fn wait_for_projection_jobs_for_test(&mut self) {
        while !self.pending_content_projections.is_empty() {
            self.drain_projection_results()
                .expect("content executor test result handling must remain valid");
            let admission_pending = self
                .pending_content_projections
                .iter()
                .filter_map(|(connector_id, pending)| {
                    pending
                        .result
                        .is_none()
                        .then_some((*connector_id, pending.key.width))
                })
                .collect::<Vec<_>>();
            for (connector_id, width) in admission_pending {
                self.prepare_connector_projection_async(connector_id, width, None)
                    .expect("content executor test admission must remain valid");
            }
            if !self.pending_content_projections.is_empty() {
                std::thread::yield_now();
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn install_projection_latch_for_test(
        &self,
    ) -> (std::sync::mpsc::Receiver<()>, std::sync::mpsc::Sender<()>) {
        self.executor.install_projection_latch()
    }

    #[cfg(test)]
    pub(crate) fn clear_projection_latch_for_test(&self) {
        self.executor.clear_projection_latch();
    }

    pub(crate) fn ui_connector_owner_key(&self, connector: ResourceKey) -> Option<ResourceKey> {
        self.ui_connector_keys
            .iter()
            .find(|(_, connector_key)| **connector_key == connector)
            .map(|(port_key, _)| *port_key)
            .or_else(|| {
                self.ui_confirmed_connectors
                    .iter()
                    .find(|(_, (connector_key, _))| *connector_key == connector)
                    .map(|(port_key, _)| *port_key)
            })
    }

    fn connector_is_visible(&self, connector_id: u64) -> Result<bool> {
        let connector = self.connectors.get(&connector_id).ok_or_else(|| {
            anyhow!(
                "INTERNAL_INVARIANT: UI Connector {connector_id} disappeared during visibility read"
            )
        })?;
        Ok(connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned during visibility read"))?
            .visible)
    }

    /// Reads the native execution status for an occurrence-owned UI resource.
    /// The UI resource key remains the identity authority; this map is only the
    /// derived ContentProvider execution mapping.
    pub(crate) fn ui_port_mounted(&self, key: ResourceKey) -> Result<bool> {
        let Some(port_id) = self.ui_ports.get(&key).copied() else {
            return Ok(false);
        };
        self.port_status(port_id)
    }

    pub(crate) fn ui_connector_status(
        &self,
        owner: &UiResourceOwner,
        key: ResourceKey,
    ) -> Result<ContentConnectorStatus> {
        if let Some((port_key, _)) = self
            .ui_connector_keys
            .iter()
            .find(|(_, connector_key)| **connector_key == key)
        {
            let connector_id = self.ui_connectors.get(port_key).copied().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: UI Connector {key:?} has no derived execution owner")
            })?;
            return self.connector_status(connector_id);
        }
        if let Some((connector_key, connector_id)) = self
            .ui_confirmed_connectors
            .values()
            .find(|(connector_key, _)| *connector_key == key)
            .copied()
        {
            debug_assert_eq!(connector_key, key);
            return self.connector_status(connector_id);
        }
        if owner.connectors.contains_key(&key) {
            let requested = owner.connector_requested(key);
            return Ok(ContentConnectorStatus {
                phase: if requested {
                    "waiting-for-mount"
                } else {
                    "idle"
                }
                .to_owned(),
                requested,
                visible: false,
                projected_source_revision: None,
                error: None,
                cleanup_pending: false,
                cleanup_error: None,
            });
        }
        Err(anyhow!("STALE_HANDLE: UI Connector is unavailable"))
    }

    pub(crate) fn ui_content_visible(&self, owner: &UiResourceOwner) -> Result<bool> {
        for key in owner.demanded_ports()? {
            let Some(port_id) = self.ui_ports.get(&key).copied() else {
                return Ok(false);
            };
            let Some(port) = self.ports.get(&port_id) else {
                return Ok(false);
            };
            let (mounted, connector_id, desired_mounted, desired_connector) = {
                let state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    state.visible_mounted,
                    state.visible_connector,
                    state.desired_mounted,
                    state.desired_connector,
                )
            };
            if mounted != desired_mounted || connector_id != desired_connector {
                return Ok(false);
            }
            let Some(connector_id) = connector_id else {
                return Ok(false);
            };
            if !mounted {
                return Ok(false);
            }
            let Some(connector) = self.connectors.get(&connector_id) else {
                return Ok(false);
            };
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if !state.visible
                || !state
                    .committed_projection
                    .as_ref()
                    .is_some_and(|projection| {
                        projection.identity != 0 && projection.physically_complete
                    })
            {
                return Ok(false);
            }
            let Some((_, source, _)) = owner.content_binding(key)? else {
                return Ok(false);
            };
            let source_revision = source.snapshot()?.revision;
            if state.projected_source_revision != Some(source_revision) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn ui_content_failure(&self) -> Result<Option<String>> {
        for connector_id in self.ui_connectors.values() {
            let connector = self.connectors.get(&connector_id).ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: UI Connector {connector_id} disappeared during failure read"
                )
            })?;
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned during failure read"))?;
            if state.requested
                && !state.visible
                && let Some(error) = state.error.as_ref()
            {
                return Ok(Some(format!("{}: {}", error.code, error.diagnostic)));
            }
        }
        Ok(None)
    }

    pub(crate) fn fail_ui_connector(
        &mut self,
        owner_key: ResourceKey,
        diagnostic: String,
    ) -> Result<()> {
        if let Some(connector_id) = self.ui_connectors.get(&owner_key).copied() {
            self.fail_next_activation(connector_id, diagnostic)
        } else {
            self.ui_failure_injections.insert(owner_key, diagnostic);
            Ok(())
        }
    }

    pub(crate) fn fail_next_ui_connector_for_test(&mut self, diagnostic: String) -> Result<()> {
        if diagnostic.is_empty() {
            return Err(anyhow!("UI Connector failure diagnostic cannot be empty"));
        }
        self.ui_next_failure_injection = Some(diagnostic);
        Ok(())
    }

    fn touch_port(&mut self, port_id: u64) {
        if self.candidate_commit_prepared {
            return;
        }
        self.candidate_touched_ports.insert(port_id);
    }

    fn mark_binding_change(&mut self, port_id: u64) {
        self.next_binding_revision = self
            .next_binding_revision
            .checked_add(1)
            .expect("ContentPort binding revision exhausted");
        self.pending_binding_revisions
            .insert(port_id, self.next_binding_revision);
        if self.candidate_commit_prepared {
            self.pending_binding_changes.insert(port_id);
        } else if self.candidate_capture_active {
            self.candidate_binding_changes.insert(port_id);
            self.candidate_binding_revisions
                .insert(port_id, self.next_binding_revision);
        } else {
            self.pending_binding_changes.insert(port_id);
        }
    }

    pub(crate) fn create_port(
        &mut self,
        host: Weak<Mutex<HostInner>>,
        family: ContentFamily,
    ) -> Result<HostContentPort> {
        self.owner_host = host.clone();
        let port_id = NEXT_CONTENT_PORT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| anyhow!("ContentPort identity exhausted"))?;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("ContentPort generation exhausted"))?;
        let record = Arc::new(Mutex::new(PortRecord {
            id: port_id,
            generation: self.next_generation,
            family,
            ui_key: None,
            ui_retire_after_receipt: false,
            lifecycle: PortLifecycle::Live,
            host: host.clone(),
            connector_ids: HashSet::new(),
            desired_mounted: false,
            visible_mounted: false,
            desired_connector: None,
            visible_connector: None,
        }));
        self.ports.insert(port_id, Arc::clone(&record));
        Ok(HostContentPort {
            id: port_id,
            generation: self.next_generation,
            family,
            record,
            host,
        })
    }

    fn connect(
        &mut self,
        port: &Arc<Mutex<PortRecord>>,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        self.connect_with_membership(port, source, funnel, true)
    }

    /// Builds the execution-only Connector used by an occurrence-owned UI
    /// resource. Source membership was accepted by `UiResourceOwner`; the
    /// derived adapter must not acquire a hidden second lease.
    fn connect_derived(
        &mut self,
        port: &Arc<Mutex<PortRecord>>,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        self.connect_with_membership(port, source, funnel, false)
    }

    fn connect_with_membership(
        &mut self,
        port: &Arc<Mutex<PortRecord>>,
        source: &HostContentSource,
        funnel: HostContentFunnel,
        membership_owned: bool,
    ) -> Result<HostContentConnector> {
        let (port_id, port_family, port_live) = {
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (port.id, port.family, port.lifecycle == PortLifecycle::Live)
        };
        if !port_live {
            return Err(anyhow!("PORT_DISPOSED: ContentPort is disposed"));
        }
        if self
            .ports
            .get(&port_id)
            .is_none_or(|candidate| !Arc::ptr_eq(candidate, port))
        {
            return Err(anyhow!(
                "STALE_HANDLE: ContentPort is not owned by this host"
            ));
        }
        if !source.same_environment(&self.source_registry) {
            return Err(anyhow!(
                "WRONG_ENVIRONMENT: Source belongs to a different environment"
            ));
        }
        if !source.is_live() {
            return Err(anyhow!("SOURCE_DISPOSED: Source is disposed"));
        }
        if !source.retention_compatible(funnel.kind)? {
            return Err(anyhow!(
                "RETENTION_INCOMPATIBLE: Markdown requires an untruncated Source from its logical start"
            ));
        }
        if funnel.family != port_family || funnel.family != source.family() {
            return Err(anyhow!(
                "CONTENT_FAMILY_MISMATCH: ContentPort and Source/Funnel families differ"
            ));
        }
        if membership_owned {
            source.acquire_connector()?;
        }
        self.next_connector_id = match self.next_connector_id.checked_add(1) {
            Some(id) => id,
            None => {
                if membership_owned {
                    source
                        .release_connector()
                        .expect("Connector rollback must release Source membership");
                }
                return Err(anyhow!("Connector identity exhausted"));
            }
        };
        self.next_generation = match self.next_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                if membership_owned {
                    source
                        .release_connector()
                        .expect("Connector rollback must release Source membership");
                }
                return Err(anyhow!("Connector generation exhausted"));
            }
        };
        let record = Arc::new(Mutex::new(ConnectorRecord {
            id: self.next_connector_id,
            generation: self.next_generation,
            lifecycle: ConnectorLifecycle::Live,
            port: Arc::downgrade(port),
            port_id,
            source: source.clone(),
            funnel,
            requested: false,
            visible: false,
            subscribed: false,
            membership_released: !membership_owned,
            membership_owned,
            cleanup_error: None,
            phase: "idle",
            error: None,
            failed_source_revision: None,
            activation_failure: None,
            projection_cache: VecDeque::new(),
            prefix_proof_cache: VecDeque::new(),
            committed_projection: None,
            candidate_projection: None,
            projected_source_revision: None,
            projection_failure_key: None,
            control_revision: 0,
            delivery_revision: 0,
            candidate_delivery_frontier: StreamOffset::ZERO,
            committed_delivery_frontier: StreamOffset::ZERO,
            execution: None,
        }));
        self.connectors
            .insert(self.next_connector_id, Arc::clone(&record));
        port.lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .connector_ids
            .insert(self.next_connector_id);
        Ok(HostContentConnector {
            id: self.next_connector_id,
            generation: self.next_generation,
            source_id: source.id(),
            record,
            host: Weak::new(),
        })
    }

    pub(crate) fn validate_targets(&self, targets: &[u64]) -> Result<()> {
        let mut seen = HashSet::with_capacity(targets.len());
        for id in targets {
            if !seen.insert(*id) {
                return Err(anyhow!(
                    "DUPLICATE_CONTENT_PORT_ATTACHMENT: ContentPort {id} occurs more than once"
                ));
            }
            let Some(port) = self.ports.get(id) else {
                return Err(anyhow!(
                    "STALE_HANDLE: ContentPort {id} is not owned by this host"
                ));
            };
            let port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            if port.lifecycle != PortLifecycle::Live {
                return Err(anyhow!("PORT_DISPOSED: ContentPort {id} is disposed"));
            }
        }
        Ok(())
    }

    pub(crate) fn set_desired(&mut self, targets: &[u64]) -> Result<()> {
        self.validate_targets(targets)?;
        let target_set = targets.iter().copied().collect::<HashSet<_>>();
        let port_ids = self.ports.keys().copied().collect::<Vec<_>>();
        crate::perf::add(
            crate::perf::Counter::ContentRegistryPortScans,
            port_ids.len() as u64,
        );
        let changes = port_ids
            .into_iter()
            .map(|port_id| (port_id, target_set.contains(&port_id)))
            .collect::<Vec<_>>();
        self.set_desired_sparse(&changes)
    }

    /// Updates only the ContentPorts whose occurrence membership changed.
    /// Unlike the legacy full-target operation this does not walk every live
    /// derived port, so a sparse occurrence commit cannot turn into a registry
    /// scan merely to preserve unchanged bindings.
    pub(crate) fn set_desired_sparse(&mut self, changes: &[(u64, bool)]) -> Result<()> {
        let mut seen = HashSet::with_capacity(changes.len());
        for (port_id, _) in changes {
            if !seen.insert(*port_id) {
                return Err(anyhow!(
                    "DUPLICATE_CONTENT_PORT_ATTACHMENT: ContentPort {port_id} occurs more than once"
                ));
            }
            let Some(port) = self.ports.get(port_id) else {
                return Err(anyhow!(
                    "STALE_HANDLE: ContentPort {port_id} is not owned by this host"
                ));
            };
            if port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .lifecycle
                != PortLifecycle::Live
            {
                return Err(anyhow!("PORT_DISPOSED: ContentPort {port_id} is disposed"));
            }
        }
        for (port_id, desired_mounted) in changes {
            let Some(port) = self.ports.get(port_id).cloned() else {
                continue;
            };
            {
                let mut port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                let was_mounted = port_state.desired_mounted;
                port_state.desired_mounted = *desired_mounted;
                let desired_connector = port_state.desired_connector;
                let host = port_state.host.clone();
                drop(port_state);
                if was_mounted != *desired_mounted {
                    self.mark_binding_change(*port_id);
                    self.touch_port(*port_id);
                }
                if let Some(connector_id) = desired_connector {
                    self.refresh_requested_phase(
                        connector_id,
                        *desired_mounted,
                        !was_mounted && *desired_mounted,
                    )?;
                    if *desired_mounted {
                        self.ensure_requested_subscription(connector_id, &host)?;
                    } else {
                        self.unsubscribe_requested_if_not_visible(connector_id)?;
                        let visible = self
                            .connectors
                            .get(&connector_id)
                            .and_then(|connector| connector.lock().ok())
                            .is_some_and(|state| state.visible);
                        if !visible {
                            self.active_deadlines.remove(&connector_id);
                            self.active_connectors.remove(&connector_id);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn begin_projection_candidate(&mut self) {
        self.drain_projection_results()
            .expect("content executor result handling must remain valid");
        self.candidate_capture_active = true;
        self.candidate_commit_prepared = false;
        self.candidate_binding_changes.clear();
        self.candidate_binding_changes
            .extend(self.pending_binding_changes.iter().copied());
        self.candidate_binding_revisions.clear();
        for port_id in &self.candidate_binding_changes {
            if let Some(revision) = self.pending_binding_revisions.get(port_id) {
                self.candidate_binding_revisions.insert(*port_id, *revision);
            }
        }
        self.candidate_source_snapshots.borrow_mut().clear();
        if self.preserve_captures_for_async {
            self.preserve_captures_for_async = false;
        } else {
            self.candidate_content_captures.clear();
        }
        self.candidate_touched_connectors.extend(
            self.pending_source_cleanups
                .iter()
                .map(|cleanup| cleanup.connector_id),
        );
        self.clear_candidate_projections();
    }

    /// Advances Connector-local delivery clocks without parsing or touching
    /// Source storage. A progressed smoother invalidates only its derived
    /// projection; the host frame commits the new visible frontier later.
    pub(crate) fn advance(&mut self, now: Instant) -> Result<Vec<ContentDirty>> {
        self.drain_projection_results()?;
        self.authoritative_clock = Some(now);
        if self.active_deadlines.is_empty() {
            let mut active_candidates = std::mem::take(&mut self.active_sync_scratch);
            active_candidates.clear();
            active_candidates.extend(self.active_connectors.iter().copied());
            for id in active_candidates.drain(..) {
                self.sync_connector_deadline(id, Some(now))?;
            }
            self.active_sync_scratch = active_candidates;
        }
        if self.active_deadlines.is_empty() {
            return Ok(Vec::new());
        }
        let mut due_ids = std::mem::take(&mut self.due_connector_scratch);
        due_ids.clear();
        due_ids.extend(
            self.active_deadlines
                .iter()
                .filter_map(|(&id, &deadline)| (deadline <= now).then_some(id)),
        );
        crate::perf::add(
            crate::perf::Counter::ContentDueConnectors,
            due_ids.len() as u64,
        );
        if due_ids.is_empty() {
            self.due_connector_scratch = due_ids;
            return Ok(Vec::new());
        }
        let mut changed = Vec::new();
        for connector_id in due_ids.iter().copied() {
            let Some(connector) = self.connectors.get(&connector_id).cloned() else {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
                continue;
            };
            let mut state = match connector.lock() {
                Ok(state) => state,
                Err(_) => {
                    // A poisoned Connector must not remain in the due index:
                    // otherwise every native tick retries the same failed
                    // lock forever without producing a report. Remove its
                    // clock membership before returning the typed scheduler
                    // failure; an explicit readiness/control signal can
                    // re-admit it after the owner repairs the record.
                    self.active_deadlines.remove(&connector_id);
                    self.active_connectors.remove(&connector_id);
                    return Err(anyhow!(
                        "Connector lock is poisoned during delivery advance"
                    ));
                }
            };
            if !state.visible && !state.requested {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
                continue;
            }
            let progressed = state
                .execution
                .as_mut()
                .and_then(|execution| execution.delivery.as_mut())
                .is_some_and(|delivery| delivery.advance(now));
            let next_dl = state
                .execution
                .as_ref()
                .and_then(|execution| execution.delivery.as_ref())
                .and_then(|delivery| {
                    if !delivery.smoother.has_pending_work() {
                        None
                    } else {
                        delivery.smoother.next_wakeup().or(Some(now))
                    }
                });
            if let Some(dl) = next_dl {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, dl);
            } else {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
            }
            if !progressed {
                continue;
            }
            if let Some(delivery) = state.execution.as_ref().and_then(|e| e.delivery.as_ref()) {
                state.candidate_delivery_frontier = delivery.candidate_frontier;
            }
            state.delivery_revision = state
                .delivery_revision
                .checked_add(1)
                .expect("Connector delivery revision exhausted");
            state.candidate_projection = None;
            // Delivery ticks do not clear the width-product cache.
            let port_id = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id;
            changed.push(ContentDirty::new(
                port_id,
                Some(connector_id),
                ContentDirtyReason::DeliveryVisibility,
            ));
        }
        self.due_connector_scratch = due_ids;
        Ok(changed)
    }

    pub(crate) fn next_wakeup(&self) -> Option<Instant> {
        self.active_deadlines.values().copied().min()
    }

    pub(crate) fn sync_connector_deadline(
        &mut self,
        connector_id: u64,
        now: Option<Instant>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle == ConnectorLifecycle::Disposed || (!state.visible && !state.requested) {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_mounted = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned during deadline sync"))?
            .desired_mounted;
        if !state.visible && (!state.requested || !port_mounted) {
            // A requested Connector may remain a cold binding while its Port
            // is unmounted, but it must not index a delivery clock or perform
            // source/smoothing work until the destination is resident again.
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        if state.funnel.smooth_config().is_none() {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            return Ok(());
        }
        if state.execution.is_none() && state.funnel.smooth_config().is_some() {
            state.execution = Some(ConnectorExecution::new(&state.funnel));
        }
        let snapshot = self.source_snapshot_for(&state.source)?;
        if let Some(execution) = state.execution.as_mut()
            && let Some(delivery) = execution.delivery.as_mut()
        {
            let clock_now = now
                .or(self.authoritative_clock)
                .unwrap_or_else(Instant::now);
            delivery.smoother.ensure_clock(clock_now);
            delivery.accept_input(&snapshot)?;
            delivery.smoother.ensure_clock(clock_now);
            if !delivery.smoother.has_pending_work() {
                self.active_deadlines.remove(&connector_id);
                self.active_connectors.remove(&connector_id);
            } else if let Some(dl) = delivery.smoother.next_wakeup() {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, dl);
            } else if let Some(now) = now.or(self.authoritative_clock) {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, now);
            } else {
                self.active_connectors.insert(connector_id);
                self.active_deadlines.insert(connector_id, Instant::now());
            }
        } else {
            self.active_deadlines.remove(&connector_id);
            if state.funnel.smooth_config().is_none() || (!state.visible && !state.requested) {
                self.active_connectors.remove(&connector_id);
            }
        }
        Ok(())
    }

    pub(crate) fn connector_delivery_frontier(&self, id: u64) -> Result<StreamOffset> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.committed_delivery_frontier)
    }

    pub(crate) fn connector_candidate_delivery_frontier(&self, id: u64) -> Result<StreamOffset> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.candidate_delivery_frontier)
    }

    fn connector_projection_key(&self, connector_id: u64, width: u16) -> Result<TextProjectionKey> {
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        let snapshot = self.source_snapshot_for(&state.source)?;
        Self::projection_key_for_state(
            &state,
            port_id,
            width,
            self.theme_revision,
            &self.history_adapter,
            &snapshot,
        )
    }

    fn projection_key_for_state(
        state: &ConnectorRecord,
        port_id: u64,
        width: u16,
        theme_revision: u64,
        history_adapter: &HistoryTerminalAdapter,
        snapshot: &HostContentSourceSnapshot,
    ) -> Result<TextProjectionKey> {
        Ok(TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            head_partial: snapshot.head_partial,
            width,
            wrap: state.funnel.wrap,
            funnel_kind: state.funnel.kind,
            delivery_revision: state.delivery_revision,
            theme_revision,
            needs_finalized_prefix: history_adapter.unit_id(port_id).is_some(),
            needs_physical_rows: history_adapter.unit_id(port_id).is_some()
                || state.funnel.smooth_config().is_some(),
        })
    }

    fn connector_projection_key_for_snapshot(
        &self,
        connector_id: u64,
        width: u16,
        snapshot: &HostContentSourceSnapshot,
    ) -> Result<TextProjectionKey> {
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        Self::projection_key_for_state(
            &state,
            port_id,
            width,
            self.theme_revision,
            &self.history_adapter,
            snapshot,
        )
    }

    fn source_snapshot_for(&self, source: &HostContentSource) -> Result<HostContentSourceSnapshot> {
        if !self.candidate_capture_active {
            return source.snapshot();
        }
        let identity = super::ui_resources::SourceIdentity::from_source(source);
        if let Some(snapshot) = self
            .candidate_source_snapshots
            .borrow()
            .get(&identity)
            .cloned()
        {
            return Ok(snapshot);
        }
        let snapshot = source.snapshot()?;
        self.candidate_source_snapshots
            .borrow_mut()
            .insert(identity, snapshot.clone());
        Ok(snapshot)
    }

    fn connector_revision(&self, connector_id: u64, offered_width: u16) -> u64 {
        let Ok(key) = self.connector_projection_key(connector_id, offered_width) else {
            return 0;
        };
        let Some(connector) = self.connectors.get(&connector_id) else {
            return 0;
        };
        let Ok(state) = connector.lock() else {
            return 0;
        };
        let port_mounted = state
            .port
            .upgrade()
            .and_then(|port| port.lock().ok().map(|port| port.desired_mounted))
            .unwrap_or(false);
        let projection_ready = state
            .candidate_projection
            .as_ref()
            .is_some_and(|projection| projection.key == key)
            || state
                .committed_projection
                .as_ref()
                .is_some_and(|projection| projection.key == key)
            || Self::cached_projection(&state, &key).is_some();

        // The layout cache is keyed before ContentProvider::measure can
        // prepare a projection. Include Connector identity and readiness so a
        // cache entry from another Connector, an unmounted occurrence, or an
        // evicted width projection cannot commit a measured node that the
        // painter has no corresponding derived rows for.
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        connector_id.hash(&mut hasher);
        key.hash(&mut hasher);
        projection_ready.hash(&mut hasher);
        state.requested.hash(&mut hasher);
        state.visible.hash(&mut hasher);
        state.error.is_some().hash(&mut hasher);
        port_mounted.hash(&mut hasher);
        hasher.finish()
    }

    fn selected_connector_id(&self, port_id: u64) -> Option<u64> {
        if let Some(selection) = self.candidate_selections.get(&port_id) {
            return *selection;
        }
        let (desired, visible, desired_mounted) = {
            let port = self.ports.get(&port_id)?.lock().ok()?;
            (
                port.desired_connector,
                port.visible_connector,
                port.desired_mounted,
            )
        };
        if desired_mounted {
            if let Some(desired) = desired {
                let failed = self
                    .connectors
                    .get(&desired)
                    .and_then(|connector| connector.lock().ok())
                    .is_some_and(|state| state.error.is_some() && !state.visible);
                if !failed {
                    return Some(desired);
                }
            }
            visible
        } else {
            None
        }
    }

    fn cached_projection(
        state: &ConnectorRecord,
        key: &TextProjectionKey,
    ) -> Option<Arc<HostContentProjection>> {
        state
            .projection_cache
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, projection)| Arc::clone(projection))
    }

    /// Selects an exact product first, then an append-compatible older
    /// product. Replacement, truncation, generation, width, funnel, theme,
    /// and delivery changes cannot reuse the older product.
    fn projection_for_key(
        state: &ConnectorRecord,
        key: &TextProjectionKey,
        snapshot: &HostContentSourceSnapshot,
    ) -> Option<Arc<HostContentProjection>> {
        let matches = |projection: &Arc<HostContentProjection>| {
            projection.key == *key
                || (projection.key.source_id == key.source_id
                    && projection.key.source_generation == key.source_generation
                    && projection.key.content_generation == key.content_generation
                    && projection.key.source_base == key.source_base
                    && projection.key.head_partial == key.head_partial
                    && projection.key.width == key.width
                    && projection.key.wrap == key.wrap
                    && projection.key.funnel_kind == key.funnel_kind
                    && projection.key.delivery_revision == key.delivery_revision
                    && projection.key.theme_revision == key.theme_revision
                    && projection.key.needs_finalized_prefix == key.needs_finalized_prefix
                    && projection.key.needs_physical_rows == key.needs_physical_rows
                    && projection.source_snapshot.source_end <= snapshot.source_end)
        };
        let mut projections = state
            .candidate_projection
            .iter()
            .chain(state.committed_projection.iter())
            .chain(
                state
                    .projection_cache
                    .iter()
                    .map(|(_, projection)| projection),
            );
        projections
            .clone()
            .find(|projection| projection.key == *key)
            .or_else(|| projections.find(|projection| matches(projection)))
            .cloned()
    }

    fn prepare_connector_projection(
        &mut self,
        connector_id: u64,
        offered_width: u16,
    ) -> Result<ContentMeasurement> {
        self.prepare_connector_projection_async(connector_id, offered_width, None)
    }

    fn admit_connector_projection(
        &mut self,
        request: ContentProjectionAdmission<'_>,
    ) -> Result<()> {
        let ContentProjectionAdmission {
            connector,
            connector_id,
            key,
            snapshot,
            funnel,
            offered_width,
            delivery_revision,
            needs_finalized_prefix,
            task_bytes,
            cancelled,
        } = request;
        let waiter = if self.deferred_projection_waiter.is_none() {
            Some(self.completion_wake())
        } else {
            None
        };
        let (permit, waiter_id) = match self.executor.reserve_projection(task_bytes, waiter) {
            Ok(admission) => admission,
            Err(error) => {
                self.pending_content_projections.remove(&connector_id);
                self.release_deferred_projection_waiter_if_idle();
                return Err(anyhow::Error::new(ContentProjectionFailure {
                    kind: Self::projection_admission_failure(&error),
                    diagnostic: error.to_string(),
                }));
            }
        };
        let Some(permit) = permit else {
            if let Some(waiter_id) = waiter_id {
                self.deferred_projection_waiter = Some(waiter_id);
            }
            let pending = self
                .pending_content_projections
                .get_mut(&connector_id)
                .expect("saturated projection must retain its pending marker");
            pending.key = key;
            return Ok(());
        };
        {
            let (result_sender, result) = sync_channel(1);
            let (execution, prefix_proof_cache) = match connector.lock() {
                Ok(mut state) => (
                    state
                        .execution
                        .take()
                        .unwrap_or_else(|| ConnectorExecution::new(&funnel)),
                    std::mem::take(&mut state.prefix_proof_cache),
                ),
                Err(_) => {
                    self.pending_content_projections.remove(&connector_id);
                    self.release_deferred_projection_waiter_if_idle();
                    drop(permit);
                    return Err(anyhow!("Connector lock is poisoned"));
                }
            };
            let task = ContentProjectionTask {
                bytes: task_bytes,
                connector_id,
                key,
                result: result_sender,
                snapshot: snapshot.clone(),
                funnel,
                offered_width,
                needs_finalized_prefix,
                theme: Arc::clone(&self.theme),
                theme_revision: self.theme_revision,
                delivery_revision,
                execution,
                prefix_proof_cache,
                wake: self.completion_wake(),
                cancelled,
            };
            if let Err(error) = self.executor.submit_projection(permit, task) {
                self.pending_content_projections.remove(&connector_id);
                self.release_deferred_projection_waiter_if_idle();
                return Err(anyhow::Error::new(ContentProjectionFailure {
                    kind: Self::projection_admission_failure(&error),
                    diagnostic: error.to_string(),
                }));
            }
            let pending = self
                .pending_content_projections
                .get_mut(&connector_id)
                .expect("admitted projection must retain its pending marker");
            pending.key = key;
            pending.result = Some(result);
            self.release_deferred_projection_waiter_if_idle();
            Ok(())
        }
    }

    /// Schedules a missing width realization on the shared content executor.
    /// The method intentionally returns a ready compatible product when one
    /// exists, otherwise a loading measurement. It never waits for the task;
    /// the environment wake turns the completion into a later candidate.
    fn prepare_connector_projection_async(
        &mut self,
        connector_id: u64,
        offered_width: u16,
        captured_source: Option<&HostContentSourceSnapshot>,
    ) -> Result<ContentMeasurement> {
        self.touch_connector(connector_id);
        let connector =
            self.connectors.get(&connector_id).cloned().ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: Connector {connector_id} disappeared")
            })?;
        let (source, funnel, delivery_revision, needs_finalized_prefix, needs_physical_rows) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live || (!state.requested && !state.visible) {
                return Ok(ContentMeasurement::default());
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let port_id = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id;
            (
                state.source.clone(),
                state.funnel,
                state.delivery_revision,
                self.history_adapter.unit_id(port_id).is_some(),
                self.history_adapter.unit_id(port_id).is_some()
                    || state.funnel.smooth_config().is_some(),
            )
        };
        let snapshot = captured_source
            .cloned()
            .map_or_else(|| self.source_snapshot_for(&source), Ok)?;
        if funnel.kind == TextFunnelKind::Markdown && snapshot.source_base != 0 {
            return Err(anyhow::Error::new(ContentProjectionFailure {
                kind: ContentProjectionFailureKind::RetentionIncompatible,
                diagnostic: "RETENTION_INCOMPATIBLE: Markdown requires an untruncated Source from its logical start"
                    .to_owned(),
            }));
        }
        let key = TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            head_partial: snapshot.head_partial,
            width: offered_width,
            wrap: funnel.wrap,
            funnel_kind: funnel.kind,
            delivery_revision,
            theme_revision: self.theme_revision,
            needs_finalized_prefix,
            needs_physical_rows,
        };
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.error.is_some() && state.projection_failure_key == Some(key) {
                return Err(anyhow!(
                    "PROJECTION_RETRY_BLOCKED: Connector projection is already failed for this input"
                ));
            }
            if let Some(projection) = Self::projection_for_key(&state, &key, &snapshot)
                && projection.key == key
            {
                state.candidate_projection = Some(Arc::clone(&projection));
                state.error = None;
                state.failed_source_revision = None;
                state.projection_failure_key = None;
                return Ok(projection.measurement(connector_id));
            }
        }
        // One in-flight task or admission marker per Connector is the
        // coalescing boundary. Saturation retains only this marker; Source
        // snapshots and parser execution are captured after a later atomic
        // executor reservation succeeds.
        if let Some(pending) = self.pending_content_projections.get(&connector_id) {
            if pending.result.is_some() {
                let state = connector
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                return Ok(Self::projection_for_key(&state, &key, &snapshot)
                    .map_or_else(ContentMeasurement::default, |projection| {
                        projection.measurement(connector_id)
                    }));
            }
        } else {
            let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.pending_content_projections.insert(
                connector_id,
                PendingContentProjection {
                    key,
                    result: None,
                    cancelled: Arc::clone(&cancelled),
                },
            );
        }
        let cancelled = self
            .pending_content_projections
            .get(&connector_id)
            .map(|pending| Arc::clone(&pending.cancelled))
            .expect("projection admission marker must retain cancellation state");
        let task_bytes = usize::try_from(snapshot.retained_bytes()).unwrap_or(usize::MAX);
        self.admit_connector_projection(ContentProjectionAdmission {
            connector: &connector,
            connector_id,
            key,
            snapshot: &snapshot,
            funnel,
            offered_width,
            delivery_revision,
            needs_finalized_prefix,
            task_bytes,
            cancelled,
        })?;
        if self
            .pending_content_projections
            .get(&connector_id)
            .is_some_and(|pending| pending.result.is_none())
        {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            return Ok(Self::projection_for_key(&state, &key, &snapshot)
                .map_or_else(ContentMeasurement::default, |projection| {
                    projection.measurement(connector_id)
                }));
        }
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(Self::projection_for_key(&state, &key, &snapshot)
            .map_or_else(ContentMeasurement::default, |projection| {
                projection.measurement(connector_id)
            }))
    }

    fn projection_measurement(
        &self,
        connector_id: u64,
        offered_width: u16,
    ) -> Option<ContentMeasurement> {
        let key = self
            .connector_projection_key(connector_id, offered_width)
            .ok()?;
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let snapshot = self.source_snapshot_for(&connector.source).ok()?;
        Self::projection_for_key(&connector, &key, &snapshot)
            .map(|projection| projection.measurement(connector_id))
    }

    fn loading_measurement(&self, connector_id: u64, offered_width: u16) -> ContentMeasurement {
        let Some(connector) = self.connectors.get(&connector_id) else {
            return ContentMeasurement::default();
        };
        let Ok(source) = connector.lock().map(|state| state.source.clone()) else {
            return ContentMeasurement::default();
        };
        let Ok(snapshot) = self.source_snapshot_for(&source) else {
            return ContentMeasurement::default();
        };
        let Ok(key) = self.connector_projection_key(connector_id, offered_width) else {
            return ContentMeasurement::default();
        };
        let size = Size::new(0, 1);
        ContentMeasurement {
            intrinsic_size: size,
            physically_complete: false,
            projection_revision: key.revision(),
            metric_revision: key.metric_revision(size, false),
            paint_revision: key.revision(),
            connector_id: Some(connector_id),
            projection_identity: 0,
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            sealed: snapshot.sealed,
            head_partial: snapshot.head_partial,
        }
    }

    fn projection_failure_is_recorded(&self, connector_id: u64, key: TextProjectionKey) -> bool {
        self.connectors
            .get(&connector_id)
            .and_then(|connector| connector.lock().ok())
            .is_some_and(|state| state.error.is_some() && state.projection_failure_key == Some(key))
    }

    fn record_projection_failure(
        &mut self,
        connector_id: u64,
        key: TextProjectionKey,
        error: &anyhow::Error,
    ) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let mut state = connector
            .lock()
            .expect("Connector lock must remain usable while recording a failure");
        let code = error
            .downcast_ref::<ContentProjectionFailure>()
            .map_or(ContentProjectionFailureKind::Projection.code(), |failure| {
                failure.kind.code()
            });
        state.error = Some(ContentConnectorError {
            code: code.to_owned(),
            diagnostic: error.to_string(),
        });
        state.failed_source_revision = Some(key.source_revision);
        state.projection_failure_key = Some(key);
        state.phase = if state.visible { "active" } else { "failed" };
    }

    fn record_connector_operating_failure(
        &mut self,
        connector_id: u64,
        error: anyhow::Error,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: Connector {connector_id} disappeared while recording an operating failure"
            ));
        };
        let mut state = connector.lock().map_err(|_| {
            anyhow!("Connector lock is poisoned while recording an operating failure")
        })?;
        state.error = Some(ContentConnectorError {
            code: "CONTENT_OPERATING_FAILED".to_owned(),
            diagnostic: error.to_string(),
        });
        state.failed_source_revision = None;
        state.projection_failure_key = None;
        state.phase = if state.visible { "active" } else { "failed" };
        Ok(())
    }

    fn refine_fit_measurement(
        &mut self,
        connector_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::ContentWidthRule,
        measurement: ContentMeasurement,
    ) -> ContentMeasurement {
        if width_rule != crate::presentation::ContentWidthRule::Fit
            || measurement.intrinsic_size.width == 0
            || measurement.intrinsic_size.width >= offered_width
        {
            return measurement;
        }
        // The first probe already compiled the semantic product at the
        // offered width. When its natural width is smaller, no line wrapped
        // in that product, so re-keying the prepared ticket is equivalent to
        // recompiling at the natural width. Keep one canonical paint product
        // and avoid a second full semantic/layout/paint compilation for the
        // common width-fit ContentHost path.
        if let Some(measurement) = self.rekey_fit_projection(
            connector_id,
            offered_width,
            measurement.intrinsic_size.width,
        ) {
            return measurement;
        }
        match self.prepare_connector_projection(connector_id, measurement.intrinsic_size.width) {
            Ok(measurement) => measurement,
            Err(error) => {
                let _ = self.record_connector_operating_failure(connector_id, error);
                measurement
            }
        }
    }

    fn rekey_fit_projection(
        &mut self,
        connector_id: u64,
        offered_width: u16,
        intrinsic_width: u16,
    ) -> Option<ContentMeasurement> {
        let connector = self.connectors.get(&connector_id).cloned()?;
        let mut state = connector.lock().ok()?;
        let projection = state.candidate_projection.as_ref()?.clone();
        if projection.key.width != offered_width
            || projection.intrinsic_size.width != intrinsic_width
            || intrinsic_width == 0
        {
            return None;
        }
        let mut key = projection.key;
        key.width = intrinsic_width;
        if key == projection.key {
            return Some(projection.measurement(connector_id));
        }
        let mut rekeyed = (*projection).clone();
        rekeyed.identity = next_content_projection_id();
        rekeyed.key = key;
        let rekeyed = Arc::new(rekeyed);
        state
            .projection_cache
            .retain(|(candidate, _)| candidate != &key);
        state
            .projection_cache
            .push_front((key, Arc::clone(&rekeyed)));
        while state.projection_cache.len() > CONTENT_CACHE_CAPACITY {
            state.projection_cache.pop_back();
        }
        state.candidate_projection = Some(Arc::clone(&rekeyed));
        Some(rekeyed.measurement(connector_id))
    }

    fn adjust_history_measurement(
        &self,
        port_id: u64,
        mut measurement: ContentMeasurement,
    ) -> ContentMeasurement {
        let committed_rows = self.history_adapter.committed_content_rows(port_id);
        if committed_rows == 0 {
            return measurement;
        }
        measurement.intrinsic_size.height = measurement
            .intrinsic_size
            .height
            .saturating_sub(u16::try_from(committed_rows).unwrap_or(u16::MAX));
        measurement
    }

    fn history_measurement_adjustment(
        &self,
        port_id: u64,
        offered_width: u16,
        measurement: &ContentMeasurement,
        product: Option<&HostContentProjection>,
    ) -> Option<HistoryMeasurementAdjustment> {
        let removed_rows = self.history_adapter.committed_content_rows(port_id);
        let product = product?;
        (removed_rows > 0 && measurement.projection_identity == product.identity).then_some(
            HistoryMeasurementAdjustment {
                projection_identity: product.identity,
                offered_width,
                removed_rows,
            },
        )
    }

    fn measure_content(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::ContentWidthRule,
    ) -> ContentMeasurement {
        self.touch_port(port_id);
        let Some(port) = self.ports.get(&port_id).cloned() else {
            return ContentMeasurement::default();
        };
        let (desired, visible, desired_mounted) = {
            let Ok(port) = port.lock() else {
                return ContentMeasurement::default();
            };
            (
                port.desired_connector,
                port.visible_connector,
                port.desired_mounted,
            )
        };
        let measurement = if !desired_mounted {
            self.candidate_selections.insert(port_id, None);
            ContentMeasurement::default()
        } else {
            let Some(connector_id) = desired else {
                self.candidate_selections.insert(port_id, None);
                return ContentMeasurement::default();
            };

            // Keep the native/unit failure fixture on the same candidate-rollback
            // boundary as real projection failures.
            let activation_failed =
                match self.prepare_activation_candidate(connector_id, offered_width) {
                    Ok(failed) => failed,
                    Err(error) => {
                        if self
                            .record_connector_operating_failure(connector_id, error)
                            .is_err()
                        {
                            return ContentMeasurement::default();
                        }
                        false
                    }
                };
            if activation_failed {
                let rollback = visible.and_then(|id| {
                    self.prepare_connector_projection(id, offered_width)
                        .ok()
                        .map(|measurement| {
                            self.refine_fit_measurement(id, offered_width, width_rule, measurement)
                        })
                });
                self.candidate_selections.insert(port_id, visible);
                rollback.unwrap_or_default()
            } else {
                match self.prepare_connector_projection(connector_id, offered_width) {
                    Ok(measurement) => {
                        if measurement.projection_identity == 0
                            && self.pending_content_projections.contains_key(&connector_id)
                        {
                            let rollback = visible.and_then(|id| {
                                self.prepare_connector_projection(id, offered_width)
                                    .ok()
                                    .or_else(|| self.projection_measurement(id, offered_width))
                            });
                            if let Some(rollback) = rollback {
                                self.candidate_selections.insert(port_id, visible);
                                return self.adjust_history_measurement(port_id, rollback);
                            }
                            self.candidate_selections
                                .insert(port_id, Some(connector_id));
                            return self.adjust_history_measurement(
                                port_id,
                                self.loading_measurement(connector_id, offered_width),
                            );
                        }
                        self.candidate_selections
                            .insert(port_id, Some(connector_id));
                        self.refine_fit_measurement(
                            connector_id,
                            offered_width,
                            width_rule,
                            measurement,
                        )
                    }
                    Err(error) => {
                        if !error.downcast_ref::<ContentProjectionPending>().is_some()
                            && let Ok(key) =
                                self.connector_projection_key(connector_id, offered_width)
                            && !self.projection_failure_is_recorded(connector_id, key)
                        {
                            self.record_projection_failure(connector_id, key, &error);
                        }
                        let rollback = visible.and_then(|id| {
                            self.prepare_connector_projection(id, offered_width)
                                .ok()
                                .or_else(|| self.projection_measurement(id, offered_width))
                                .map(|measurement| {
                                    self.refine_fit_measurement(
                                        id,
                                        offered_width,
                                        width_rule,
                                        measurement,
                                    )
                                })
                        });
                        self.candidate_selections.insert(port_id, visible);
                        rollback.unwrap_or_default()
                    }
                }
            }
        };
        self.adjust_history_measurement(port_id, measurement)
    }

    fn paint_window_direct(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (i32, i32),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        // The ticket is the product selected during preparation.  Never
        // substitute the newest same-width projection: a Source append,
        // delivery tick, Connector switch, or theme change may have created
        // another candidate while this frame is still being painted.
        let Some(projection) = self.projection_for_ticket(ticket) else {
            target.physically_complete = false;
            return;
        };
        if !projection.physically_complete {
            target.physically_complete = false;
        }
        let committed_rows = self.history_adapter.committed_content_rows(ticket.port_id);
        let max_visible = projection.visible_row_count.saturating_sub(committed_rows);
        let retained_visible_len = projection.rows.as_ref().map_or(max_visible, |rows| {
            rows.len().saturating_sub(committed_rows).min(max_visible)
        });

        let start_offset = match usize::try_from(window.first_row) {
            Ok(v) => v,
            Err(_) => return,
        };
        if start_offset >= retained_visible_len || window.row_count == 0 {
            return;
        }
        let row_count = usize::try_from(window.row_count).unwrap_or(usize::MAX);
        let end_offset = (start_offset.saturating_add(row_count)).min(retained_visible_len);
        let window_slice: &[PhysicalRow] = if let Some(rows) = projection.rows.as_ref() {
            let available_rows = if committed_rows >= rows.len() {
                &[][..]
            } else {
                &rows[committed_rows..]
            };
            &available_rows[start_offset..end_offset]
        } else {
            let first_row = committed_rows.saturating_add(start_offset);
            let source_row_count = end_offset.saturating_sub(start_offset);
            let result = projection.product.paint_window(
                &projection.theme,
                style,
                target,
                target_origin,
                clip,
                TerminalRowWindow::new(first_row, source_row_count),
            );
            if result.is_err() || !projection.product.physically_complete() {
                target.physically_complete = false;
            }
            return;
        };

        let clip_left = i32::from(clip.x);
        let clip_top = i32::from(clip.y);
        let clip_right = clip_left.saturating_add(i32::from(clip.width));
        let clip_bottom = clip_top.saturating_add(i32::from(clip.height));
        let target_width = i32::from(target.width());
        let target_height = i32::from(target.height());

        for (i, row) in window_slice.iter().enumerate() {
            let target_y = target_origin.1.saturating_add(i as i32);
            if target_y < clip_top
                || target_y >= clip_bottom
                || target_y < 0
                || target_y >= target_height
            {
                continue;
            }

            let projection_row_idx = committed_rows + start_offset + i;
            let max_col = if let Some((cut_row, cut_col)) = projection.cut
                && projection_row_idx == usize::from(cut_row)
            {
                usize::from(cut_col)
            } else if let Some((cut_row, _)) = projection.cut
                && projection_row_idx > usize::from(cut_row)
            {
                0
            } else {
                usize::from(ticket.offered_width)
            };

            if max_col == 0 {
                continue;
            }

            let dest_origin_x = target_origin.0;
            let src_cells = row.cells();
            for glyph in row.glyphs() {
                if !glyph.leader.painted {
                    continue;
                }
                if glyph.start >= max_col {
                    break;
                }
                let dest_start = dest_origin_x.saturating_add(glyph.start as i32);
                let dest_end = dest_start.saturating_add(glyph.width as i32);
                if dest_start < clip_left
                    || dest_end > clip_right
                    || dest_start < 0
                    || dest_end > target_width
                {
                    continue;
                }

                let dest_row = target.row_cells_mut(target_y as u16);
                let backing_background = dest_row
                    .get(dest_start as usize)
                    .and_then(|cell| cell.style.background);
                crate::perf::add(
                    crate::perf::Counter::SurfaceCellsComposited,
                    glyph.width as u64,
                );
                crate::physical::write_glyph_span(
                    dest_row,
                    dest_start as usize,
                    src_cells,
                    glyph.start,
                    glyph.width,
                );

                for col in (dest_start as usize)..(dest_end as usize) {
                    let cell = &mut dest_row[col];
                    if cell.painted {
                        if cell.style.foreground.is_none() {
                            cell.style.foreground = style.foreground;
                        }
                        if cell.style.background.is_none() {
                            cell.style.background = backing_background.or(style.background);
                        }
                        cell.style.bold |= style.bold;
                        cell.style.dim |= style.dim;
                        cell.style.italic |= style.italic;
                        cell.style.underline |= style.underline;
                        cell.style.reversed |= style.reversed;
                        cell.style.strikethrough |= style.strikethrough;
                    }
                }
                if backing_background.is_some() {
                    eprintln!(
                        "content backing {:?} final={:?}",
                        backing_background, dest_row[dest_start as usize].style.background
                    );
                }
            }
            debug_assert!(
                crate::physical::validate_cells(target.row_cells(target_y as u16)).is_ok()
            );
        }
    }

    fn projection_for_ticket(
        &self,
        ticket: PreparedProjectionTicket,
    ) -> Option<Arc<HostContentProjection>> {
        let connector_id = ticket.connector_id?;
        if ticket.projection_identity == 0 {
            return None;
        }
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let matches = |projection: &Arc<HostContentProjection>| {
            projection.identity == ticket.projection_identity
                && projection.key.width == ticket.offered_width
                && projection.key.revision() == ticket.projection_revision
        };
        if connector.candidate_projection.as_ref().is_some_and(matches) {
            return connector.candidate_projection.as_ref().cloned();
        }
        if connector.committed_projection.as_ref().is_some_and(matches) {
            return connector.committed_projection.as_ref().cloned();
        }
        connector
            .projection_cache
            .iter()
            .find(|(_, projection)| matches(projection))
            .map(|(_, projection)| Arc::clone(projection))
    }

    fn connector_projection(
        &self,
        connector_id: u64,
        offered_width: u16,
    ) -> Option<Arc<HostContentProjection>> {
        let key = self
            .connector_projection_key(connector_id, offered_width)
            .ok()?;
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        if let Some(projection) = connector
            .candidate_projection
            .as_ref()
            .filter(|projection| projection.key == key)
        {
            // The candidate owns the immutable Source snapshot captured for
            // this frame. A concurrent Source revision is left for the next
            // host epoch rather than mixing snapshots during paint.
            return Some(Arc::clone(projection));
        }
        if let Some(projection) = Self::cached_projection(&connector, &key) {
            return Some(projection);
        }
        connector
            .committed_projection
            .as_ref()
            .filter(|projection| projection.key == key)
            .cloned()
    }

    fn prepare_connector_commit(
        &self,
        connector_id: u64,
        visible: bool,
    ) -> Result<PreparedContentConnector> {
        let record = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: Connector {connector_id} disappeared during candidate preparation"
                )
            })?;
        let (
            source,
            source_id,
            generation,
            requested,
            subscribed,
            control_revision,
            deadline,
            candidate_projection,
            delivery_frontier,
            delivery_input,
            delivery_revision,
        ) = {
            let state = record
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned during candidate preparation"))?;
            if state.lifecycle == ConnectorLifecycle::Disposed {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: Connector {connector_id} was disposed during candidate preparation"
                ));
            }
            let deadline = state
                .execution
                .as_ref()
                .and_then(|execution| execution.delivery.as_ref())
                .and_then(|delivery| {
                    delivery
                        .smoother
                        .has_pending_work()
                        .then(|| delivery.smoother.next_wakeup())
                        .flatten()
                });
            let delivery_input = state.execution.as_ref().and_then(|execution| {
                execution.delivery.as_ref().map(|delivery| {
                    (
                        delivery.indexed_generation,
                        delivery.indexed_revision,
                        delivery.indexed_sealed,
                    )
                })
            });
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let _port = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (
                state.source.clone(),
                state.source.id(),
                state.generation,
                state.requested,
                state.subscribed,
                state.control_revision,
                deadline,
                state.candidate_projection.clone(),
                state.candidate_delivery_frontier,
                delivery_input,
                state.delivery_revision,
            )
        };
        Ok(PreparedContentConnector {
            id: connector_id,
            record,
            source,
            source_id,
            generation,
            requested,
            subscribed,
            control_revision,
            deadline,
            visible,
            candidate_projection,
            delivery_frontier,
            delivery_input,
            delivery_revision,
        })
    }

    /// Captures only the changed Port/Connector records needed to promote one
    /// prepared content candidate. This is the only place where the live
    /// Port/Connector maps are resolved for the commit. Receipt-time code
    /// consumes the resulting Arc-backed plan and performs no handle lookup,
    /// validity rediscovery, or temporary registry-wide scan.
    pub(crate) fn prepare_content_commit(&mut self) -> Result<PreparedContentCommit> {
        let mut changed_port_ids = self
            .candidate_binding_changes
            .iter()
            .copied()
            .collect::<Vec<_>>();
        changed_port_ids.sort_unstable();
        let changed_port_count = changed_port_ids.len();
        let mut ports = Vec::with_capacity(changed_port_count);
        let mut changed_ports = Vec::with_capacity(changed_port_count);
        for port_id in changed_port_ids {
            let port = self.ports.get(&port_id).cloned().ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} disappeared during candidate preparation"
                )
            })?;
            let (mounted, desired_connector, visible_connector) = {
                let state = port.lock().map_err(|_| {
                    anyhow!("ContentPort lock is poisoned during candidate preparation")
                })?;
                (
                    state.desired_mounted,
                    state.desired_connector,
                    state.visible_connector,
                )
            };
            let next_connector = if mounted {
                let selected = self
                    .candidate_selections
                    .get(&port_id)
                    .copied()
                    .unwrap_or(desired_connector);
                if let Some(selected) = selected
                    && self.connector_is_candidate_ready(selected)?
                {
                    Some(selected)
                } else {
                    visible_connector
                }
            } else {
                None
            };
            changed_ports.push((port_id, mounted, next_connector));
        }
        let mut connector_ids = HashSet::with_capacity(changed_port_count * 2);
        let mut add_port = |port_id: u64, mounted: bool, next_id: Option<u64>| -> Result<()> {
            let record = self.ports.get(&port_id).cloned().ok_or_else(|| {
                anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} disappeared during candidate preparation"
                )
            })?;
            let (old_id, port_live, ui_key, ui_retire_after_receipt) = {
                let state = record.lock().map_err(|_| {
                    anyhow!("ContentPort lock is poisoned during candidate preparation")
                })?;
                (
                    state.visible_connector,
                    state.lifecycle == PortLifecycle::Live,
                    state.ui_key,
                    state.ui_retire_after_receipt,
                )
            };
            if !port_live {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: ContentPort {port_id} became disposed during candidate preparation"
                ));
            }
            if !mounted && next_id.is_some() {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: unmounted ContentPort {port_id} has a visible candidate"
                ));
            }
            let old_control_revision = if let Some(id) = old_id {
                let connector = self.connectors.get(&id).cloned().ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: visible Connector {id} disappeared during candidate preparation"
                        )
                    })?;
                let control_revision = connector
                    .lock()
                    .map_err(|_| {
                        anyhow!("Connector lock is poisoned during candidate preparation")
                    })?
                    .control_revision;
                Some(control_revision)
            } else {
                None
            };
            if let Some(id) = old_id {
                connector_ids.insert(id);
            }
            if let Some(id) = next_id {
                let connector = self.connectors.get(&id).cloned().ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: candidate Connector {id} disappeared during preparation"
                        )
                    })?;
                let belongs = connector
                    .lock()
                    .map_err(|_| {
                        anyhow!("Connector lock is poisoned during candidate preparation")
                    })?
                    .port
                    .upgrade()
                    .is_some_and(|owner| Arc::ptr_eq(&owner, &record));
                if !belongs {
                    return Err(anyhow!(
                        "INTERNAL_INVARIANT: Connector {id} does not belong to ContentPort {port_id}"
                    ));
                }
                if Some(id) != old_id
                    && !self.connector_is_candidate_ready(id)?
                    && !self.in_flight_connectors.contains(&id)
                {
                    return Err(anyhow!(
                        "INTERNAL_INVARIANT: Connector {id} was not prepared for visible commit"
                    ));
                }
                connector_ids.insert(id);
            }
            self.candidate_touched_ports.insert(port_id);
            ports.push(PreparedContentPort {
                id: port_id,
                record,
                ui_key,
                ui_retire_after_receipt,
                mounted,
                old_connector_id: old_id,
                old_connector_index: None,
                old_control_revision,
                next_connector_id: next_id,
                next_connector_index: None,
                retry_selection: false,
            });
            Ok(())
        };

        for (port_id, mounted, next_connector) in changed_ports {
            add_port(port_id, mounted, next_connector)?;
        }

        // A content-only candidate has no binding change, but its touched
        // Connector still owns the prepared projection/frontier that must be
        // promoted by the receipt rather than discarded by cleanup.
        connector_ids.extend(self.candidate_touched_connectors.iter().copied());
        let mut connector_ids = connector_ids.into_iter().collect::<Vec<_>>();
        connector_ids.sort_unstable();
        let mut connectors = Vec::with_capacity(connector_ids.len());
        let visible_connector_ids = ports
            .iter()
            .filter(|port| port.mounted)
            .filter_map(|port| port.next_connector_id)
            .collect::<HashSet<_>>();
        let hidden_connector_ids = ports
            .iter()
            .filter_map(|port| {
                port.old_connector_id
                    .filter(|id| Some(*id) != port.next_connector_id)
            })
            .collect::<HashSet<_>>();
        for connector_id in connector_ids {
            self.candidate_touched_connectors.insert(connector_id);
            let visible = if visible_connector_ids.contains(&connector_id) {
                true
            } else if hidden_connector_ids.contains(&connector_id) {
                false
            } else {
                self.connectors
                    .get(&connector_id)
                    .ok_or_else(|| {
                        anyhow!(
                            "INTERNAL_INVARIANT: touched Connector {connector_id} disappeared during candidate preparation"
                        )
                    })?
                    .lock()
                    .map_err(|_| {
                        anyhow!(
                            "Connector lock is poisoned during candidate preparation"
                        )
                    })?
                    .visible
            };
            connectors.push(self.prepare_connector_commit(connector_id, visible)?);
        }
        // Keep Connector records in the same Source-ID order used by the
        // merged lock plan. Port records retain direct prepared indexes, so
        // receipt-time association never searches this vector.
        connectors.sort_unstable_by_key(|connector| (connector.source_id, connector.id));
        // Ensure every prepared visible deadline has an owned slot before the
        // backend receipt. Receipt-time promotion only updates/removes these
        // existing entries; newer control operations may consume capacity
        // without making the old candidate depend on spare shared space.
        for connector in &connectors {
            if connector.visible
                && let Some(deadline) = connector.deadline
            {
                self.active_connectors.insert(connector.id);
                self.active_deadlines.insert(connector.id, deadline);
            }
        }
        let mut sources = Vec::with_capacity(connectors.len());
        for connector in &connectors {
            sources.push(PreparedContentSource {
                id: connector.source_id,
                source: connector.source.clone(),
            });
        }
        sources.sort_unstable_by_key(|source| source.id);
        sources.dedup_by_key(|source| source.id);
        // Preflight the merged Source lock plan in global Source-ID order.
        // Receipt-time code uses the same candidate-owned order and never
        // deduplicates Source Arcs with a linear pointer scan.
        for source in &sources {
            let _guard = source.source.record.lock().map_err(|_| {
                anyhow!("content Source lock is poisoned during candidate preparation")
            })?;
        }
        let mut source_cleanups = Vec::with_capacity(connectors.len());
        for connector in &connectors {
            // Keep one candidate-owned cleanup slot for every Connector that
            // this candidate hides. A newer disposal can arrive while the
            // receipt is pending even when the Connector was already
            // unsubscribed at capture; the commit then recomputes membership
            // release from the current lifecycle without rediscovering the
            // record through a registry scan.
            if !connector.visible {
                source_cleanups.push(PreparedSourceCleanup {
                    source: connector.source.clone(),
                    source_id: connector.source_id,
                    record: connector.record.clone(),
                    connector_id: connector.id,
                    connector_generation: connector.generation,
                    unsubscribe: connector.subscribed,
                    error: Arc::new(ContentConnectorError {
                        code: "SOURCE_CLEANUP_PENDING".to_owned(),
                        diagnostic: format!(
                            "Source {} cleanup for Connector {} was deferred",
                            connector.source_id, connector.id
                        ),
                    }),
                });
            }
        }
        source_cleanups.sort_unstable_by_key(|cleanup| (cleanup.source_id, cleanup.connector_id));
        // A Source lock can become poisoned after preparation but before the
        // logical frame receipt commits.  Reserve the deferred-cleanup
        // capacity now so conservative post-promotion retention cannot make
        // receipt completion fallible through vector/table growth.
        self.pending_source_cleanups.reserve(source_cleanups.len());
        self.pending_source_cleanup_ids
            .reserve(source_cleanups.len());

        let connector_indexes = connectors
            .iter()
            .enumerate()
            .map(|(index, connector)| (connector.id, index))
            .collect::<HashMap<_, _>>();
        for port in &mut ports {
            port.old_connector_index = port.old_connector_id.map(|id| {
                *connector_indexes
                    .get(&id)
                    .expect("prepared old Connector must be indexed")
            });
            port.next_connector_index = port.next_connector_id.map(|id| {
                *connector_indexes
                    .get(&id)
                    .expect("prepared next Connector must be indexed")
            });
            let desired = port
                .record
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned during candidate preparation"))?
                .desired_connector;
            let active_desired = desired.filter(|desired_id| {
                connector_indexes
                    .get(desired_id)
                    .is_some_and(|index| connectors[*index].requested || connectors[*index].visible)
            });
            port.retry_selection = port.next_connector_id != active_desired;
        }
        let mut binding_changes = Vec::with_capacity(self.candidate_binding_changes.len());
        for (port_index, port) in ports.iter().enumerate() {
            let revision = self
                .candidate_binding_revisions
                .get(&port.id)
                .copied()
                .unwrap_or(0);
            binding_changes.push(PreparedContentBindingChange {
                port_index,
                revision,
            });
        }
        crate::perf::add(
            crate::perf::Counter::ContentCandidateRecordsPrepared,
            (ports.len() + connectors.len() + sources.len()) as u64,
        );
        self.candidate_commit_prepared = true;
        Ok(PreparedContentCommit {
            ports,
            connectors,
            sources,
            source_cleanups,
            binding_changes,
        })
    }

    /// Applies the native/unit-only operational failure hook at candidate
    /// preparation time. This keeps activation request state truthful and
    /// exercises the same old-visible rollback boundary that real projection
    /// errors will use in PERF-13-F.
    fn prepare_activation_candidate(
        &mut self,
        connector_id: u64,
        offered_width: u16,
    ) -> Result<bool> {
        self.touch_connector(connector_id);
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation candidate {connector_id} disappeared"
            ));
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            if state.activation_failure.is_none() {
                return Ok(false);
            }
            return Err(anyhow!(
                "INTERNAL_INVARIANT: activation failure targeted a non-candidate Connector {connector_id}"
            ));
        }
        if state.error.is_some() && state.projection_failure_key.is_some() {
            // One candidate may measure the same Connector at several widths
            // (for example an unconstrained probe followed by the committed
            // width). Preserve the failed attempt across those probes instead
            // of consuming a synthetic failure once and accidentally selecting
            // the Connector on a later width pass in the same frame.
            return Ok(true);
        }
        if state.activation_failure.is_none() {
            return Ok(false);
        }
        // Capture the revision and input key at the start of the failed
        // attempt. A concurrent Source mutation may commit while the host is
        // still preparing this candidate; recording the pre-attempt revision
        // and exact width prevents a second layout measurement in this same
        // frame from retrying the identical failed input.
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        let snapshot = self.source_snapshot_for(&state.source)?;
        let key = TextProjectionKey {
            source_id: snapshot.source_id,
            source_generation: snapshot.source_generation,
            content_generation: snapshot.content_generation,
            source_revision: snapshot.revision,
            source_base: snapshot.source_base,
            source_end: snapshot.source_end,
            head_partial: snapshot.head_partial,
            width: offered_width,
            wrap: state.funnel.wrap,
            funnel_kind: state.funnel.kind,
            delivery_revision: state.delivery_revision,
            theme_revision: self.theme_revision,
            needs_finalized_prefix: self.history_adapter.unit_id(port_id).is_some(),
            needs_physical_rows: self.history_adapter.unit_id(port_id).is_some()
                || state.funnel.smooth_config().is_some(),
        };
        let attempted_source_revision = snapshot.revision;
        let diagnostic = state
            .activation_failure
            .take()
            .expect("activation failure was checked above");
        state.error = Some(ContentConnectorError {
            code: "PROJECTION_FAILED".to_owned(),
            diagnostic,
        });
        state.failed_source_revision = Some(attempted_source_revision);
        state.projection_failure_key = Some(key);
        state.phase = "failed";
        Ok(true)
    }

    /// Retains the Connector IDs referenced by a candidate frame. Control
    /// mutations accepted while a backend receipt is in flight must not
    /// destroy or detach an identity that the captured candidate still uses.
    pub(crate) fn begin_prepared_candidate(&mut self, plan: &PreparedContentCommit) {
        self.in_flight_connectors.clear();
        for connector in &plan.connectors {
            self.in_flight_connectors.insert(connector.id);
        }
    }

    /// Releases the candidate lease after its logical frame commit. A
    /// disposing Connector selected by that frame remains visible/disposing
    /// until the following removal frame, as required by transactional
    /// disposal semantics.
    pub(crate) fn end_candidate(&mut self) {
        self.in_flight_connectors.clear();
        // Candidate projection cleanup is staged in PreparedContentCommit and
        // runs before this receipt-time lease release.  Keeping this method
        // to constant-time ownership flags avoids a post-receipt registry scan
        // and cannot consume newer desired operations.
        self.candidate_selections.clear();
        self.candidate_content_captures.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.candidate_source_snapshots.borrow_mut().clear();
        self.candidate_content_captures.clear();
    }

    /// Aborts a candidate without changing visible bindings. Deferred control
    /// mutations can now finalize identities that were never made visible and
    /// inactive requested Connectors can lose provisional subscriptions.
    pub(crate) fn abort_candidate(&mut self) {
        self.clear_candidate_projections();
        let connector_ids = self.in_flight_connectors.drain().collect::<Vec<_>>();
        for connector_id in connector_ids.iter().copied() {
            self.cleanup_aborted_candidate(connector_id);
        }
        let touched = self
            .candidate_touched_connectors
            .iter()
            .copied()
            .collect::<Vec<_>>();
        self.finalize_disposed_connectors(&touched);
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.candidate_source_snapshots.borrow_mut().clear();
    }

    /// Suspends a candidate while an immutable layout/paint request is still
    /// running. Capture IDs remain live because the next queue turn may need
    /// to refine the same captured product; no visible binding is promoted.
    pub(crate) fn suspend_candidate_for_async(&mut self) {
        self.clear_candidate_projections();
        self.candidate_selections.clear();
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.preserve_captures_for_async = true;
        self.candidate_source_snapshots.borrow_mut().clear();
    }

    fn cleanup_aborted_candidate(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        let state = connector
            .lock()
            .expect("Connector lock must remain usable while aborting a candidate");
        let source = state.source.clone();
        let generation = state.generation;
        let visible = state.visible;
        let requested = state.requested;
        let port_mounted = state.port.upgrade().is_some_and(|port| {
            port.lock()
                .expect("ContentPort lock must remain usable while aborting a candidate")
                .desired_mounted
        });
        drop(state);
        if !visible && (!requested || !port_mounted) {
            self.unsubscribe_connector(&source, connector_id, generation)
                .expect("aborted candidate Source unsubscribe must be valid");
        }
    }

    #[cfg(test)]
    fn promote_candidate_projection(&mut self, connector_id: u64) {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        if let Ok(mut state) = connector.lock()
            && let Some(projection) = state.candidate_projection.take()
        {
            state.projected_source_revision = Some(projection.key.source_revision);
            state.committed_delivery_frontier = state.candidate_delivery_frontier;
            state.committed_projection = Some(projection);
        }
    }

    fn clear_candidate_projections(&mut self) {
        self.candidate_selections.clear();
        for connector_id in self.candidate_touched_connectors.iter() {
            let Some(connector) = self.connectors.get(connector_id).cloned() else {
                continue;
            };
            let mut state = connector
                .lock()
                .expect("Connector lock must remain usable during candidate cleanup");
            state.candidate_projection = None;
            state.candidate_delivery_frontier = state.committed_delivery_frontier;
            if !state.visible && !state.requested {
                state.committed_projection = None;
                state.projection_cache.clear();
                state.prefix_proof_cache.clear();
                state.projected_source_revision = None;
                state.execution = None;
                state.delivery_revision = 0;
                state.candidate_delivery_frontier = StreamOffset::ZERO;
                state.committed_delivery_frontier = StreamOffset::ZERO;
            }
        }
        for connector_id in &self.candidate_touched_connectors {
            let active = self.connectors.get(connector_id).is_some_and(|connector| {
                let state = connector
                    .lock()
                    .expect("Connector lock must remain usable during candidate cleanup");
                state.visible || state.requested
            });
            if !active {
                self.active_deadlines.remove(connector_id);
                self.active_connectors.remove(connector_id);
            }
        }
    }

    fn defer_source_cleanup(&mut self, cleanup: &PreparedSourceCleanup) {
        // `prepare_content_commit` reserves both candidate-owned tables
        // before the backend handoff.  Receipt-time deferral therefore keeps
        // the old Source membership without growing a shared table.
        if self.pending_source_cleanup_ids.insert(cleanup.connector_id) {
            self.pending_source_cleanups.push(cleanup.clone());
        }
    }

    fn finish_source_cleanup(&mut self, connector_id: u64) {
        if self.pending_source_cleanup_ids.remove(&connector_id) {
            self.pending_source_cleanups
                .retain(|cleanup| cleanup.connector_id != connector_id);
        }
    }

    /// Promotes a candidate association using only the records captured by
    /// `PreparedContentCommit`. All potentially poisonable locks are checked
    /// before the first visible mutation. Receipt-time work uses the
    /// candidate's indexed records and preallocated vectors; it does not build
    /// guard arrays, resolve handles, scan registries, or grow shared
    /// association tables. Returns `true` when a Source cleanup was deferred
    /// after logical promotion and therefore requires another host candidate.
    pub(crate) fn commit_prepared(&mut self, plan: &PreparedContentCommit) -> Result<bool> {
        // Preflight every independently poisonable record without retaining a
        // guard collection across the backend receipt. Host serialization plus
        // this complete pass ensures the body below has no ordinary fallible
        // operation after the first visible mutation.
        for port in &plan.ports {
            let _guard = port
                .record
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned during visible commit"))?;
        }
        for connector in &plan.connectors {
            let _guard = connector
                .record
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned during visible commit"))?;
        }
        // Source records are shared and independently mutable.  Preflight the
        // merged candidate-owned lock plan before any visible association
        // mutation.  Cleanup itself is intentionally deferred until after the
        // logical promotion: a Source that becomes poisoned between this
        // check and cleanup must retain its old membership/subscription rather
        // than partially tearing down an old-visible Connector.
        for source in &plan.sources {
            let _guard =
                source.source.record.lock().map_err(|_| {
                    anyhow!("content Source lock is poisoned during visible commit")
                })?;
        }

        // --- all ordinary pre-promotion fallibility ends above this line ---
        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            if connector.visible {
                if let Some(projection) = connector.candidate_projection.as_ref() {
                    state.projected_source_revision = Some(projection.key.source_revision);
                    // The projection and frontier belong to the prepared
                    // candidate, not to the mutable Connector slot. A newer
                    // delivery tick may have cleared/replaced that slot while
                    // this receipt was outstanding; publishing the current
                    // slot would skip the captured frame or expose newer work
                    // through an older receipt.
                    state.committed_delivery_frontier = connector.delivery_frontier;
                    state.committed_projection = Some(Arc::clone(projection));
                } else if state.committed_delivery_frontier == StreamOffset::ZERO
                    && connector.delivery_frontier > StreamOffset::ZERO
                {
                    // A content-only plan can carry a frontier without a new
                    // projection. Preserve the captured first visible
                    // frontier, but never inspect/publish newer candidate
                    // state here.
                    state.committed_delivery_frontier = connector.delivery_frontier;
                }
            }
        }

        for port in &plan.ports {
            if let Some(old_index) = port.old_connector_index
                && Some(old_index) != port.next_connector_index
            {
                let old = &plan.connectors[old_index];
                let preserve_newer_control = port.old_control_revision.is_some_and(|revision| {
                    old.record
                        .lock()
                        .expect("prepared old Connector lock must remain usable")
                        .control_revision
                        != revision
                });
                set_connector_visible_committed(
                    &mut self.active_deadlines,
                    &mut self.active_connectors,
                    old,
                    false,
                    preserve_newer_control,
                );
            }
            let mut state = port
                .record
                .lock()
                .expect("prepared ContentPort lock must remain usable after preflight");
            state.visible_mounted = port.mounted;
            state.visible_connector = if port.mounted {
                port.next_connector_id
            } else {
                None
            };
            drop(state);
            if let Some(next_index) = port.next_connector_index {
                set_connector_visible_committed(
                    &mut self.active_deadlines,
                    &mut self.active_connectors,
                    &plan.connectors[next_index],
                    true,
                    false,
                );
            }
        }
        // Promote the qualified confirmed product only for UI Ports captured
        // in this receipt. The Port record carries its occurrence identity,
        // while the adapter index carries the exact native Connector owner;
        // neither side needs a whole-host scan or a Port key masquerading as
        // a Connector key. A superseded derived adapter is marked Disposing
        // here and removed by the same captured Source-cleanup plan below.
        for port in &plan.ports {
            let Some(ui_key) = port.ui_key else {
                continue;
            };
            let visible_id = port
                .record
                .lock()
                .expect("prepared ContentPort lock must remain usable after preflight")
                .visible_connector;
            if let Some(visible_id) = visible_id {
                let connector_key = self
                    .ui_connector_keys_by_id
                    .get(&visible_id)
                    .copied()
                    .expect("visible UI adapter must retain its qualified Connector key");
                self.ui_confirmed_connectors
                    .insert(ui_key, (connector_key, visible_id));
            } else {
                self.ui_confirmed_connectors.remove(&ui_key);
            }
            if port.old_connector_id != visible_id
                && let Some(old_id) = port.old_connector_id
                && self.ui_connector_keys_by_id.remove(&old_id).is_some()
            {
                // An unmounted Port has no replacement adapter.  Clear the
                // execution index only when it still points at the retired
                // adapter; a successful A/B switch already installed the new
                // candidate under this UI key and must retain that mapping.
                if self.ui_connectors.get(&ui_key).copied() == Some(old_id) {
                    self.ui_connectors.remove(&ui_key);
                    self.ui_connector_keys.remove(&ui_key);
                }
                self.retire_derived_connector(old_id);
            }
        }
        for change in &plan.binding_changes {
            let port = &plan.ports[change.port_index];
            let unresolved = {
                let state = port
                    .record
                    .lock()
                    .expect("prepared ContentPort lock must remain usable after preflight");
                let mounted_unresolved = state.desired_mounted != state.visible_mounted;
                let selection_unresolved =
                    state.desired_connector != state.visible_connector && port.retry_selection;
                mounted_unresolved || selection_unresolved
            };
            if !unresolved
                && self.pending_binding_revisions.get(&port.id).copied() == Some(change.revision)
            {
                self.pending_binding_changes.remove(&port.id);
                self.pending_binding_revisions.remove(&port.id);
            }
        }

        // Deadline membership is promoted from the same prepared connector
        // records. Newer desired operations retain their own pending epoch;
        // no current Source lookup is needed at receipt time.
        for connector in &plan.connectors {
            let state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            let current_delivery_input = state.execution.as_ref().and_then(|execution| {
                execution.delivery.as_ref().map(|delivery| {
                    (
                        delivery.indexed_generation,
                        delivery.indexed_revision,
                        delivery.indexed_sealed,
                    )
                })
            });
            let current_delivery_revision = state.delivery_revision;
            let current_control_revision = state.control_revision;
            drop(state);
            if connector.visible {
                if current_delivery_input != connector.delivery_input
                    || current_delivery_revision != connector.delivery_revision
                    || current_control_revision != connector.control_revision
                {
                    // A newer Source wake advanced this Connector while the
                    // old receipt was outstanding. Its newer deadline/index
                    // is already authoritative and must not be overwritten
                    // by the old candidate's schedule.
                    continue;
                }
                if let Some(deadline) = connector.deadline {
                    if let Some(current) = self.active_deadlines.get_mut(&connector.id) {
                        *current = deadline;
                    }
                } else {
                    self.active_deadlines.remove(&connector.id);
                    self.active_connectors.remove(&connector.id);
                }
            } else {
                self.active_deadlines.remove(&connector.id);
                self.active_connectors.remove(&connector.id);
            }
        }

        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable after preflight");
            if state.lifecycle == ConnectorLifecycle::Disposed {
                continue;
            }
            if state.visible {
                state.cleanup_error = None;
                state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                    "disposing"
                } else {
                    "active"
                };
            } else if state.lifecycle == ConnectorLifecycle::Disposing {
                state.phase = "disposing";
            } else if state.requested {
                state.phase = if state.error.is_some() {
                    "failed"
                } else if connector.visible {
                    "activation-pending"
                } else {
                    "waiting-for-mount"
                };
            } else {
                state.phase = "idle";
            }
        }
        // A Connector reactivated while its older cleanup was pending no
        // longer needs that cleanup.  It was included in this candidate via
        // the pending ID table, so cancel the stale deferred entry without a
        // registry scan.
        for connector in &plan.connectors {
            if connector.visible {
                self.finish_source_cleanup(connector.id);
            }
        }

        // Source association cleanup is deliberately after logical
        // promotion.  If a Source becomes poisoned in this narrow window,
        // retain both its membership and wake subscription and retry from a
        // later candidate; do not report the already-promoted frame as an
        // aborted commit or partially tear down an old-visible Connector.
        #[cfg(test)]
        let mut source_cleanup_completed = false;
        for cleanup in &plan.source_cleanups {
            #[cfg(test)]
            if source_cleanup_completed
                && self.test_poison_source_after_first_cleanup == Some(cleanup.source_id)
            {
                self.test_poison_source_after_first_cleanup = None;
                let source_record = cleanup.source.record.clone();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _guard = source_record.lock().unwrap();
                    panic!("intentional post-promotion Source cleanup failure");
                }));
            }
            let Ok(mut state) = cleanup.record.lock() else {
                self.defer_source_cleanup(cleanup);
                continue;
            };
            let unsubscribe = cleanup.unsubscribe && !state.requested && state.subscribed;
            let release_membership =
                !state.membership_released && state.lifecycle == ConnectorLifecycle::Disposing;
            if !unsubscribe && !release_membership {
                state.cleanup_error = None;
                drop(state);
                self.finish_source_cleanup(cleanup.connector_id);
                continue;
            }
            let Ok(mut source) = cleanup.source.record.lock() else {
                state.cleanup_error = Some(Arc::clone(&cleanup.error));
                drop(state);
                self.defer_source_cleanup(cleanup);
                continue;
            };
            if unsubscribe {
                remove_source_subscription_locked(
                    &mut source,
                    &self.owner_host,
                    cleanup.connector_id,
                    cleanup.connector_generation,
                );
                state.subscribed = false;
            }
            if release_membership {
                release_source_membership_locked(&mut source);
                state.membership_released = true;
            }
            drop(source);
            state.cleanup_error = None;
            drop(state);
            self.finish_source_cleanup(cleanup.connector_id);
            #[cfg(test)]
            {
                source_cleanup_completed = true;
            }
        }
        for connector in &plan.connectors {
            let mut state = connector
                .record
                .lock()
                .expect("prepared Connector lock must remain usable during cleanup");
            let captured_projection_is_current = match (
                connector.candidate_projection.as_ref(),
                state.candidate_projection.as_ref(),
            ) {
                (Some(captured), Some(current)) => Arc::ptr_eq(captured, current),
                (None, None) => true,
                _ => false,
            };
            if captured_projection_is_current
                && state.control_revision == connector.control_revision
                && state.delivery_revision == connector.delivery_revision
            {
                state.candidate_projection = None;
                state.candidate_delivery_frontier = state.committed_delivery_frontier;
                if !state.visible && !state.requested {
                    state.committed_projection = None;
                    state.projection_cache.clear();
                    state.prefix_proof_cache.clear();
                    state.projected_source_revision = None;
                    state.execution = None;
                    state.delivery_revision = 0;
                    state.candidate_delivery_frontier = StreamOffset::ZERO;
                    state.committed_delivery_frontier = StreamOffset::ZERO;
                }
            }
            let active = state.visible || state.requested;
            if !active {
                self.active_deadlines.remove(&connector.id);
                self.active_connectors.remove(&connector.id);
            }
        }
        for connector in &plan.connectors {
            if self.pending_source_cleanup_ids.contains(&connector.id) {
                continue;
            }
            remove_prepared_connector_committed(&mut self.connectors, connector);
        }
        for port in &plan.ports {
            if !port.ui_retire_after_receipt {
                continue;
            }
            let removable = port
                .record
                .lock()
                .expect("prepared ContentPort lock must remain usable during retirement")
                .connector_ids
                .is_empty();
            if removable {
                let mut state = port
                    .record
                    .lock()
                    .expect("prepared ContentPort lock must remain usable during retirement");
                state.lifecycle = PortLifecycle::Disposed;
                drop(state);
                self.ports
                    .remove(&port.id)
                    .expect("retired UI ContentPort must remain owned until receipt cleanup");
            }
        }
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_commit_prepared = false;
        Ok(!self.pending_source_cleanup_ids.is_empty())
    }

    pub(crate) fn fail_next_activation(
        &mut self,
        connector_id: u64,
        diagnostic: String,
    ) -> Result<()> {
        if diagnostic.is_empty() {
            return Err(anyhow!("activation failure diagnostic cannot be empty"));
        }
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live {
            return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not live"));
        }
        if state.visible {
            return Err(anyhow!(
                "INVALID_ARGUMENT: cannot inject activation failure for a visible Connector"
            ));
        }
        state.activation_failure = Some(diagnostic);
        Ok(())
    }

    fn request_activation(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (
            port,
            generation,
            source,
            was_requested,
            was_selected,
            port_mounted,
            was_failed,
            smooth,
        ) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not activatable"));
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (was_selected, port_mounted) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    port_state.desired_connector == Some(connector_id),
                    port_state.desired_mounted,
                )
            };
            (
                port,
                state.generation,
                state.source.clone(),
                state.requested,
                was_selected,
                port_mounted,
                state.error.is_some(),
                state.funnel.smooth_config().is_some(),
            )
        };
        if was_requested && was_selected && !was_failed {
            return Ok(false);
        }
        let old_selected = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        self.touch_port(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id,
        );
        if let Some(old_id) = old_selected
            && old_id != connector_id
        {
            self.clear_requested(old_id)?;
        }
        {
            let mut port_state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            port_state.desired_connector = Some(connector_id);
        }
        self.mark_binding_change(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .id,
        );
        {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = true;
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
            state.phase = if port_mounted {
                "activation-pending"
            } else {
                "waiting-for-mount"
            };
        }
        if smooth && port_mounted {
            self.active_connectors.insert(connector_id);
        } else {
            // Immediate delivery has no native deadline, and a smooth
            // Connector selected before its Port is mounted is cold. Keep
            // this index reserved for mounted connectors whose clock can
            // actually advance so unrelated host work cannot trigger parser
            // or delivery work for a cold destination.
            self.active_connectors.remove(&connector_id);
        }
        if port_mounted {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        // The request itself is not the activation/projection operation. A
        // A mounted candidate is processed during content measurement inside
        // the frame transaction, where injected/real operational failure can
        // fall back to the committed Connector without changing the visible
        // frame.
        Ok(port_mounted)
    }

    fn request_deactivation(&mut self, connector_id: u64) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let (port, source, generation, was_visible, was_requested, was_selected, visible_connector) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle != ConnectorLifecycle::Live {
                return Err(anyhow!("CONNECTOR_DISPOSING: Connector is not active"));
            }
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (was_selected, visible_connector) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    port_state.desired_connector == Some(connector_id),
                    port_state.visible_connector,
                )
            };
            (
                port,
                state.source.clone(),
                state.generation,
                state.visible,
                state.requested,
                was_selected,
                visible_connector,
            )
        };
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        self.touch_port(port_id);
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        if !was_requested && !was_visible && !in_flight {
            return Ok(false);
        }
        self.mark_binding_change(port_id);
        if was_selected
            && port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .desired_connector
                == Some(connector_id)
        {
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
                .desired_connector = None;
        }
        // A captured candidate may still select this Connector even though it
        // is not visible in the old frame. Keep its subscription and identity
        // until that receipt commits or aborts; otherwise the old candidate
        // can resurrect a deactivated Connector without a follow-up epoch.
        if !was_visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        state.control_revision = state
            .control_revision
            .checked_add(1)
            .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
        {
            state.requested = false;
            state.phase = if state.visible { "active" } else { "idle" };
        }
        if !was_visible {
            self.active_connectors.remove(&connector_id);
        }
        // A failed switch keeps the old visible Connector (rollback)
        // while the requested candidate remains selected. Deactivating that
        // candidate must still schedule the removal of the rolled-back visible;
        // otherwise the port would stay visibly active with no requested
        // Connector and no pending epoch to remove it.
        Ok(was_visible
            || in_flight
            || (was_requested && was_selected && visible_connector.is_some()))
    }

    fn request_connector_disposal(&mut self, connector_id: u64) -> Result<bool> {
        self.touch_connector(connector_id);
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let in_flight = self.in_flight_connectors.contains(&connector_id);
        let (port, source, generation, visible, desired, visible_connector) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            if state.lifecycle == ConnectorLifecycle::Disposed
                || state.lifecycle == ConnectorLifecycle::Disposing
            {
                return Ok(false);
            }
            state.lifecycle = ConnectorLifecycle::Disposing;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = false;
            state.phase = "disposing";
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            let (desired, visible_connector) = {
                let port_state = port
                    .lock()
                    .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
                (
                    (port_state.desired_connector == Some(connector_id)).then_some(()),
                    port_state.visible_connector,
                )
            };
            (
                port,
                state.source.clone(),
                state.generation,
                state.visible,
                desired,
                visible_connector,
            )
        };
        let port_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        self.touch_port(port_id);
        self.mark_binding_change(port_id);
        if !visible && !in_flight {
            self.unsubscribe_connector(&source, connector_id, generation)?;
            self.active_connectors.remove(&connector_id);
        }
        if desired.is_some()
            && let Ok(mut port_state) = port.lock()
        {
            port_state.desired_connector = None;
        }
        if visible || in_flight {
            // The captured candidate still owns a short-lived identity lease;
            // finalize only after commit/abort reconciles that candidate.
            return Ok(true);
        }
        let removes_visible_rollback = desired.is_some() && visible_connector.is_some();
        self.remove_connector(connector_id);
        Ok(removes_visible_rollback)
    }

    fn dispose_port(&mut self, port: &Arc<Mutex<PortRecord>>) -> Result<()> {
        let (id, desired_mounted, visible_mounted, connector_ids) = {
            let state = port
                .lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?;
            (
                state.id,
                state.desired_mounted,
                state.visible_mounted,
                state.connector_ids.clone(),
            )
        };
        if desired_mounted || visible_mounted {
            return Err(anyhow!(
                "PORT_MOUNTED: ContentPort {id} is still structurally mounted"
            ));
        }
        if !connector_ids.is_empty() {
            return Err(anyhow!(
                "PORT_IN_USE: ContentPort {id} still has Connector membership"
            ));
        }
        if let Ok(mut state) = port.lock() {
            state.lifecycle = PortLifecycle::Disposed;
        }
        self.ports.remove(&id);
        Ok(())
    }

    pub(crate) fn dispose_all(&mut self) {
        for pending in self
            .pending_content_projections
            .drain()
            .map(|(_, pending)| pending)
        {
            pending.cancelled.store(true, Ordering::Release);
        }
        self.release_deferred_projection_waiter_if_idle();
        self.in_flight_connectors.clear();
        let connector_ids = self.connectors.keys().copied().collect::<Vec<_>>();
        for connector_id in connector_ids {
            self.remove_connector(connector_id);
        }
        for port in self.ports.values() {
            if let Ok(mut state) = port.lock() {
                state.lifecycle = PortLifecycle::Disposed;
                state.desired_mounted = false;
                state.visible_mounted = false;
                state.desired_connector = None;
                state.visible_connector = None;
            }
        }
        self.ports.clear();
        self.active_connectors.clear();
        self.active_sync_scratch.clear();
        self.due_connector_scratch.clear();
        self.candidate_selections.clear();
        self.candidate_content_captures.clear();
        self.pending_binding_changes.clear();
        self.pending_binding_revisions.clear();
        self.candidate_binding_changes.clear();
        self.candidate_binding_revisions.clear();
        self.candidate_touched_connectors.clear();
        self.candidate_touched_ports.clear();
        self.pending_source_cleanups.clear();
        self.pending_source_cleanup_ids.clear();
        #[cfg(test)]
        {
            self.test_poison_source_after_first_cleanup = None;
        }
        self.candidate_source_snapshots.borrow_mut().clear();
        self.candidate_capture_active = false;
        self.candidate_commit_prepared = false;
        self.history_adapter = HistoryTerminalAdapter::new();
        self.ui_ports.clear();
        self.retired_ui_ports.clear();
        self.ui_connectors.clear();
        self.ui_connector_keys.clear();
        self.ui_connector_keys_by_id.clear();
        self.ui_confirmed_connectors.clear();
        self.ui_failure_injections.clear();
    }

    fn refresh_requested_phase(
        &mut self,
        connector_id: u64,
        mounted: bool,
        remounted: bool,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.lifecycle != ConnectorLifecycle::Live || !state.requested || state.visible {
            return Ok(());
        }
        if remounted && mounted {
            // A failed candidate is retryable on a real remount. Clear only
            // the old operational diagnostic here; an error from the new
            // candidate will be recorded again during frame preparation.
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
        }
        if !mounted && !state.visible {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
        }
        state.phase = if state.error.is_some() && mounted {
            "failed"
        } else if mounted {
            "activation-pending"
        } else {
            "waiting-for-mount"
        };
        Ok(())
    }

    fn ensure_requested_subscription(
        &mut self,
        connector_id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, requested, visible, lifecycle) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (
                state.source.clone(),
                state.generation,
                state.requested,
                state.visible,
                state.lifecycle,
            )
        };
        if requested && !visible && lifecycle == ConnectorLifecycle::Live {
            self.subscribe_connector(connector_id, &source, generation, host)?;
        }
        Ok(())
    }

    fn unsubscribe_requested_if_not_visible(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        Ok(())
    }

    fn clear_requested(&mut self, connector_id: u64) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Ok(());
        };
        let (source, generation, visible) = {
            let mut state = connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?;
            state.control_revision = state
                .control_revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("Connector control revision exhausted"))?;
            state.requested = false;
            if !state.visible {
                state.phase = "idle";
            }
            (state.source.clone(), state.generation, state.visible)
        };
        if !visible && !self.in_flight_connectors.contains(&connector_id) {
            self.unsubscribe_connector(&source, connector_id, generation)?;
        }
        Ok(())
    }

    fn subscribe_connector(
        &mut self,
        connector_id: u64,
        source: &HostContentSource,
        generation: u32,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<()> {
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return Err(anyhow!("STALE_HANDLE: Connector is unavailable"));
        };
        let already_subscribed = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed;
        if already_subscribed {
            return Ok(());
        }
        source.subscribe(host, connector_id, generation)?;
        connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .subscribed = true;
        Ok(())
    }

    fn unsubscribe_connector(
        &mut self,
        source: &HostContentSource,
        id: u64,
        generation: u32,
    ) -> Result<()> {
        source.unsubscribe(&self.owner_host, id, generation)?;
        if let Some(connector) = self.connectors.get(&id) {
            connector
                .lock()
                .map_err(|_| anyhow!("Connector lock is poisoned"))?
                .subscribed = false;
        }
        Ok(())
    }

    #[cfg(test)]
    fn set_connector_visible_record(
        &mut self,
        connector_id: u64,
        connector: &Arc<Mutex<ConnectorRecord>>,
        visible: bool,
        synchronize_deadline: bool,
        preserve_newer_control: bool,
    ) -> Result<()> {
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned during visibility update"))?;
        state.visible = visible;
        let source = state.source.clone();
        let generation = state.generation;
        if visible {
            state.phase = if state.lifecycle == ConnectorLifecycle::Disposing {
                "disposing"
            } else {
                "active"
            };
        } else if !preserve_newer_control {
            state.committed_projection = None;
            state.candidate_projection = None;
            state.projection_cache.clear();
            state.prefix_proof_cache.clear();
            state.projected_source_revision = None;
            state.projection_failure_key = None;
            state.execution = None;
            state.delivery_revision = 0;
            state.candidate_delivery_frontier = StreamOffset::ZERO;
            state.committed_delivery_frontier = StreamOffset::ZERO;
        }
        if !visible && state.lifecycle != ConnectorLifecycle::Disposing {
            state.phase = if state.error.is_some() && state.requested {
                "failed"
            } else if state.requested {
                "activation-pending"
            } else {
                "idle"
            };
        }
        let requested = state.requested;
        drop(state);
        if visible {
            if synchronize_deadline {
                self.sync_connector_deadline(connector_id, None)?;
            }
        } else {
            self.active_deadlines.remove(&connector_id);
            self.active_connectors.remove(&connector_id);
            if !requested {
                source.unsubscribe(&self.owner_host, connector_id, generation)?;
                connector
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned after unsubscribe"))?
                    .subscribed = false;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn set_connector_visible(&mut self, connector_id: u64, visible: bool) {
        self.touch_connector(connector_id);
        let Some(connector) = self.connectors.get(&connector_id).cloned() else {
            return;
        };
        self.set_connector_visible_record(connector_id, &connector, visible, visible, false)
            .expect("test connector visibility update must succeed");
    }

    fn remove_connector(&mut self, connector_id: u64) {
        self.cancel_projection(connector_id);
        self.finish_source_cleanup(connector_id);
        self.active_deadlines.remove(&connector_id);
        self.active_connectors.remove(&connector_id);
        let Some(connector) = self.connectors.remove(&connector_id) else {
            return;
        };
        let mut state = connector
            .lock()
            .expect("Connector lock must remain usable during removal");
        let source = state.source.clone();
        let generation = state.generation;
        let subscribed = state.subscribed;
        let membership_released = state.membership_released;
        let mut source_guard = source
            .record
            .lock()
            .expect("connector Source lock must remain usable during removal");
        if subscribed {
            remove_source_subscription_locked(
                &mut source_guard,
                &self.owner_host,
                connector_id,
                generation,
            );
            state.subscribed = false;
        }
        if !membership_released && state.membership_owned {
            release_source_membership_locked(&mut source_guard);
            state.membership_released = true;
        }
        drop(source_guard);
        state.lifecycle = ConnectorLifecycle::Disposed;
        state.phase = "disposed";
        state.visible = false;
        state.requested = false;
        state.cleanup_error = None;
        let port = state.port.upgrade();
        drop(state);
        if let Some(port) = port {
            let mut port_state = port
                .lock()
                .expect("ContentPort lock must remain usable during removal");
            port_state.connector_ids.remove(&connector_id);
            if port_state.desired_connector == Some(connector_id) {
                port_state.desired_connector = None;
            }
            if port_state.visible_connector == Some(connector_id) {
                port_state.visible_connector = None;
            }
        }
    }

    /// Retires a derived UI Connector after its superseding binding has been
    /// accepted by a native receipt. Source subscription/membership cleanup
    /// remains owned by the captured commit plan; marking the lifecycle here
    /// lets that cleanup defer safely if the Source lock is poisoned without
    /// retaining one adapter per A/B switch.
    fn retire_derived_connector(&mut self, connector_id: u64) {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .expect("derived UI Connector must remain owned until receipt retirement");
        let mut state = connector
            .lock()
            .expect("derived UI Connector lock must remain usable at receipt retirement");
        assert!(
            !state.visible,
            "a derived UI Connector may retire only after visibility promotion"
        );
        if state.lifecycle == ConnectorLifecycle::Live {
            state.lifecycle = ConnectorLifecycle::Disposing;
            state.requested = false;
            state.phase = "disposing";
        }
    }

    fn finalize_disposed_connectors(&mut self, candidate_ids: &[u64]) {
        for id in candidate_ids.iter().copied() {
            let removable = self.connectors.get(&id).is_some_and(|connector| {
                let state = connector
                    .lock()
                    .expect("Connector lock must remain usable during finalization");
                state.lifecycle == ConnectorLifecycle::Disposing && !state.visible
            });
            if removable {
                self.remove_connector(id);
            }
        }
    }

    pub(crate) fn connector_status(&self, id: u64) -> Result<ContentConnectorStatus> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(ContentConnectorStatus {
            phase: state.phase.to_owned(),
            requested: state.requested,
            visible: state.visible,
            projected_source_revision: state.projected_source_revision,
            error: state.error.clone(),
            cleanup_pending: state.cleanup_error.is_some(),
            cleanup_error: state.cleanup_error.as_deref().cloned(),
        })
    }

    pub(crate) fn port_status(&self, id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&id)
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {id} is unavailable"))?;
        Ok(port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .visible_mounted)
    }

    fn dirty_for_port(&self, port_id: u64, reason: ContentDirtyReason) -> ContentDirty {
        ContentDirty::new(port_id, None, reason)
    }

    fn dirty_for_connector(
        &self,
        connector_id: u64,
        reason: ContentDirtyReason,
    ) -> Result<ContentDirty> {
        let connector = self
            .connectors
            .get(&connector_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {connector_id} is unavailable"))?;
        let port_id = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .id;
        Ok(ContentDirty::new(port_id, Some(connector_id), reason))
    }

    pub(crate) fn set_history_unit(
        &mut self,
        port_id: u64,
        unit_id: u64,
        insets: crate::presentation::Insets,
    ) -> Result<()> {
        let port = self
            .ports
            .get(&port_id)
            .ok_or_else(|| anyhow!("INTERNAL_INVARIANT: ContentPort {port_id} disappeared"))?;
        drop(
            port.lock()
                .map_err(|_| anyhow!("ContentPort lock is poisoned"))?,
        );
        self.history_adapter.bind_unit(port_id, unit_id, insets);
        Ok(())
    }

    fn history_rows(&self, port_id: u64, offered_width: u16) -> Option<HistoryContentRows> {
        let port = self.ports.get(&port_id)?;
        let connector_id = {
            let state = port.lock().ok()?;
            state.visible_connector.or(state.desired_connector)
        }?;
        let raw_insets = self.history_adapter.insets(port_id);
        // Match the layout compiler's width-safe horizontal padding clamp.
        // History rows are placed at the offered terminal width, so using the
        // raw semantic inset here would otherwise export a different product
        // at narrow widths while silently dropping the right-side geometry.
        let left = raw_insets.left.min(offered_width.saturating_sub(1));
        let right = raw_insets
            .right
            .min(offered_width.saturating_sub(left.saturating_add(1)));
        let insets = crate::Insets::new(raw_insets.top, right, raw_insets.bottom, left);
        let committed_rows = self.history_adapter.committed_rows(port_id);
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let snapshot = self.source_snapshot_for(&connector.source).ok()?;
        let sealed = snapshot.sealed;
        drop(connector);
        let content_width =
            offered_width.saturating_sub(insets.left().saturating_add(insets.right()));
        let projection = self.connector_projection(connector_id, content_width)?;
        // History consumes the separately proved finalized-prefix product,
        // not a row-count slice of the open document.  An open Markdown tail
        // can be reinterpreted as more bytes arrive, so slicing
        // `projection.rows` would export rows that were never finalized (and
        // could disagree with the sealed-prefix rendering).
        let content_rows = projection
            .finalized_prefix
            .as_ref()
            .map_or(&[][..], |prefix| &prefix.rows[..]);
        let top_padding = usize::from(insets.top());
        // History is irreversible.  Only rows that have crossed the same
        // finalized/delivered frontier used by the screen may be transferred;
        // a sealed Source can still have Smooth backlog.  In particular, do
        // not treat sealing as permission to export the unmasked tail.
        let transferable_content_rows = projection.stable_rows.min(content_rows.len());
        let complete_content = sealed && transferable_content_rows >= content_rows.len();
        let bottom_padding = complete_content
            .then_some(usize::from(insets.bottom()))
            .unwrap_or(0);
        let total_height = top_padding
            .saturating_add(transferable_content_rows)
            .saturating_add(bottom_padding);
        let content_start = top_padding;
        let content_end = content_start.saturating_add(content_rows.len());
        let stable_end = content_start
            .saturating_add(transferable_content_rows)
            .saturating_add(bottom_padding);
        let start = committed_rows.min(total_height);
        let end = stable_end.min(total_height);

        let rows = if start < end {
            (start..end)
                .map(|row_idx| {
                    if row_idx < top_padding || row_idx >= content_end {
                        let cells = vec![PhysicalCell::transparent(); usize::from(offered_width)];
                        PhysicalRow::from_cells(cells)
                    } else if row_idx - top_padding < transferable_content_rows {
                        let content_row = &content_rows[row_idx - top_padding];
                        let placed = content_row.placed(offered_width, insets.left());
                        // Finalized-prefix rows are whole compiled rows.  A
                        // Smooth cut belongs to the open paint product and
                        // must never be applied to this independent History
                        // product.
                        placed
                    } else {
                        PhysicalRow::from_cells(vec![
                            PhysicalCell::transparent();
                            usize::from(offered_width)
                        ])
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let payload_content_start = content_start.max(start).saturating_sub(start);
        let payload_content_end = content_end.min(end).saturating_sub(start);
        let leading_padding = top_padding
            .saturating_sub(start)
            .min(end.saturating_sub(start));
        let trailing_padding = if sealed {
            end.saturating_sub(content_end.max(start))
                .min(bottom_padding)
        } else {
            0
        };
        Some(HistoryContentRows {
            rows,
            complete: complete_content && end >= total_height,
            content_start: payload_content_start.min(payload_content_end),
            content_end: payload_content_end.max(payload_content_start),
            leading_padding,
            trailing_padding,
        })
    }

    pub(crate) fn history_rows_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        self.history_adapter.record_committed(
            port_id,
            rows,
            content_rows,
            leading_padding,
            trailing_padding,
        );
    }

    pub(crate) fn clear_history_unit(&mut self, unit_id: u64) {
        self.history_adapter.clear_unit(unit_id);
    }

    pub(crate) fn history_unit_retired(&mut self, unit_id: u64) {
        let ports = self.history_adapter.retire_unit(unit_id);
        for port_id in ports {
            let ui_keys = self
                .ui_ports
                .iter()
                .filter_map(|(key, candidate)| (*candidate == port_id).then_some(*key))
                .collect::<Vec<_>>();
            self.pending_binding_changes.remove(&port_id);
            self.pending_binding_revisions.remove(&port_id);
            self.candidate_binding_changes.remove(&port_id);
            self.candidate_binding_revisions.remove(&port_id);
            self.candidate_touched_ports.remove(&port_id);
            let connector_ids = if let Some(port) = self.ports.remove(&port_id) {
                if let Ok(mut state) = port.lock() {
                    state.desired_mounted = false;
                    state.visible_mounted = false;
                    state.desired_connector = None;
                    state.visible_connector = None;
                    state.lifecycle = PortLifecycle::Disposed;
                    state.connector_ids.clone()
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            };
            for connector_id in connector_ids {
                self.candidate_touched_connectors.remove(&connector_id);
                self.remove_connector(connector_id);
            }
            for key in ui_keys {
                self.ui_ports.remove(&key);
                self.ui_connectors.remove(&key);
                self.ui_connector_keys.remove(&key);
                self.ui_confirmed_connectors.remove(&key);
                self.retired_ui_ports.insert(key);
            }
        }
    }

    fn connector_is_candidate_ready(&self, id: u64) -> Result<bool> {
        let Some(connector) = self.connectors.get(&id) else {
            return Ok(false);
        };
        let state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(state.lifecycle == ConnectorLifecycle::Live
            && state.requested
            && state.error.is_none()
            && state.candidate_projection.is_some())
    }

    pub(super) fn source_subscription_is_live(
        &mut self,
        id: u64,
        generation: u32,
        source_revision: u64,
    ) -> Result<Option<ContentDirty>> {
        let Some(connector) = self.connectors.get(&id).cloned() else {
            return Ok(None);
        };
        let mut state = connector
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if state.generation != generation
            || !state.subscribed
            || state.lifecycle == ConnectorLifecycle::Disposed
        {
            return Ok(None);
        }
        if state.error.is_some()
            && state.requested
            && !state.visible
            && state
                .failed_source_revision
                .is_some_and(|failed_revision| source_revision > failed_revision)
        {
            state.error = None;
            state.failed_source_revision = None;
            state.projection_failure_key = None;
            state.phase = "activation-pending";
        }
        let smooth_delivery = state.funnel.smooth_config().is_some();
        if state.visible {
            // Visible membership is the committed Port association. The
            // immutable cached ID avoids locking the Port record or consulting
            // the inactive/in-flight indexes for the common mounted wake path;
            // host teardown clears visibility before removing the Port. The
            // poison check retains the exceptional retirement failure signal
            // without reacquiring the Port mutex.
            let port = state
                .port
                .upgrade()
                .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
            if port.is_poisoned() {
                return Err(anyhow!("ContentPort lock is poisoned"));
            }
            let port_id = state.port_id;
            drop(state);
            if smooth_delivery {
                self.sync_connector_deadline(id, None)?;
            }
            return Ok(Some(ContentDirty::new(
                port_id,
                Some(id),
                ContentDirtyReason::SourceInput,
            )));
        }
        let in_flight = self.in_flight_connectors.contains(&id);
        let cleanup_pending = self.pending_source_cleanup_ids.contains(&id);
        let port_id = state.port_id;
        let port = state
            .port
            .upgrade()
            .ok_or_else(|| anyhow!("PORT_DISPOSED: Connector's ContentPort is gone"))?;
        let port_mounted = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_mounted;
        let is_live = state.visible
            || (state.requested && port_mounted)
            || in_flight
            // A Source mutation is an independent readiness signal for a
            // deferred cleanup. It admits exactly one retry candidate; a
            // persistently poisoned Source is then blocked by the normal
            // environment failure path rather than spinning on every tick.
            || cleanup_pending;
        drop(state);
        // Immediate Connectors never have a delivery deadline. Avoid a
        // second Connector/Port lock on every Source append in that common
        // case; smooth delivery still resynchronizes its clock after input.
        if is_live && smooth_delivery {
            self.sync_connector_deadline(id, None)?;
        }
        Ok(is_live.then_some(ContentDirty::new(
            port_id,
            Some(id),
            ContentDirtyReason::SourceInput,
        )))
    }

    pub(crate) fn connector_is_disposed(&self, id: u64) -> bool {
        self.connectors
            .get(&id)
            .and_then(|connector| connector.lock().ok())
            .is_none_or(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }

    #[cfg(test)]
    pub(crate) fn poison_connector_for_test(&self, id: u64) -> Result<()> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = connector
                .lock()
                .expect("Connector must be healthy before poison");
            panic!("intentional Connector lock poison");
        }));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn clear_connector_poison_for_test(&self, id: u64) -> Result<()> {
        let connector = self
            .connectors
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: Connector {id} is unavailable"))?;
        connector.clear_poison();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn poison_source_after_first_cleanup_for_test(&mut self, source_id: u64) {
        self.test_poison_source_after_first_cleanup = Some(source_id);
    }

    #[cfg(test)]
    pub(crate) fn clear_source_poison_for_test(&self, source: &HostContentSource) {
        source.record.clear_poison();
    }

    #[cfg(test)]
    pub(crate) fn pending_source_cleanup_count(&self) -> usize {
        self.pending_source_cleanups.len()
    }

    pub(crate) fn has_pending_source_cleanup(&self) -> bool {
        !self.pending_source_cleanup_ids.is_empty()
    }

    fn deactivate_port(&mut self, port_id: u64) -> Result<bool> {
        let port = self
            .ports
            .get(&port_id)
            .cloned()
            .ok_or_else(|| anyhow!("STALE_HANDLE: ContentPort {port_id} is unavailable"))?;
        let connector_id = port
            .lock()
            .map_err(|_| anyhow!("ContentPort lock is poisoned"))?
            .desired_connector;
        match connector_id {
            Some(connector_id) => self.request_deactivation(connector_id),
            None => Ok(false),
        }
    }

    fn request_connector_activation(
        &mut self,
        id: u64,
        host: &Weak<Mutex<HostInner>>,
    ) -> Result<bool> {
        self.request_activation(id, host)
    }

    fn request_connector_deactivation(&mut self, id: u64) -> Result<bool> {
        self.request_deactivation(id)
    }

    fn request_connector_dispose(&mut self, id: u64) -> Result<bool> {
        self.request_connector_disposal(id)
    }
}

impl ContentProvider for ContentHostRegistry {
    fn set_theme(&mut self, theme: &Arc<Theme>) {
        if Arc::ptr_eq(&self.theme, theme) || *self.theme == **theme {
            return;
        }
        self.theme = Arc::clone(theme);
        self.theme_revision = self
            .theme_revision
            .checked_add(1)
            .expect("content theme revision exhausted");
        for connector in self.connectors.values() {
            if let Ok(mut state) = connector.lock() {
                state.projection_cache.clear();
                state.candidate_projection = None;
            }
        }
    }

    fn projection_revision(&self, port_id: u64, offered_width: u16) -> u64 {
        let connector_revision = self
            .selected_connector_id(port_id)
            .map_or(0, |connector_id| {
                self.connector_revision(connector_id, offered_width)
            });
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        connector_revision.hash(&mut hasher);
        self.theme_revision.hash(&mut hasher);
        self.history_adapter.hash_state(port_id, &mut hasher);
        hasher.finish()
    }

    fn layout_input_revision(&self, port_id: u64, offered_width: u16) -> u64 {
        let connector_id = self.selected_connector_id(port_id);
        let connector_revision = connector_id
            .and_then(|id| self.connector_projection_key(id, offered_width).ok())
            .map_or(0, |key| key.layout_input_revision());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        use std::hash::{Hash, Hasher};
        connector_revision.hash(&mut hasher);
        self.history_adapter.hash_state(port_id, &mut hasher);
        hasher.finish()
    }

    fn measure(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::ContentWidthRule,
    ) -> ContentMeasurement {
        self.measure_content(port_id, offered_width, width_rule)
    }

    fn capture_measurement(
        &mut self,
        port_id: u64,
        offered_width: u16,
        width_rule: crate::presentation::ContentWidthRule,
    ) -> anyhow::Result<ContentMeasurementCapture> {
        let measurement = self.measure_content(port_id, offered_width, width_rule);
        let capture_id = next_content_capture_id();
        let candidate_connector = self.candidate_selections.get(&port_id).copied().flatten();
        let port = self
            .ports
            .get(&port_id)
            .ok_or_else(|| anyhow!("CONTENT_CAPTURE_FAILED: ContentPort {port_id} is unavailable"))?
            .lock()
            .map_err(|_| {
                anyhow!("CONTENT_CAPTURE_FAILED: ContentPort {port_id} lock is poisoned")
            })?;
        let confirmed_connector = port.visible_connector;
        let desired_connector = port.desired_connector;
        // `measure_content` records a failed candidate and clears its
        // selection when no confirmed product exists. Retain that failed
        // connector in the capture so the host can publish its structured
        // projection diagnostic after preparation instead of replacing the
        // root cause with a generic capture error.
        let failed_desired_connector = if let Some(connector_id) = desired_connector {
            let connector = self.connectors.get(&connector_id).ok_or_else(|| {
                anyhow!("CONTENT_CAPTURE_FAILED: requested Connector {connector_id} is unavailable")
            })?;
            let state = connector.lock().map_err(|_| {
                anyhow!(
                    "CONTENT_CAPTURE_FAILED: requested Connector {connector_id} lock is poisoned"
                )
            })?;
            (!state.visible && state.error.is_some()).then_some(connector_id)
        } else {
            None
        };
        let candidate_connector = candidate_connector.or(failed_desired_connector);
        let candidate_product = measurement
            .connector_id
            .and_then(|connector| self.projection_for_measurement(connector, measurement));
        let history_adjustment = self.history_measurement_adjustment(
            port_id,
            offered_width,
            &measurement,
            candidate_product.as_deref(),
        );
        let confirmed_product =
            confirmed_connector.and_then(|connector| self.confirmed_projection(connector));
        let source_snapshot = if let Some(product) = candidate_product.as_ref() {
            Some(product.source_snapshot.clone())
        } else if let Some(connector) = candidate_connector {
            let source = self.connector_source(connector).ok_or_else(|| {
                anyhow!("INTERNAL_INVARIANT: candidate Connector source is missing")
            })?;
            Some(self.source_snapshot_for(&source)?)
        } else {
            None
        };
        let candidate = match (candidate_connector, source_snapshot, candidate_product) {
            (None, None, None) => CapturedCandidate::None,
            (Some(connector_id), Some(source_snapshot), Some(product)) => {
                CapturedCandidate::Prepared(CapturedProjection {
                    connector_id,
                    source_snapshot,
                    product,
                })
            }
            (Some(connector_id), Some(source_snapshot), None) => CapturedCandidate::Failed {
                connector_id,
                source_snapshot,
            },
            _ => {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: candidate content capture state is incomplete"
                ));
            }
        };
        let confirmed = match (confirmed_connector, confirmed_product) {
            (None, None) => None,
            (Some(connector_id), Some(product)) => Some(CapturedProjection {
                connector_id,
                source_snapshot: product.source_snapshot.clone(),
                product,
            }),
            _ => {
                return Err(anyhow!(
                    "INTERNAL_INVARIANT: confirmed content product is unavailable"
                ));
            }
        };
        let candidate_failure_without_error = match &candidate {
            CapturedCandidate::Failed { connector_id, .. } if confirmed.is_none() => {
                let connector = self.connectors.get(connector_id).ok_or_else(|| {
                    anyhow!(
                        "INTERNAL_INVARIANT: failed candidate Connector {connector_id} disappeared"
                    )
                })?;
                !(measurement.connector_id.is_some() && !measurement.physically_complete)
                    && !self.pending_content_projections.contains_key(connector_id)
                    && !connector
                        .lock()
                        .map_err(|_| anyhow!("CONTENT_CAPTURE_FAILED: Connector lock is poisoned"))?
                        .error
                        .is_some()
            }
            _ => false,
        };
        if let CapturedCandidate::Failed {
            connector_id,
            source_snapshot,
        } = &candidate
            && candidate_failure_without_error
        {
            return Err(anyhow!(
                "CONTENT_CAPTURE_FAILED: Connector {connector_id} projection failed at Source revision {} without a confirmed product",
                source_snapshot.revision,
            ));
        }
        self.candidate_content_captures.insert(
            capture_id,
            CandidateContentCapture {
                port_id,
                candidate: candidate.clone(),
                confirmed: confirmed.clone(),
            },
        );
        let (min_content, max_content) = self.content_measurement_bounds(
            offered_width,
            measurement,
            match &candidate {
                CapturedCandidate::Prepared(capture) => Some(&capture.product),
                CapturedCandidate::None | CapturedCandidate::Failed { .. } => {
                    confirmed.as_ref().map(|capture| &capture.product)
                }
            },
        );
        let (semantic_contents, terminal_policy, terminal_product) = match &candidate {
            CapturedCandidate::Prepared(capture) => Some(&capture.product),
            CapturedCandidate::None | CapturedCandidate::Failed { .. } => {
                confirmed.as_ref().map(|capture| &capture.product)
            }
        }
        .map(|product| {
            (
                Some(Arc::clone(&product.semantic_contents)),
                product.terminal_policy.clone(),
                Some(Arc::clone(&product.product)),
            )
        })
        .unwrap_or_else(|| (None, content_text_policy(TextWrapMode::Word), None));
        Ok(ContentMeasurementCapture {
            capture_id,
            min_content,
            max_content,
            history_adjustment,
            semantic_contents,
            terminal_policy,
            terminal_product,
            measurement,
        })
    }

    fn refine_captured_measurement(
        &mut self,
        port_id: u64,
        capture_id: u64,
        offered_width: u16,
        _width_rule: crate::presentation::ContentWidthRule,
    ) -> anyhow::Result<ContentMeasurementCapture> {
        let Some(capture) = self.candidate_content_captures.get(&capture_id).cloned() else {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: direct content capture is unavailable"
            ));
        };
        if capture.port_id != port_id {
            return Err(anyhow!(
                "INTERNAL_INVARIANT: direct content capture targets another Port"
            ));
        }
        let candidate = match capture.candidate {
            CapturedCandidate::None => None,
            CapturedCandidate::Failed {
                connector_id,
                source_snapshot,
            } if capture.confirmed.is_none() => {
                let connector = self.connectors.get(&connector_id).ok_or_else(|| {
                    anyhow!(
                        "INTERNAL_INVARIANT: failed candidate Connector {connector_id} disappeared"
                    )
                })?;
                let has_error = connector
                    .lock()
                    .map_err(|_| anyhow!("CONTENT_CAPTURE_FAILED: Connector lock is poisoned"))?
                    .error
                    .is_some();
                if !has_error && !self.pending_content_projections.contains_key(&connector_id) {
                    return Err(anyhow!(
                        "CONTENT_CAPTURE_FAILED: Connector {connector_id} projection failed at Source revision {} without a confirmed product",
                        source_snapshot.revision,
                    ));
                }
                None
            }
            CapturedCandidate::Failed { .. } => None,
            CapturedCandidate::Prepared(binding) => {
                match self.prepare_connector_projection_async(
                    binding.connector_id,
                    offered_width,
                    Some(&binding.source_snapshot),
                ) {
                    Ok(measurement) => {
                        let product = measurement.connector_id.and_then(|connector| {
                            self.projection_for_measurement(connector, measurement)
                        });
                        if let Some(product) = product {
                            Some((binding.connector_id, measurement, product))
                        } else if self
                            .pending_content_projections
                            .contains_key(&binding.connector_id)
                        {
                            // Keep the confirmed/compatible capture's
                            // actual old-width metrics while the requested
                            // realization is pending. The layout callback
                            // must not manufacture a new height.
                            let mut measurement = binding.product.measurement(binding.connector_id);
                            measurement.physically_complete = false;
                            Some((binding.connector_id, measurement, binding.product))
                        } else {
                            return Err(anyhow!(
                                "INTERNAL_INVARIANT: prepared candidate product disappeared"
                            ));
                        }
                    }
                    Err(error) => {
                        let key = self.connector_projection_key_for_snapshot(
                            binding.connector_id,
                            offered_width,
                            &binding.source_snapshot,
                        )?;
                        self.record_projection_failure(binding.connector_id, key, &error);
                        None
                    }
                }
            }
        };
        let (selected_connector, measurement, product) =
            if let Some((connector, measurement, product)) = candidate {
                (Some(connector), measurement, Some(product))
            } else if let Some(binding) = capture.confirmed {
                match self.prepare_connector_projection_async(
                    binding.connector_id,
                    offered_width,
                    Some(&binding.source_snapshot),
                ) {
                    Ok(measurement) => {
                        if let Some(product) = measurement.connector_id.and_then(|connector| {
                            self.projection_for_measurement(connector, measurement)
                        }) {
                            (Some(binding.connector_id), measurement, Some(product))
                        } else {
                            // A new width is a product miss, not permission
                            // to invent old-width geometry. Keep confirmed A
                            // and its actual metrics/clip while the executor
                            // prepares the requested realization.
                            (
                                Some(binding.connector_id),
                                binding.product.measurement(binding.connector_id),
                                Some(binding.product),
                            )
                        }
                    }
                    Err(error) => {
                        let key = self.connector_projection_key_for_snapshot(
                            binding.connector_id,
                            offered_width,
                            &binding.source_snapshot,
                        )?;
                        self.record_projection_failure(binding.connector_id, key, &error);
                        (
                            Some(binding.connector_id),
                            binding.product.measurement(binding.connector_id),
                            Some(binding.product),
                        )
                    }
                }
            } else {
                (None, ContentMeasurement::default(), None)
            };
        let measurement = self.adjust_history_measurement(port_id, measurement);
        self.candidate_selections
            .insert(port_id, selected_connector);
        let (min_content, max_content) =
            self.content_measurement_bounds(offered_width, measurement, product.as_ref());
        let (semantic_contents, terminal_policy, terminal_product) = product
            .as_ref()
            .map(|product| {
                (
                    Some(Arc::clone(&product.semantic_contents)),
                    product.terminal_policy.clone(),
                    Some(Arc::clone(&product.product)),
                )
            })
            .unwrap_or_else(|| (None, content_text_policy(TextWrapMode::Word), None));
        let history_adjustment = self.history_measurement_adjustment(
            port_id,
            offered_width,
            &measurement,
            product.as_deref(),
        );
        Ok(ContentMeasurementCapture {
            capture_id,
            min_content,
            max_content,
            history_adjustment,
            semantic_contents,
            terminal_policy,
            terminal_product,
            measurement,
        })
    }

    fn paint_window(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (u16, u16),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        self.paint_window_direct(
            ticket,
            window,
            target,
            (i32::from(target_origin.0), i32::from(target_origin.1)),
            clip,
            style,
        );
    }

    fn paint_window_signed(
        &self,
        ticket: PreparedProjectionTicket,
        window: ContentWindow,
        target: &mut Surface,
        target_origin: (i32, i32),
        clip: crate::geometry::Rect,
        style: crate::physical::PhysicalStyle,
    ) {
        self.paint_window_direct(ticket, window, target, target_origin, clip, style);
    }

    fn history_rows(&self, port_id: u64, offered_width: u16) -> Option<HistoryContentRows> {
        self.history_rows(port_id, offered_width)
    }

    fn history_rows_committed(
        &mut self,
        port_id: u64,
        rows: usize,
        content_rows: usize,
        leading_padding: usize,
        trailing_padding: usize,
    ) {
        self.history_rows_committed(
            port_id,
            rows,
            content_rows,
            leading_padding,
            trailing_padding,
        );
    }

    fn history_unit_retired(&mut self, unit_id: u64) {
        self.history_unit_retired(unit_id);
    }

    fn history_transfer_blocked(&self, port_id: u64, offered_width: u16) -> bool {
        let Some(port) = self.ports.get(&port_id) else {
            return true;
        };
        let Some(state) = port.lock().ok() else {
            return true;
        };
        if self.history_adapter.unit_id(port_id).is_none() {
            return false;
        }
        if state
            .visible_connector
            .or(state.desired_connector)
            .is_none()
        {
            return true;
        }
        drop(state);
        let rows = self.history_rows(port_id, offered_width);
        rows.is_none_or(|rows| rows.rows.is_empty() && !rows.complete)
    }
}

impl ContentHostRegistry {
    fn connector_source(&self, connector_id: u64) -> Option<HostContentSource> {
        self.connectors
            .get(&connector_id)
            .and_then(|connector| connector.lock().ok())
            .map(|connector| connector.source.clone())
    }

    fn projection_for_measurement(
        &self,
        connector_id: u64,
        measurement: ContentMeasurement,
    ) -> Option<Arc<HostContentProjection>> {
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        let matches = |projection: &Arc<HostContentProjection>| {
            projection.identity == measurement.projection_identity
        };
        connector
            .candidate_projection
            .as_ref()
            .filter(|projection| matches(projection))
            .cloned()
            .or_else(|| {
                connector
                    .committed_projection
                    .as_ref()
                    .filter(|projection| matches(projection))
                    .cloned()
            })
            .or_else(|| {
                connector
                    .projection_cache
                    .iter()
                    .find(|(_, projection)| matches(projection))
                    .map(|(_, projection)| Arc::clone(projection))
            })
    }

    fn confirmed_projection(&self, connector_id: u64) -> Option<Arc<HostContentProjection>> {
        let connector = self.connectors.get(&connector_id)?.lock().ok()?;
        connector.committed_projection.as_ref().cloned()
    }

    fn content_measurement_bounds(
        &self,
        _offered_width: u16,
        measurement: ContentMeasurement,
        product: Option<&Arc<HostContentProjection>>,
    ) -> (Size, Size) {
        product.map_or(
            (measurement.intrinsic_size, measurement.intrinsic_size),
            |projection| (projection.min_content, projection.max_content),
        )
    }
}

impl Drop for ContentHostRegistry {
    fn drop(&mut self) {
        // HostInner normally calls dispose_all first. This final owner guard
        // also covers direct registry tests and releases the waiter callback
        // even when an admitted command is still finishing on the executor.
        for pending in self.pending_content_projections.values() {
            pending.cancelled.store(true, Ordering::Release);
        }
        self.pending_content_projections.clear();
        if let Some(waiter_id) = self.deferred_projection_waiter.take() {
            self.executor.unregister_waiter(waiter_id);
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostContentPort {
    id: u64,
    generation: u32,
    family: ContentFamily,
    record: Arc<Mutex<PortRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentPort {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn family(&self) -> ContentFamily {
        self.family
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_port(self.id(), ContentDirtyReason::SelectionLifecycle);
        let needs_frame = inner.content.deactivate_port(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn connect(
        &self,
        source: &HostContentSource,
        funnel: HostContentFunnel,
    ) -> Result<HostContentConnector> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        inner
            .content
            .connect(&self.record, source, funnel)
            .map(|mut connector| {
                connector.host = host_weak;
                connector
            })
    }

    pub fn dispose(&self) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .dispose_port(&self.record)
    }

    pub fn is_mounted(&self) -> Result<bool> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: ContentPort host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .port_status(self.id)
    }
}

#[derive(Clone, Debug)]
pub struct HostContentConnector {
    id: u64,
    generation: u32,
    source_id: u64,
    record: Arc<Mutex<ConnectorRecord>>,
    host: Weak<Mutex<HostInner>>,
}

impl HostContentConnector {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    #[must_use]
    pub fn source_id(&self) -> u64 {
        self.source_id
    }

    pub fn activate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let host_weak = Arc::downgrade(&host);
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner
            .content
            .request_connector_activation(self.id(), &host_weak)?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn deactivate(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner.content.request_connector_deactivation(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    pub fn dispose(&self) -> Result<WakeDisposition> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        let mut inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        let dirty = inner
            .content
            .dirty_for_connector(self.id(), ContentDirtyReason::SelectionLifecycle)?;
        let needs_frame = inner.content.request_connector_dispose(self.id())?;
        if needs_frame {
            return inner.mark_content_pending(dirty);
        }
        Ok(WakeDisposition::default())
    }

    /// Injects one deterministic operational failure for a native/unit
    /// fixture. It is not part of the TypeScript content API; real projection
    /// failures use the same candidate-rollback state.
    pub fn fail_next_activation(&self, diagnostic: String) -> Result<()> {
        let host = self
            .host
            .upgrade()
            .ok_or_else(|| anyhow!("HOST_DISPOSED: Connector host is gone"))?;
        host.lock()
            .map_err(|_| anyhow!("host lock is poisoned"))?
            .content
            .fail_next_activation(self.id(), diagnostic)
    }

    pub fn status(&self) -> Result<ContentConnectorStatus> {
        if let Some(host) = self.host.upgrade() {
            let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
            return match inner.content.connector_status(self.id) {
                Ok(status) => Ok(status),
                Err(error) => {
                    // A disposed Connector is removed from the live registry
                    // but detached handles retain its final record. Read that
                    // record only while the owning Host lock is held so live
                    // status never races an in-flight commit.
                    let state = self
                        .record
                        .lock()
                        .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                    if state.lifecycle == ConnectorLifecycle::Disposed {
                        Ok(ContentConnectorStatus {
                            phase: "disposed".to_owned(),
                            requested: state.requested,
                            visible: state.visible,
                            projected_source_revision: state.projected_source_revision,
                            error: state.error.clone(),
                            cleanup_pending: state.cleanup_error.is_some(),
                            cleanup_error: state.cleanup_error.as_deref().cloned(),
                        })
                    } else {
                        Err(error)
                    }
                }
            };
        }
        // HostInner::drop() marks retained Connector records disposed before
        // its weak owner disappears. A live record with no owner is an
        // invariant failure, not a reason to fabricate a status.
        self.record_status()
    }

    pub fn visible_delivery_frontier(&self) -> Result<StreamOffset> {
        let Some(host) = self.host.upgrade() else {
            return self.record_frontier(false);
        };
        let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        match inner.content.connector_delivery_frontier(self.id) {
            Ok(frontier) => Ok(frontier),
            Err(error) => {
                let state = self
                    .record
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                (state.lifecycle == ConnectorLifecycle::Disposed)
                    .then_some(state.committed_delivery_frontier)
                    .ok_or(error)
            }
        }
    }

    pub fn candidate_delivery_frontier(&self) -> Result<StreamOffset> {
        let Some(host) = self.host.upgrade() else {
            return self.record_frontier(true);
        };
        let inner = host.lock().map_err(|_| anyhow!("host lock is poisoned"))?;
        match inner.content.connector_candidate_delivery_frontier(self.id) {
            Ok(frontier) => Ok(frontier),
            Err(error) => {
                let state = self
                    .record
                    .lock()
                    .map_err(|_| anyhow!("Connector lock is poisoned"))?;
                (state.lifecycle == ConnectorLifecycle::Disposed)
                    .then_some(state.candidate_delivery_frontier)
                    .ok_or(error)
            }
        }
    }

    fn record_frontier(&self, candidate: bool) -> Result<StreamOffset> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        if candidate {
            Ok(state.candidate_delivery_frontier)
        } else {
            Ok(state.committed_delivery_frontier)
        }
    }

    /// Final detached-handle readback is valid only after the owning HostInner
    /// has gone away; while the host is live, `status` routes through its
    /// serialized registry owner.
    fn record_status(&self) -> Result<ContentConnectorStatus> {
        let state = self
            .record
            .lock()
            .map_err(|_| anyhow!("Connector lock is poisoned"))?;
        Ok(ContentConnectorStatus {
            phase: if state.lifecycle == ConnectorLifecycle::Disposed {
                "disposed".to_owned()
            } else {
                state.phase.to_owned()
            },
            requested: state.requested,
            visible: state.visible,
            projected_source_revision: state.projected_source_revision,
            error: state.error.clone(),
            cleanup_pending: state.cleanup_error.is_some(),
            cleanup_error: state.cleanup_error.as_deref().cloned(),
        })
    }

    #[must_use]
    pub fn is_disposed(&self) -> bool {
        if let Some(host) = self.host.upgrade() {
            return host
                .lock()
                .ok()
                .is_none_or(|inner| inner.content.connector_is_disposed(self.id));
        }
        self.record
            .lock()
            .is_ok_and(|state| state.lifecycle == ConnectorLifecycle::Disposed)
    }
}

impl Drop for HostContentConnector {
    fn drop(&mut self) {
        // Explicit disposal owns semantic release. A dropped wrapper is not a
        // hidden lifecycle operation; host teardown calls dispose_all instead.
    }
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
