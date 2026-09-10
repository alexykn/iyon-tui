//! One-way M1 occurrence-to-renderer adapter.
//!
//! React and the occurrence document own identity, topology, properties and
//! resource bindings.  This module only derives the input accepted by the
//! existing terminal renderer.  It has no mutation API and must be deleted at
//! the T7 renderer cutover (the explicit deletion gate for this adapter).

use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};

use crate::history::{FlowBoundary, HistoryUnitId};
use crate::occurrence::{
    AlignmentAxis, HostKind, LayerValue, LayoutMode, NodeKey, OccurrenceSnapshot, PropertyId,
    PropertyValue, ResourceKey, SizeMode, UiChangeSet,
};
use crate::presentation::factory as vf;
use crate::presentation::ir::ViewId;
use crate::presentation::{BorderSpec, BorderStyle, StyleRef, StyleSpec, TextAttribute, View};

use super::content::ContentHostRegistry;
use super::ui_resources::{HistoryUnitStatus, UiResourceOwner};

#[derive(Default)]
pub(crate) struct LegacySceneAdapter {
    recipes: HashMap<NodeKey, View>,
    view_ids: HashMap<NodeKey, ViewId>,
    control_components: HashMap<ResourceKey, u64>,
    #[cfg(test)]
    recipe_builds: usize,
    #[cfg(test)]
    recipe_cache_hits: usize,
}

pub(crate) struct HistoryUnitRecipe {
    pub(crate) root: NodeKey,
    pub(crate) view: View,
    pub(crate) unit_identity: HistoryUnitId,
    pub(crate) status: HistoryUnitStatus,
    pub(crate) flow_boundary: FlowBoundary,
}

impl LegacySceneAdapter {
    pub(crate) fn set_control_component(&mut self, key: ResourceKey, component_id: u64) {
        self.control_components.insert(key, component_id);
    }

    pub(crate) fn remove_control_component(&mut self, key: ResourceKey) {
        self.control_components.remove(&key);
    }

    pub(crate) fn control_for_component(&self, component_id: u64) -> Option<ResourceKey> {
        self.control_components
            .iter()
            .find_map(|(control, candidate)| (*candidate == component_id).then_some(*control))
    }

    pub(crate) fn occurrence_geometry(
        &self,
        view_geometry: &HashMap<ViewId, crate::presentation::layout::ComponentGeometry>,
    ) -> HashMap<NodeKey, crate::presentation::layout::ComponentGeometry> {
        self.view_ids
            .iter()
            .filter_map(|(node, view)| {
                view_geometry
                    .get(view)
                    .copied()
                    .map(|geometry| (*node, geometry))
            })
            .collect()
    }

    pub(crate) fn children_for(&self, snapshot: &OccurrenceSnapshot) -> Result<Vec<View>> {
        snapshot
            .children
            .iter()
            .map(|key| {
                self.recipes
                    .get(key)
                    .cloned()
                    .ok_or_else(|| anyhow!("child occurrence recipe is missing for {key:?}"))
            })
            .collect()
    }
    pub(crate) fn synchronize(
        &mut self,
        owner: &UiResourceOwner,
        content: &mut ContentHostRegistry,
        changes: Option<&UiChangeSet>,
    ) -> Result<()> {
        let initial = self.recipes.is_empty();
        content.sync_ui_resources(owner, changes)?;
        let body = owner
            .document
            .as_ref()
            .ok_or_else(|| anyhow!("UI resource owner has no occurrence document"))?
            .body_root();
        let roots = std::iter::once(body)
            .chain(owner.history_roots())
            .chain(
                owner
                    .document
                    .as_ref()
                    .expect("open UI resource owner retains document")
                    .portal_roots(),
            )
            .collect::<Vec<_>>();

        let mut dirty = HashSet::new();
        if let Some(changes) = changes {
            for key in changes.changed_nodes.iter().copied() {
                let mut cursor = Some(key);
                while let Some(current) = cursor {
                    if !dirty.insert(current) {
                        break;
                    }
                    cursor = owner
                        .document
                        .as_ref()
                        .expect("open UI resource owner retains document")
                        .parent_of(current);
                }
            }
            for root in &roots {
                let Some(root_owner) = owner
                    .document
                    .as_ref()
                    .expect("open UI resource owner retains document")
                    .root_owner(*root)
                else {
                    continue;
                };
                if dirty.contains(&root_owner) {
                    dirty.insert(*root);
                }
            }
            for key in &changes.retired_nodes {
                self.recipes.remove(key);
                self.view_ids.remove(key);
            }
        }

        for root in roots {
            if initial || dirty.contains(&root) || !self.recipes.contains_key(&root) {
                self.build_node(owner, content, root, &dirty, initial)?;
            }
        }
        Ok(())
    }

