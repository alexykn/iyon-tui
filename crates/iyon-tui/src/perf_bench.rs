//! Deterministic, opt-in performance oracle for the TUI refactor.
//!
//! The executable target prints JSONL to stdout. It deliberately keeps the
//! fixtures are framework-generic and contain no product-specific data. Result
//! files are not written by the benchmark and should not be committed.

use std::{process::Command, time::Instant};

use crate::{
    Component, History, IntoView, TextSpan, Theme, View,
    component::{ComponentRegistry, MountGraph},
    geometry::{LayoutConstraints, Size},
    history::{HistoryViewportAnchor, project_into_session_for_host},
    perf::{self, PerfSnapshot},
    presentation::{
        layout::{self, LayoutCache},
        paint::{PaintCache, ViewPainter},
    },
    scene::{ResolveSession, layout_resolved_scene, layout_resolved_scene_with_cache},
};

const VIEW_SIZES: [(&str, usize); 4] = [
    ("small_view", 20),
    ("medium_view", 200),
    ("large_view", 2_000),
    ("huge_view", 10_000),
];

#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
enum Workload {
    TextHeavy,
    ColumnHeavy,
    RowHeavy,
    GridHeavy,
    StyledSpanHeavy,
    ComponentHeavy,
}

impl Workload {
    const ALL: [Self; 6] = [
        Self::TextHeavy,
        Self::ColumnHeavy,
        Self::RowHeavy,
        Self::GridHeavy,
        Self::StyledSpanHeavy,
        Self::ComponentHeavy,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::TextHeavy => "text_heavy",
            Self::ColumnHeavy => "column_heavy",
            Self::RowHeavy => "row_heavy",
            Self::GridHeavy => "grid_heavy",
            Self::StyledSpanHeavy => "styled_span_heavy",
            Self::ComponentHeavy => "component_heavy",
        }
    }
}

struct ViewFixture {
    view: View,
    registry: Option<ComponentRegistry>,
}

struct RenderTiming {
    height: usize,
    paint_ns: u128,
}

struct PerfComponent {
    body: View,
}

impl Component for PerfComponent {
    fn view(&self) -> View {
        self.body.clone()
    }
}

fn leaf(workload: Workload, index: usize) -> View {
    match workload {
        Workload::StyledSpanHeavy => View::styled_text([
            TextSpan::plain(format!("span-{index} ")),
            TextSpan::plain("stable "),
            TextSpan::plain("text "),
            TextSpan::plain("payload"),
        ])
        .into_view(),
        Workload::TextHeavy => {
            View::text(format!("text-{index}: deterministic payload\n")).into_view()
        }
        _ => View::text(format!("node-{index}")).into_view(),
    }
}

fn build_fixture(workload: Workload, nodes: usize) -> ViewFixture {
    let leaves = nodes.saturating_sub(1).max(1);
    match workload {
        Workload::ComponentHeavy => {
            let mut registry = ComponentRegistry::new();
            let mut handles = Vec::with_capacity(leaves);
            for index in 0..leaves {
                handles.push(registry.register(PerfComponent {
                    body: View::text(format!("component-{index}")).into_view(),
                }));
            }
            let view = View::vertical(|column| {
                for handle in handles {
                    column.child(View::component(handle));
                }
            });
            ViewFixture {
                view,
                registry: Some(registry),
            }
        }
        Workload::GridHeavy => {
            let side = (leaves as f64).sqrt().ceil() as usize;
            let view = View::grid(|grid| {
                grid.columns((0..side).map(|_| crate::GridTrack::content()));
                for row in 0..side {
                    grid.row(|cells| {
                        for column in 0..side {
                            let index = row * side + column;
                            if index < leaves {
                                cells.cell(leaf(workload, index));
                            }
                        }
                    });
                }
            });
            ViewFixture {
                view,
                registry: None,
            }
        }
        Workload::RowHeavy => {
            let view = View::horizontal(|row| {
                for index in 0..leaves {
                    row.child(leaf(workload, index));
                }
            });
            ViewFixture {
                view,
                registry: None,
            }
        }
        Workload::ColumnHeavy | Workload::TextHeavy | Workload::StyledSpanHeavy => {
            let view = View::vertical(|column| {
                for index in 0..leaves {
                    column.child(leaf(workload, index));
                }
            });
            ViewFixture {
                view,
                registry: None,
            }
        }
    }
}