    pub(crate) fn history_units(
        &self,
        owner: &UiResourceOwner,
        changes: Option<&UiChangeSet>,
    ) -> Result<Vec<HistoryUnitRecipe>> {
        let mut units = Vec::new();
        let roots = changes.map_or_else(
            || owner.history_roots(),
            |changes| changes.history_roots.clone(),
        );
        for root in roots {
            let view = self
                .recipes
                .get(&root)
                .cloned()
                .ok_or_else(|| anyhow!("History root recipe is missing"))?;
            let config = owner
                .root_config(root)
                .ok_or_else(|| anyhow!("History root config is missing"))?;
            let unit = owner
                .history_unit(root)
                .ok_or_else(|| anyhow!("History root native unit identity is missing"))?;
            units.push(HistoryUnitRecipe {
                root,
                view,
                unit_identity: unit.id,
                status: unit.status,
                flow_boundary: if config.flow_boundary == 1 {
                    FlowBoundary::AttachToPrevious
                } else {
                    FlowBoundary::Default
                },
            });
        }
        Ok(units)
    }

    pub(crate) fn body(&self, owner: &UiResourceOwner) -> Result<View> {
        let body = owner
            .document
            .as_ref()
            .ok_or_else(|| anyhow!("UI resource owner has no occurrence document"))?
            .body_root();
        let body = self
            .recipes
            .get(&body)
            .cloned()
            .ok_or_else(|| anyhow!("body occurrence has not been synchronized"))?;
        let mut portals = Vec::new();
        if let Some(document) = owner.document.as_ref() {
            for root in document.portal_roots() {
                if !owner.portal_is_demanded(root)? {
                    continue;
                }
                let view = self
                    .recipes
                    .get(&root)
                    .cloned()
                    .ok_or_else(|| anyhow!("Portal root recipe is missing"))?;
                portals.push(view);
            }
        }
        if portals.is_empty() {
            return Ok(body);
        }
        let mut views = Vec::with_capacity(portals.len() + 1);
        views.push(body);
        views.extend(portals);
        Ok(vf::column(views, 0))
    }

    fn build_node(
        &mut self,
        owner: &UiResourceOwner,
        content: &ContentHostRegistry,
        key: NodeKey,
        dirty: &HashSet<NodeKey>,
        force: bool,
    ) -> Result<View> {
        if !force
            && !dirty.contains(&key)
            && let Some(entry) = self.recipes.get(&key)
        {
            crate::perf::inc(crate::perf::Counter::LegacyRecipeCacheHits);
            #[cfg(test)]
            {
                self.recipe_cache_hits = self.recipe_cache_hits.saturating_add(1);
            }
            return Ok(entry.clone());
        }
        let snapshot = owner
            .document_snapshot(key)
            .map_err(|error| anyhow!(error))?;
        let mut children = Vec::with_capacity(snapshot.children.len());
        for child in snapshot.children.iter().copied() {
            children.push(self.build_node(owner, content, child, dirty, force)?);
        }
        let mut view = self.lower_node(owner, content, &snapshot, children)?;
        view = apply_properties(view, &snapshot.properties);
        if snapshot.hidden {
            view = vf::spacer(0);
        }
        crate::perf::inc(crate::perf::Counter::LegacyRecipeBuilds);
        #[cfg(test)]
        {
            self.recipe_builds = self.recipe_builds.saturating_add(1);
        }
        self.recipes.insert(key, view.clone());
        self.view_ids.insert(key, view.id());
        Ok(view)
    }

    fn lower_node(
        &self,
        owner: &UiResourceOwner,
        content: &ContentHostRegistry,
        snapshot: &OccurrenceSnapshot,
        children: Vec<View>,
    ) -> Result<View> {
        match snapshot.kind {
            HostKind::Box => Ok(match layout_mode(snapshot) {
                LayoutMode::Row => vf::row(children, gap(snapshot)),
                LayoutMode::Grid => {
                    return Err(anyhow!(
                        "grid layout is not available in the M1 terminal adapter"
                    ));
                }
                LayoutMode::Box | LayoutMode::Column => vf::column(children, gap(snapshot)),
            }),
            HostKind::Scroll | HostKind::Animation => {
                let control = snapshot
                    .control
                    .ok_or_else(|| anyhow!("native control occurrence has no control"))?;
                let component = self
                    .control_components
                    .get(&control)
                    .copied()
                    .ok_or_else(|| anyhow!("native control is not installed in the renderer"))?;
                Ok(vf::native_component(component))
            }
            HostKind::ContentHost => {
                let port = snapshot
                    .port
                    .ok_or_else(|| anyhow!("ContentHost occurrence has no ContentPort"))?;
                let id = content
                    .ui_port_id(port)
                    .ok_or_else(|| anyhow!("ContentPort is not installed in the renderer"))?;
                vf::content_host(id).map_err(|error| anyhow!(error))
            }
            HostKind::Editor => {
                let control = snapshot
                    .control
                    .ok_or_else(|| anyhow!("Editor occurrence has no control"))?;
                let component = self
                    .control_components
                    .get(&control)
                    .copied()
                    .ok_or_else(|| anyhow!("Editor control is not installed in the renderer"))?;
                let _ = owner;
                Ok(vf::native_component(component))
            }
        }
    }
}

fn gap(snapshot: &OccurrenceSnapshot) -> u16 {
    effective_u16(snapshot, PropertyId::Gap).unwrap_or(0)
}

fn layout_mode(snapshot: &OccurrenceSnapshot) -> LayoutMode {
    match snapshot.properties.iter().find_map(|(property, value)| {
        (*property == PropertyId::Layout).then_some(match value {
            LayerValue::Value(PropertyValue::LayoutMode(mode)) => *mode,
            _ => LayoutMode::Box,
        })
    }) {
        Some(mode) => mode,
        None => LayoutMode::Box,
    }
}

fn effective_u16(snapshot: &OccurrenceSnapshot, property: PropertyId) -> Option<u16> {
    snapshot.properties.iter().find_map(|(candidate, value)| {
        (*candidate == property).then_some(match value {
            LayerValue::Value(PropertyValue::U16(value)) => Some(*value),
            _ => None,
        })
    })?
}

fn apply_properties(view: View, properties: &[(PropertyId, LayerValue)]) -> View {
    let mut view = view;
    for (property, value) in properties {
        let LayerValue::Value(value) = value else {
            continue;
        };
        view = match (*property, value) {
            (PropertyId::Width, PropertyValue::SizeMode(SizeMode::Fill)) => vf::fill_width(view),
            (PropertyId::Width, PropertyValue::SizeMode(SizeMode::Fit)) => vf::fit_width(view),
            (PropertyId::Height, PropertyValue::SizeMode(SizeMode::Fill)) => vf::fill_height(view),
            (PropertyId::Height, PropertyValue::SizeMode(SizeMode::Fit)) => vf::fit_height(view),
            (PropertyId::Padding, PropertyValue::Insets(insets)) => vf::padding(view, *insets),
            (PropertyId::MinWidth, PropertyValue::U16(width)) => vf::min_width(view, *width),
            (PropertyId::MaxWidth, PropertyValue::U16(width)) => vf::max_width(view, *width),
            (PropertyId::MinHeight, PropertyValue::U16(height)) => vf::min_height(view, *height),
            (PropertyId::MaxHeight, PropertyValue::U16(height)) => vf::max_height(view, *height),
            (PropertyId::Foreground, PropertyValue::Color(color)) => {
                vf::foreground(view, color.clone())
            }
            (PropertyId::Background, PropertyValue::Color(color)) => {
                vf::background(view, color.clone())
            }
            (PropertyId::BorderStyle, PropertyValue::BorderStyle(style)) => {
                let border = match style {
                    BorderStyle::Plain => BorderSpec::plain(),
                    BorderStyle::Rounded => BorderSpec::rounded(),
                    BorderStyle::Double => BorderSpec::double(),
                };
                vf::border(view, border)
            }
            (PropertyId::BorderEdges, PropertyValue::Edges(edges)) => {
                vf::border(view, BorderSpec::plain().edges(*edges))
            }
            (PropertyId::BorderColor, PropertyValue::Color(color)) => {
                vf::border(view, BorderSpec::plain().color(color.clone()))
            }
            (PropertyId::BorderGlyphs, PropertyValue::Glyphs(glyphs)) => {
                vf::border(view, BorderSpec::custom(glyphs.clone()))
            }
            (PropertyId::TextAttributes, PropertyValue::TextAttributes(attributes)) => {
                let mut style = StyleSpec::new();
                for attribute in [
                    TextAttribute::Bold,
                    TextAttribute::Dim,
                    TextAttribute::Italic,
                    TextAttribute::Underline,
                    TextAttribute::Reversed,
                    TextAttribute::Strikethrough,
                ] {
                    if let Some(enabled) = attributes.attribute_value(attribute) {
                        style.set_attribute(attribute, enabled);
                    }
                }
                vf::style(view, StyleRef::direct(style))
            }
            (PropertyId::Style, PropertyValue::Style(style)) => vf::style(view, style.clone()),
            (PropertyId::Alignment, PropertyValue::Alignment(alignment)) => {
                apply_alignment(view, *alignment)
            }
            _ => view,
        };
    }
    view
}

fn apply_alignment(view: View, alignment: crate::occurrence::Alignment) -> View {
    // The current factories expose horizontal/vertical alignment through
    // text and row constructors.  Keep this adapter conservative for Box
    // geometry until the Taffy driver owns the full alignment vocabulary.
    let _ = alignment.horizontal.map(|axis| match axis {
        AlignmentAxis::Start | AlignmentAxis::Top => 0,
        AlignmentAxis::Center => 1,
        AlignmentAxis::End | AlignmentAxis::Bottom => 2,
    });
    view
}

#[cfg(test)]
mod tests {
    use super::super::environment::TuiEnvironment;
    use super::super::host::TuiHost;
    use crate::application::content::TextSourceKind;
    use crate::occurrence::{
        HostKind, LayerValue, NodeRef, OwnershipMode, PropertyId, PropertyValue, ResourceRef,
        UiCommit, UiOperation,
    };
    use tokio::sync::oneshot;