fn render_view_timed(
    view: &View,
    registry: Option<&ComponentRegistry>,
    width: u16,
    height: u16,
    cache: &mut LayoutCache,
    paint_cache: Option<&mut PaintCache>,
) -> RenderTiming {
    let (tree, graph) = if let Some(registry) = registry {
        let mut session = ResolveSession::new(registry);
        let resolved = session
            .resolve_root(view)
            .expect("deterministic component fixture must resolve");
        let scene = session.finish(resolved);
        let geometry = layout_resolved_scene_with_cache(&scene, Size::new(width, height), cache);
        (geometry.tree, scene.mounts.clone())
    } else {
        let tree = layout::layout_view_with_overlay_and_cache(
            view,
            LayoutConstraints::width_only(width),
            &crate::scene::ResolutionOverlay::default(),
            cache,
        );
        (tree, MountGraph::default())
    };
    let compiler = crate::presentation::layout::ViewCompiler::with_interaction(
        &Theme::default(),
        None,
        &graph,
    );

    let paint_start = Instant::now();
    let surface = match paint_cache {
        Some(cache) => ViewPainter.paint_tree_with_cache(&compiler, &tree, cache),
        None => ViewPainter.paint_tree(&compiler, &tree),
    };
    RenderTiming {
        height: usize::from(surface.height()),
        paint_ns: paint_start.elapsed().as_nanos(),
    }
}

fn render_view(
    view: &View,
    registry: Option<&ComponentRegistry>,
    width: u16,
    height: u16,
    cache: &mut LayoutCache,
) -> usize {
    render_view_timed(view, registry, width, height, cache, None).height
}

fn iterations_for(nodes: usize) -> usize {
    if let Ok(value) = std::env::var("PERF_ITERATIONS") {
        return value.parse().expect("PERF_ITERATIONS must be an integer");
    }
    match nodes {
        0..=20 => 100,
        21..=200 => 50,
        201..=2_000 => 20,
        _ => 5,
    }
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len().saturating_sub(1)) * percentile).div_ceil(100);
    sorted[index]
}