    #[test]
    fn occurrence_literal_reaches_existing_terminal_content_provider() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Block)
            .expect("source");
        let host = TuiHost::open_in_environment(20, 4, true, environment).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(ResourceRef::Local(2)),
        });
        batch.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(2),
            content_format: 1,
            content: b"hello".to_vec(),
            annotations: Vec::new(),
        });
        // The literal connector occupies the port's private connector slot.
        let acknowledgement = host.commit_ui(batch, &[source]).expect("UI commit");
        host.flush_pending_hosts(8, true).expect("frame");
        assert!(host.screen_rows().iter().any(|row| row.contains("hello")));
        let node = acknowledgement.acknowledgement.created[0];
        let mut retire = UiCommit::new(acknowledgement.acknowledgement.accepted_ui_revision);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(node),
        });
        host.commit_ui(retire, &[]).expect("retire UI subtree");
        host.close().expect("close");
    }

    #[test]
    fn legacy_history_root_installs_with_typed_identity_and_repeated_freeze_is_idempotent() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: crate::occurrence::RootRole::LegacyHistoryUnit,
            owner: None,
        });
        let acknowledgement = host.commit_ui(create, &[]).expect("History root");
        host.flush_pending_hosts(8, true).expect("History frame");
        assert_eq!(host.inner.lock().expect("host lock").ui_history_len(), 1);

        let root = acknowledgement.acknowledgement.created[0];
        let mut freeze = UiCommit::new(acknowledgement.acknowledgement.accepted_ui_revision);
        freeze.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(root),
            action_id: 1,
        });
        let frozen = host.commit_ui(freeze, &[]).expect("freeze History root");
        host.flush_pending_hosts(8, true).expect("freeze frame");
        assert_eq!(host.inner.lock().expect("host lock").ui_history_len(), 1);
        assert_eq!(frozen.acknowledgement.accepted_ui_revision, 2);
        host.close().expect("close");
    }

    #[test]
    fn ordinary_history_root_retirement_cannot_bypass_live_tail_ownership() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let mut create = UiCommit::new(0);
        for local_ordinal in 1..=2 {
            create.push(UiOperation::CreateRoot {
                local_ordinal,
                role: crate::occurrence::RootRole::LegacyHistoryUnit,
                owner: None,
            });
        }
        let acknowledgement = host.commit_ui(create, &[]).expect("History roots");
        host.flush_pending_hosts(8, true).expect("History frame");

        let mut retire = UiCommit::new(acknowledgement.acknowledgement.accepted_ui_revision);
        retire.push(UiOperation::RetireRoot {
            root: NodeRef::Existing(acknowledgement.acknowledgement.created[0]),
        });
        let rejection = host
            .commit_ui(retire, &[])
            .expect_err("non-tail retirement");
        assert!(rejection.message.contains("live-tail restrictions"));
        assert_eq!(host.inner.lock().expect("host lock").ui_history_len(), 2);
        host.close().expect("close");
    }

    #[test]
    fn accepted_history_discard_action_retires_the_native_tail() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: crate::occurrence::RootRole::LegacyHistoryUnit,
            owner: None,
        });
        let root = host
            .commit_ui(create, &[])
            .expect("History root")
            .acknowledgement
            .created[0];
        host.flush_pending_hosts(8, true).expect("History frame");

        let mut discard = UiCommit::new(1);
        discard.push(UiOperation::HistoryAction {
            root: NodeRef::Existing(root),
            action_id: 2,
        });
        host.commit_ui(discard, &[]).expect("tail History discard");
        host.flush_pending_hosts(8, true).expect("discard frame");
        assert_eq!(host.inner.lock().expect("host lock").ui_history_len(), 0);
        host.close().expect("close");
    }

    #[test]
    fn native_history_root_can_adapt_an_existing_same_host_unit() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let existing = host
            .history()
            .push(crate::presentation::factory::text("existing"))
            .expect("existing History unit");
        let mut create = UiCommit::new(0);
        create.push(UiOperation::CreateRoot {
            local_ordinal: 1,
            role: crate::occurrence::RootRole::LegacyHistoryUnit,
            owner: None,
        });
        create.set_root_config(
            1,
            crate::occurrence::RootConfig {
                flow_boundary: 0,
                unit_identity: Some(existing.value()),
            },
        );
        create.push(UiOperation::HistoryAction {
            root: NodeRef::Local(1),
            action_id: 1,
        });
        let root = host
            .commit_ui(create, &[])
            .expect("adapt existing History unit")
            .acknowledgement
            .created[0];
        host.flush_pending_hosts(8, true)
            .expect("adapted History frame");
        assert_eq!(
            host.ui_history_unit_identity(root).expect("identity"),
            Some(existing.value())
        );
        assert_eq!(host.inner.lock().expect("host lock").ui_history_len(), 1);
        host.close().expect("close");
    }

    #[test]
    fn masked_declaration_promotes_metadata_without_running_scene_preparation() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::SetOverride {
            node: NodeRef::Local(1),
            property: crate::occurrence::PropertyId::Foreground,
            value: crate::occurrence::LayerValue::Value(crate::occurrence::PropertyValue::Color(
                crate::occurrence::ColorValue::ansi(2),
            )),
        });
        let mounted = host.commit_ui(mount, &[]).expect("mount");
        let node = mounted.acknowledgement.created[0];
        host.flush_pending_hosts(8, true).expect("initial frame");
        let before = host.epochs().expect("initial epochs");
        let before_recipe_builds = host
            .inner
            .lock()
            .expect("host lock")
            .legacy_scene
            .recipe_builds;

        crate::presentation::layout::reset_layout_counters();
        let mut masked = UiCommit::new(1);
        masked.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(node),
            property: crate::occurrence::PropertyId::Foreground,
            value: crate::occurrence::LayerValue::Value(crate::occurrence::PropertyValue::Color(
                crate::occurrence::ColorValue::ansi(4),
            )),
        });
        masked.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(node),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(masked, &[]).expect("masked declaration");
        host.flush_pending_hosts(8, true).expect("metadata frame");
        let after = host.epochs().expect("metadata epochs");
        assert!(after.visible_structural_revision > before.visible_structural_revision);
        assert_eq!(after.visible_frame_revision, before.visible_frame_revision);
        assert_eq!(after.pending_epoch, after.committed_epoch);
        assert_eq!(crate::presentation::layout::layout_counters(), (0, 0, 0));
        assert_eq!(
            host.inner
                .lock()
                .expect("host lock")
                .legacy_scene
                .recipe_builds,
            before_recipe_builds,
            "metadata-only UI work must not rebuild occurrence recipes"
        );
        host.close().expect("close");
    }

    #[test]
    fn cached_descendants_survive_metadata_then_deep_sparse_projection() {
        let host = TuiHost::open(24, 6, true).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        for ordinal in 1..=3 {
            mount.push(UiOperation::CreateNode {
                local_ordinal: ordinal,
                kind: HostKind::Box,
            });
        }
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(2),
            child: NodeRef::Local(3),
            before: None,
        });
        let mounted = host.commit_ui(mount, &[]).expect("nested mount");
        host.flush_pending_hosts(8, true).expect("initial frame");
        let nodes = mounted.acknowledgement.created.clone();
        {
            let inner = host.inner.lock().expect("host lock");
            assert_eq!(inner.legacy_scene.recipes.len(), 4);
            assert!(
                inner
                    .legacy_scene
                    .recipes
                    .contains_key(&nodes[2].node_key().expect("node"))
            );
        }

        let mut metadata = UiCommit::new(1);
        metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(nodes[2]),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(metadata, &[]).expect("metadata update");
        host.flush_pending_hosts(8, true).expect("metadata frame");

        let mut deep_edit = UiCommit::new(2);
        deep_edit.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(nodes[2]),
            property: PropertyId::Foreground,
            value: LayerValue::Value(PropertyValue::Color(crate::occurrence::ColorValue::ansi(5))),
        });
        host.commit_ui(deep_edit, &[])
            .expect("deep property update");
        host.flush_pending_hosts(8, true).expect("deep frame");

        let inner = host.inner.lock().expect("host lock");
        for handle in nodes.iter() {
            let key = handle.node_key().expect("node handle");
            assert!(
                inner.legacy_scene.recipes.contains_key(&key),
                "cached occurrence recipe was lost for {key:?}"
            );
        }
        drop(inner);
        host.close().expect("close");
    }

    #[test]
    fn wide_tree_leaf_projection_reuses_clean_sibling_recipes() {
        const WIDTH: u32 = 32;
        let host = TuiHost::open(40, 8, true).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        for ordinal in 1..=WIDTH {
            mount.push(UiOperation::CreateNode {
                local_ordinal: ordinal,
                kind: HostKind::Box,
            });
            mount.push(UiOperation::InsertBefore {
                parent: NodeRef::Existing(body),
                child: NodeRef::Local(ordinal),
                before: None,
            });
        }
        let mounted = host.commit_ui(mount, &[]).expect("wide mount");
        host.flush_pending_hosts(8, true).expect("initial frame");
        let initial_builds = host
            .inner
            .lock()
            .expect("host lock")
            .legacy_scene
            .recipe_builds;
        assert_eq!(initial_builds, WIDTH as usize + 1);

        let leaf = mounted.acknowledgement.created[usize::try_from(WIDTH - 1).expect("index")];
        let mut update = UiCommit::new(1);
        update.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(leaf),
            property: PropertyId::Foreground,
            value: LayerValue::Value(PropertyValue::Color(crate::occurrence::ColorValue::ansi(6))),
        });
        host.commit_ui(update, &[]).expect("leaf update");
        host.flush_pending_hosts(8, true).expect("sparse frame");

        let inner = host.inner.lock().expect("host lock");
        assert_eq!(
            inner.legacy_scene.recipe_builds - initial_builds,
            2,
            "only the changed leaf and its body ancestor need new legacy recipes"
        );
        assert!(inner.legacy_scene.recipe_cache_hits >= WIDTH as usize - 1);
        drop(inner);
        host.close().expect("close");
    }

    #[test]
    fn wide_literal_update_uses_only_the_affected_demand_route() {
        const WIDTH: usize = 32;
        let host =
            TuiHost::open_in_environment(48, 8, true, TuiEnvironment::new_manual()).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        for index in 0..WIDTH {
            let node = u32::try_from(index * 2 + 1).expect("node ordinal");
            let port = node + 1;
            mount.push(UiOperation::CreateNode {
                local_ordinal: node,
                kind: HostKind::ContentHost,
            });
            mount.push(UiOperation::CreatePort {
                local_ordinal: port,
                content_family: 1,
                ownership: OwnershipMode::OccurrenceOwned,
                owner: Some(NodeRef::Local(node)),
            });
            mount.push(UiOperation::InsertBefore {
                parent: NodeRef::Existing(body),
                child: NodeRef::Local(node),
                before: None,
            });
            mount.push(UiOperation::AttachPort {
                node: NodeRef::Local(node),
                port: Some(ResourceRef::Local(port)),
            });
            mount.push(UiOperation::ReplaceLiteral {
                port: ResourceRef::Local(port),
                content_format: 1,
                content: format!("literal-{index}").into_bytes(),
                annotations: Vec::new(),
            });
        }
        let mounted = host.commit_ui(mount, &[]).expect("wide literal mount");
        host.flush_pending_hosts(8, true)
            .expect("wide literal initial frame");
        let before = host
            .inner
            .lock()
            .expect("host lock")
            .content
            .test_ui_demand_nodes_visited();
        let before_owner_nodes = host
            .inner
            .lock()
            .expect("host lock")
            .content
            .test_ui_owner_nodes_visited();

        let first_port = mounted.acknowledgement.created[1];
        let mut update = UiCommit::new(1);
        update.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Existing(first_port),
            content_format: 1,
            content: b"updated-first".to_vec(),
            annotations: Vec::new(),
        });
        host.commit_ui(update, &[]).expect("wide literal update");
        host.flush_pending_hosts(8, true)
            .expect("wide literal update frame");
        let after = host
            .inner
            .lock()
            .expect("host lock")
            .content
            .test_ui_demand_nodes_visited();
        let after_owner_nodes = host
            .inner
            .lock()
            .expect("host lock")
            .content
            .test_ui_owner_nodes_visited();
        assert!(after.saturating_sub(before) <= 3);
        assert!(after.saturating_sub(before) < WIDTH);
        assert!(after_owner_nodes.saturating_sub(before_owner_nodes) < WIDTH);
        assert!(
            host.screen_rows()
                .iter()
                .any(|row| row.contains("updated-first"))
        );
        host.close().expect("close");
    }

    #[test]
    fn portal_descendant_update_reuses_root_and_preserves_physical_text() {
        let environment = TuiEnvironment::new();
        let source = environment
            .create_content_source(TextSourceKind::Block)
            .expect("source");
        let host = TuiHost::open_in_environment(24, 6, true, environment).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateRoot {
            local_ordinal: 2,
            role: crate::occurrence::RootRole::Portal,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 4,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(3)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(2),
            child: NodeRef::Local(3),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(3),
            port: Some(ResourceRef::Local(4)),
        });
        mount.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(4),
            content_format: 1,
            content: b"portal".to_vec(),
            annotations: Vec::new(),
        });
        let mounted = host.commit_ui(mount, &[source]).expect("portal mount");
        host.flush_pending_hosts(8, true).expect("portal frame");
        let before = host.screen_rows();

        let child = mounted.acknowledgement.created[2];
        let mut update = UiCommit::new(1);
        update.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(child),
            property: PropertyId::Foreground,
            value: LayerValue::Value(PropertyValue::Color(crate::occurrence::ColorValue::ansi(3))),
        });
        host.commit_ui(update, &[])
            .expect("portal descendant update");
        host.flush_pending_hosts(8, true)
            .expect("portal update frame");
        assert_eq!(host.screen_rows(), before);
        assert!(host.screen_rows().iter().any(|row| row.contains("portal")));

        let owner = mounted.acknowledgement.created[0];
        let mut hide = UiCommit::new(2);
        hide.push(UiOperation::SetHidden {
            node: NodeRef::Existing(owner),
            hidden: true,
        });
        host.commit_ui(hide, &[]).expect("hide portal owner");
        host.flush_pending_hosts(8, true)
            .expect("hidden portal frame");
        assert!(!host.screen_rows().iter().any(|row| row.contains("portal")));

        let mut unhide = UiCommit::new(3);
        unhide.push(UiOperation::SetHidden {
            node: NodeRef::Existing(owner),
            hidden: false,
        });
        host.commit_ui(unhide, &[]).expect("unhide portal owner");
        host.flush_pending_hosts(8, true)
            .expect("unhidden portal frame");
        assert!(host.screen_rows().iter().any(|row| row.contains("portal")));
        host.close().expect("close");
    }

    #[test]
    fn portal_owned_by_inactive_animation_frame_is_not_demanded() {
        let host =
            TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Animation,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 2,
            kind: crate::occurrence::ControlKind::Animation,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 4,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateRoot {
            local_ordinal: 5,
            role: crate::occurrence::RootRole::Portal,
            owner: Some(NodeRef::Local(4)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 6,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 7,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(6)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(3),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(4),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(5),
            child: NodeRef::Local(6),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(6),
            port: Some(ResourceRef::Local(7)),
        });
        mount.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(7),
            content_format: 1,
            content: b"inactive-portal".to_vec(),
            annotations: Vec::new(),
        });
        host.commit_ui(mount, &[]).expect("inactive portal mount");
        host.flush_pending_hosts(8, true)
            .expect("inactive portal frame");
        assert!(
            !host
                .screen_rows()
                .iter()
                .any(|row| row.contains("inactive-portal"))
        );
        host.close().expect("close");
    }

    #[test]
    fn overlapping_membership_frontier_roots_are_union_deduplicated() {
        let host =
            TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 3,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(2)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(2),
            port: Some(ResourceRef::Local(3)),
        });
        mount.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(3),
            content_format: 1,
            content: b"overlap".to_vec(),
            annotations: Vec::new(),
        });
        let mounted = host.commit_ui(mount, &[]).expect("nested content mount");
        host.flush_pending_hosts(8, true).expect("initial frame");
        assert!(host.screen_rows().iter().any(|row| row.contains("overlap")));

        let parent = mounted.acknowledgement.created[0];
        let child = mounted.acknowledgement.created[1];
        let mut hide = UiCommit::new(1);
        hide.push(UiOperation::SetHidden {
            node: NodeRef::Existing(parent),
            hidden: true,
        });
        hide.push(UiOperation::SetHidden {
            node: NodeRef::Existing(child),
            hidden: true,
        });
        host.commit_ui(hide, &[]).expect("overlapping hide");
        host.flush_pending_hosts(8, true)
            .expect("overlapping hide frame");
        assert!(!host.screen_rows().iter().any(|row| row.contains("overlap")));

        let mut show = UiCommit::new(2);
        show.push(UiOperation::SetHidden {
            node: NodeRef::Existing(parent),
            hidden: false,
        });
        show.push(UiOperation::SetHidden {
            node: NodeRef::Existing(child),
            hidden: false,
        });
        host.commit_ui(show, &[]).expect("overlapping show");
        host.flush_pending_hosts(8, true)
            .expect("overlapping show frame");
        assert!(host.screen_rows().iter().any(|row| row.contains("overlap")));
        host.close().expect("close");
    }

    #[test]
    fn retired_membership_frontier_drops_old_generation_before_slot_reuse() {
        let host =
            TuiHost::open_in_environment(32, 8, true, TuiEnvironment::new_manual()).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 2,
            kind: HostKind::ContentHost,
        });
        mount.push(UiOperation::CreatePort {
            local_ordinal: 3,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(2)),
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(2),
            before: None,
        });
        mount.push(UiOperation::AttachPort {
            node: NodeRef::Local(2),
            port: Some(ResourceRef::Local(3)),
        });
        mount.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(3),
            content_format: 1,
            content: b"retired".to_vec(),
            annotations: Vec::new(),
        });
        let mounted = host.commit_ui(mount, &[]).expect("generation mount");
        host.flush_pending_hosts(8, true)
            .expect("generation initial frame");
        let old_parent = mounted.acknowledgement.created[0];
        let old_child = mounted.acknowledgement.created[1];
        let old_port = mounted.acknowledgement.created[2];

        let mut hide = UiCommit::new(1);
        hide.push(UiOperation::SetHidden {
            node: NodeRef::Existing(old_parent),
            hidden: true,
        });
        host.commit_ui(hide, &[]).expect("hide before retirement");
        let mut retire = UiCommit::new(2);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(old_parent),
        });
        host.commit_ui(retire, &[]).expect("retire before drain");
        let report = host.flush_pending_hosts(8, true).expect("retirement frame");
        assert!(
            report.errors.is_empty(),
            "retirement errors: {:?}",
            report.errors
        );
        {
            let inner = host.inner.lock().expect("host lock");
            assert!(
                inner
                    .ui_resources
                    .document_snapshot(old_parent.node_key().expect("old parent"))
                    .is_err()
            );
            assert!(
                inner
                    .ui_resources
                    .document_snapshot(old_child.node_key().expect("old child"))
                    .is_err()
            );
            assert!(
                !inner
                    .content
                    .ui_port_id(old_port.resource_key().expect("old port"))
                    .is_some()
            );
        }

        let mut reuse = UiCommit::new(3);
        reuse.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Box,
        });
        reuse.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        let reused = host.commit_ui(reuse, &[]).expect("slot reuse");
        host.flush_pending_hosts(8, true).expect("reuse frame");
        let new_node = reused.acknowledgement.created[0];
        assert_ne!(new_node.generation, old_parent.generation);
        let inner = host.inner.lock().expect("host lock");
        assert!(
            inner
                .legacy_scene
                .recipes
                .contains_key(&new_node.node_key().expect("new node"))
        );
        drop(inner);
        host.close().expect("close");
    }

    #[test]
    fn cached_control_children_remain_addressable_after_deep_update() {
        let host = TuiHost::open(24, 6, true).expect("host");
        let body = host.ui_body_handle().expect("body");
        let mut mount = UiCommit::new(0);
        mount.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::Scroll,
        });
        mount.push(UiOperation::CreateControl {
            local_ordinal: 2,
            kind: crate::occurrence::ControlKind::Scroll,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        mount.push(UiOperation::CreateNode {
            local_ordinal: 3,
            kind: HostKind::Box,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        mount.push(UiOperation::InsertBefore {
            parent: NodeRef::Local(1),
            child: NodeRef::Local(3),
            before: None,
        });
        let mounted = host.commit_ui(mount, &[]).expect("control mount");
        host.flush_pending_hosts(8, true)
            .expect("initial control frame");
        let scroll = mounted.acknowledgement.created[0];
        let child = mounted.acknowledgement.created[2];

        let mut metadata = UiCommit::new(1);
        metadata.push(UiOperation::SetSubscriptions {
            node: NodeRef::Existing(scroll),
            mask_low: 1,
            mask_high: 0,
        });
        host.commit_ui(metadata, &[])
            .expect("control metadata update");
        host.flush_pending_hosts(8, true)
            .expect("control metadata frame");

        let mut deep_update = UiCommit::new(2);
        deep_update.push(UiOperation::SetDeclared {
            node: NodeRef::Existing(child),
            property: PropertyId::Foreground,
            value: LayerValue::Value(PropertyValue::Color(crate::occurrence::ColorValue::ansi(4))),
        });
        host.commit_ui(deep_update, &[])
            .expect("control descendant update");
        host.flush_pending_hosts(8, true)
            .expect("control descendant frame");

        let inner = host.inner.lock().expect("host lock");
        let snapshot = inner
            .ui_resources
            .document_snapshot(scroll.node_key().expect("scroll"))
            .expect("scroll snapshot");
        assert_eq!(
            inner
                .legacy_scene
                .children_for(&snapshot)
                .expect("cached control children")
                .len(),
            1
        );
        drop(inner);
        host.close().expect("close");
    }

    fn mount_literal(host: &TuiHost, text: &[u8]) -> crate::occurrence::UiAcknowledgement {
        let body = host.ui_body_handle().expect("body");
        let mut batch = UiCommit::new(0);
        batch.push(UiOperation::CreateNode {
            local_ordinal: 1,
            kind: HostKind::ContentHost,
        });
        batch.push(UiOperation::CreatePort {
            local_ordinal: 2,
            content_family: 1,
            ownership: OwnershipMode::OccurrenceOwned,
            owner: Some(NodeRef::Local(1)),
        });
        batch.push(UiOperation::InsertBefore {
            parent: NodeRef::Existing(body),
            child: NodeRef::Local(1),
            before: None,
        });
        batch.push(UiOperation::AttachPort {
            node: NodeRef::Local(1),
            port: Some(ResourceRef::Local(2)),
        });
        batch.push(UiOperation::ReplaceLiteral {
            port: ResourceRef::Local(2),
            content_format: 1,
            content: text.to_vec(),
            annotations: Vec::new(),
        });
        host.commit_ui(batch, &[])
            .expect("literal UI commit")
            .acknowledgement
    }

    #[test]
    fn occurrence_route_receipt_failure_retains_confirmed_output_and_recovery() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        mount_literal(&host, b"confirmed");
        host.flush_pending_hosts(8, true).expect("initial frame");
        let old = host.screen_rows();
        let (sender, receiver) = oneshot::channel();
        {
            let mut inner = host.inner.lock().expect("host lock");
            let candidate = inner.prepare_test_candidate().expect("candidate frame");
            inner
                .install_test_in_flight(candidate, receiver)
                .expect("in-flight frame");
            inner.mark_pending().expect("pending receipt work");
        }
        sender
            .send(Err(anyhow::anyhow!("simulated partial viewport write")))
            .expect("receipt send");
        let report = host.flush_pending_hosts(8, false).expect("failure report");
        assert_eq!(report.errors.len(), 1);
        assert_eq!(host.screen_rows(), old);
        host.flush_pending_hosts(8, true).expect("recovery frame");
        assert_eq!(host.screen_rows(), old);
        host.close().expect("close");
    }

    #[test]
    fn retiring_a_visible_occurrence_removes_its_sparse_content_projection() {
        let host =
            TuiHost::open_in_environment(20, 4, true, TuiEnvironment::new_manual()).expect("host");
        let acknowledgement = mount_literal(&host, b"retire");
        host.flush_pending_hosts(8, true).expect("initial frame");
        let mut retire = UiCommit::new(acknowledgement.accepted_ui_revision);
        retire.push(UiOperation::RetireSubtree {
            root: NodeRef::Existing(acknowledgement.created[0]),
        });
        host.commit_ui(retire, &[]).expect("retire occurrence");
        let report = host.flush_pending_hosts(8, true).expect("retirement frame");
        assert!(
            report.errors.is_empty(),
            "retirement errors: {:?}",
            report.errors
        );
        assert!(!host.screen_rows().iter().any(|row| row.contains("retire")));
        host.close().expect("close");
    }

    #[test]
    fn occurrence_route_close_waits_for_pending_receipt_before_owner_cleanup() {
        let host = TuiHost::open(20, 4, true).expect("host");
        mount_literal(&host, b"close-pending");
        host.flush_pending_hosts(8, true).expect("initial frame");
        let (sender, receiver) = oneshot::channel();
        {
            let mut inner = host.inner.lock().expect("host lock");
            let candidate = inner.prepare_test_candidate().expect("candidate frame");
            inner
                .install_test_in_flight(candidate, receiver)
                .expect("in-flight frame");
            inner.mark_pending().expect("pending receipt work");
        }
        let closing = host.clone();
        let join = std::thread::spawn(move || closing.close());
        std::thread::sleep(std::time::Duration::from_millis(5));
        sender.send(Ok(())).expect("receipt send");
        join.join().expect("close thread").expect("close");
        assert!(host.exited());
    }
}