fn git_sha() -> String {
    if let Ok(value) = std::env::var("GIT_SHA") {
        return value;
    }
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn print_record(
    benchmark: &str,
    implementation: &str,
    node_count: usize,
    source_bytes: usize,
    iterations: usize,
    samples: &[u128],
    counters: PerfSnapshot,
    sha: &str,
) {
    let counters_json = counters
        .iter()
        .map(|(name, value)| format!("\"{name}\":{value}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"benchmark\":\"{benchmark}\",\"implementation\":\"{implementation}\",\"node_count\":{node_count},\"source_bytes\":{source_bytes},\"iterations\":{iterations},\"median_ns\":{},\"p95_ns\":{},\"p99_ns\":{},\"counters\":{{{counters_json}}},\"git_sha\":\"{sha}\"}}",
        percentile(samples, 50),
        percentile(samples, 95),
        percentile(samples, 99),
    );
}

fn run_view_clone_case(sha: &str) {
    let iterations = std::env::var("PERF_CLONE_ITERATIONS")
        .ok()
        .map(|value| {
            value
                .parse()
                .expect("PERF_CLONE_ITERATIONS must be an integer")
        })
        .unwrap_or(100);

    for nodes in [100, 10_000] {
        let fixture = build_fixture(Workload::ColumnHeavy, nodes);
        let mut samples = Vec::with_capacity(iterations);
        perf::reset();
        for _ in 0..iterations {
            let start = Instant::now();
            std::hint::black_box(fixture.view.clone());
            samples.push(start.elapsed().as_nanos());
        }
        print_record(
            &format!("view_clone/{nodes}"),
            "persistent",
            nodes,
            0,
            iterations,
            &samples,
            perf::snapshot(),
            sha,
        );
    }
}

fn run_view_case(
    size_name: &str,
    workload: Workload,
    nodes: usize,
    pattern: &'static str,
    sha: &str,
) {
    let iterations = iterations_for(nodes);
    let base = build_fixture(workload, nodes);
    let shared = base.view.clone();
    let mut cache = LayoutCache::default();
    let mut samples = Vec::with_capacity(iterations);
    perf::reset();
    for index in 0..iterations {
        cache.begin_epoch();
        let start = Instant::now();
        match pattern {
            "COLD" => {
                let fixture = build_fixture(workload, nodes);
                std::hint::black_box(render_view(
                    &fixture.view,
                    fixture.registry.as_ref(),
                    80,
                    24,
                    &mut cache,
                ));
            }
            "IDENTICAL_IDENTITY" => {
                std::hint::black_box(render_view(
                    &base.view,
                    base.registry.as_ref(),
                    80,
                    24,
                    &mut cache,
                ));
            }
            "SHARED_PATH" => {
                let changed = View::text(format!("changed-{index}"));
                let view = View::vertical(|column| {
                    column.child(shared.clone());
                    column.child(changed);
                });
                std::hint::black_box(render_view(
                    &view,
                    base.registry.as_ref(),
                    80,
                    24,
                    &mut cache,
                ));
            }
            "REBUILT_EQUIVALENT" => {
                let fixture = build_fixture(workload, nodes);
                std::hint::black_box(render_view(
                    &fixture.view,
                    fixture.registry.as_ref(),
                    80,
                    24,
                    &mut cache,
                ));
            }
            _ => unreachable!("unknown View benchmark pattern"),
        }
        samples.push(start.elapsed().as_nanos());
    }
    print_record(
        &format!("view/{size_name}/{}/{pattern}", workload.name()),
        "baseline",
        nodes,
        0,
        iterations,
        &samples,
        perf::snapshot(),
        sha,
    );
}

fn render_history(history: &History, registry: &ComponentRegistry) -> usize {
    let mut session = ResolveSession::new(registry);
    let projection = project_into_session_for_host(
        history,
        Size::new(80, 24),
        &mut session,
        HistoryViewportAnchor::FollowEnd,
    )
    .expect("deterministic History fixture must project");
    let scene = session.finish(projection.view);
    let geometry = layout_resolved_scene(&scene, Size::new(80, 24));
    let compiler = crate::presentation::layout::ViewCompiler::with_interaction(
        &Theme::default(),
        None,
        &scene.mounts,
    );
    usize::from(ViewPainter.paint_tree(&compiler, &geometry.tree).height())
}

fn run_paint_gate_case(workload: Workload, nodes: usize, sha: &str) {
    let iterations = std::env::var("PERF_PAINT_ITERATIONS")
        .ok()
        .map(|value| {
            value
                .parse()
                .expect("PERF_PAINT_ITERATIONS must be an integer")
        })
        .unwrap_or(30);
    let base = build_fixture(workload, nodes);
    let shared = base.view.clone();
    let mut cache = LayoutCache::default();
    let mut paint_cache = PaintCache::default();
    paint_cache.begin_epoch(&Theme::default());

    for index in 0..3 {
        cache.begin_epoch();
        let view = View::vertical(|column| {
            column.child(shared.clone());
            column.child(View::text(format!("warmup-{index}")));
        });
        let _ = render_view_timed(
            &view,
            base.registry.as_ref(),
            80,
            24,
            &mut cache,
            Some(&mut paint_cache),
        );
    }

    let mut total_samples = Vec::with_capacity(iterations);
    let mut paint_samples = Vec::with_capacity(iterations);
    perf::reset();
    for index in 0..iterations {
        cache.begin_epoch();
        let view = View::vertical(|column| {
            column.child(shared.clone());
            column.child(View::text(format!("changed-{index}")));
        });
        let started = Instant::now();
        paint_cache.begin_epoch(&Theme::default());
        let timing = render_view_timed(
            &view,
            base.registry.as_ref(),
            80,
            24,
            &mut cache,
            Some(&mut paint_cache),
        );
        total_samples.push(started.elapsed().as_nanos());
        paint_samples.push(timing.paint_ns);
    }

    let total_p95 = percentile(&total_samples, 95);
    let paint_p95 = percentile(&paint_samples, 95);
    let paint_share = if total_p95 == 0 {
        0.0
    } else {
        paint_p95 as f64 / total_p95 as f64
    };
    let counters_json = perf::snapshot()
        .iter()
        .map(|(name, value)| format!("\"{name}\":{value}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"benchmark\":\"paint_gate/{}/{nodes}/SHARED_PATH\",\"implementation\":\"after_perf9\",\"node_count\":{nodes},\"source_bytes\":0,\"iterations\":{iterations},\"dirty_frame_median_ns\":{},\"dirty_frame_p95_ns\":{total_p95},\"paint_median_ns\":{},\"paint_p95_ns\":{paint_p95},\"paint_p95_share\":{paint_share:.6},\"counters\":{{{counters_json}}},\"git_sha\":\"{sha}\"}}",
        workload.name(),
        percentile(&total_samples, 50),
        percentile(&paint_samples, 50),
    );
}

fn run_paint_gate(sha: &str) {
    for (workload, nodes) in [
        (Workload::TextHeavy, 2_000),
        (Workload::TextHeavy, 10_000),
        (Workload::ColumnHeavy, 2_000),
        (Workload::ColumnHeavy, 10_000),
        (Workload::StyledSpanHeavy, 2_000),
        (Workload::StyledSpanHeavy, 10_000),
    ] {
        run_paint_gate_case(workload, nodes, sha);
    }
}

fn run_history_case(sha: &str) {
    let iterations = std::env::var("PERF_HISTORY_ITERATIONS")
        .ok()
        .map(|value| {
            value
                .parse()
                .expect("PERF_HISTORY_ITERATIONS must be an integer")
        })
        .unwrap_or(100);

    let mut static_history = History::new();
    for index in 0..1_000 {
        static_history
            .push(View::text(format!("static-{index}")))
            .expect("static fixture append");
    }
    let static_registry = ComponentRegistry::new();
    let _ = render_history(&static_history, &static_registry);
    let mut static_samples = Vec::with_capacity(iterations);
    perf::reset();
    for _ in 0..iterations {
        let start = Instant::now();
        std::hint::black_box(render_history(&static_history, &static_registry));
        static_samples.push(start.elapsed().as_nanos());
    }
    print_record(
        "history_static_1000",
        "baseline",
        1_000,
        0,
        iterations,
        &static_samples,
        perf::snapshot(),
        sha,
    );

    let mut registry = ComponentRegistry::new();
    let handle = registry.register(PerfComponent {
        body: View::text("live tail").into_view(),
    });
    let mut history = History::new();
    for index in 0..1_000 {
        history
            .push(View::text(format!("static-{index}")))
            .expect("static fixture append");
    }
    history
        .push(View::component(handle))
        .expect("live fixture append");
    let _ = render_history(&history, &registry);

    let mut samples = Vec::with_capacity(iterations);
    perf::reset();
    for _ in 0..iterations {
        registry.with_any_mut(handle.id(), |_| {});
        let start = Instant::now();
        std::hint::black_box(render_history(&history, &registry));
        samples.push(start.elapsed().as_nanos());
    }
    print_record(
        "history_live_tail",
        "baseline",
        1_001,
        0,
        iterations,
        &samples,
        perf::snapshot(),
        sha,
    );
}

/// Runs the complete first-tranche oracle and emits one JSON object per line.
pub fn run() {
    let sha = git_sha();
    if std::env::var_os("PERF_ONLY_PAINT_GATE").is_some() {
        run_paint_gate(&sha);
        return;
    }
    run_view_clone_case(&sha);
    for workload in Workload::ALL {
        for (size_name, node_count) in VIEW_SIZES {
            for pattern in [
                "COLD",
                "IDENTICAL_IDENTITY",
                "SHARED_PATH",
                "REBUILT_EQUIVALENT",
            ] {
                run_view_case(size_name, workload, node_count, pattern, &sha);
            }
        }
    }
    run_history_case(&sha);
}
