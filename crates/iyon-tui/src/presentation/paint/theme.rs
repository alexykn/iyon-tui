//! Semantic style cascade and theme-key resolution into physical styles.

use crate::{
    Theme,
    component::{ComponentId, MountGraph},
    physical::{AnsiColor as PhysicalAnsiColor, PhysicalColor, PhysicalStyle},
    presentation::api::{
        AnsiColor, ColorSpec, StyleFacts, StyleRef, StyleSpec, StyleStates, ThemeColor,
    },
};

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleContext {
    pub(crate) inherited_states: StyleStates,
    pub(crate) local_facts: StyleFacts,
    pub(crate) focused: bool,
    pub(crate) focus_within: bool,
}

impl StyleContext {
    /// Enters an occurrence node, installing its inheritable state and
    /// self-only semantic facts while updating the node's focus scope.
    pub(crate) fn enter_node(&self, states: &StyleStates, facts: &StyleFacts, scope: Self) -> Self {
        let mut next = self.clone();
        next.inherited_states.overlay(states);
        next.local_facts = facts.clone();
        next.focused = scope.focused;
        next.focus_within = scope.focus_within;
        next
    }

    /// Context passed to descendants. Physical style is passed separately;
    /// only the current node's local facts are cleared.
    pub(crate) fn for_descendant(&self) -> Self {
        Self {
            inherited_states: self.inherited_states.clone(),
            local_facts: StyleFacts::default(),
            focused: self.focused,
            focus_within: self.focus_within,
        }
    }

    pub(crate) fn with_local_facts(&self, facts: &StyleFacts) -> Self {
        let mut next = self.for_descendant();
        next.local_facts = facts.clone();
        next
    }

    pub(crate) fn for_scope(
        scope: Option<ComponentId>,
        focused: Option<ComponentId>,
        graph: Option<&MountGraph>,
    ) -> Self {
        let is_focused = scope.is_some_and(|scope| focused == Some(scope));
        let focus_within = scope.is_some_and(|scope| {
            focused.is_some_and(|focused| {
                graph.is_some_and(|graph| graph.is_descendant_or_self(focused, scope))
            })
        });
        Self {
            inherited_states: StyleStates::default(),
            local_facts: StyleFacts::default(),
            focused: is_focused,
            focus_within,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeResolver {
    framework: Theme,
    application: Theme,
}

impl Default for ThemeResolver {
    fn default() -> Self {
        Self::new(&Theme::default())
    }
}

impl ThemeResolver {
    pub(crate) fn new(application: &Theme) -> Self {
        Self {
            framework: crate::theme::framework_theme(),
            application: application.clone(),
        }
    }

    pub(crate) fn resolve_text_style(
        &self,
        inherited: PhysicalStyle,
        patch: &StyleRef,
        context: &StyleContext,
    ) -> PhysicalStyle {
        let mut resolved = inherited;
        if let Some(key) = &patch.theme {
            if let Some(named) = self.resolve_style(&self.framework, key.as_str(), context) {
                resolved = self.apply_style(resolved, &named, context);
            }
            if let Some(named) = self.resolve_style(&self.application, key.as_str(), context) {
                resolved = self.apply_style(resolved, &named, context);
            }
        }
        self.apply_style(resolved, &patch.local, context)
    }

    fn apply_style(
        &self,
        mut resolved: PhysicalStyle,
        patch: &StyleSpec,
        context: &StyleContext,
    ) -> PhysicalStyle {
        if let Some(foreground) = &patch.foreground {
            resolved.foreground = Some(self.resolve_color(foreground, context));
        }
        if let Some(background) = &patch.background {
            resolved.background = Some(self.resolve_color(background, context));
        }
        if let Some(value) = patch.attributes.bold {
            resolved.bold = value;
        }
        if let Some(value) = patch.attributes.dim {
            resolved.dim = value;
        }
        if let Some(value) = patch.attributes.italic {
            resolved.italic = value;
        }
        if let Some(value) = patch.attributes.underline {
            resolved.underline = value;
        }
        if let Some(value) = patch.attributes.reversed {
            resolved.reversed = value;
        }
        if let Some(value) = patch.attributes.strikethrough {
            resolved.strikethrough = value;
        }
        resolved
    }

    fn resolve_style(&self, theme: &Theme, key: &str, context: &StyleContext) -> Option<StyleSpec> {
        theme.resolve_style(
            key,
            context.focused,
            context.focus_within,
            &context.inherited_states,
            &context.local_facts,
        )
    }

    fn resolve_theme_color(&self, key: &str, context: &StyleContext) -> Option<ThemeColor> {
        self.application
            .resolve_color(
                key,
                context.focused,
                context.focus_within,
                &context.inherited_states,
                &context.local_facts,
            )
            .or_else(|| {
                self.framework.resolve_color(
                    key,
                    context.focused,
                    context.focus_within,
                    &context.inherited_states,
                    &context.local_facts,
                )
            })
    }

    pub(crate) fn resolve_color(&self, color: &ColorSpec, context: &StyleContext) -> PhysicalColor {
        match color {
            ColorSpec::Ansi(value) => PhysicalColor::Indexed(*value),
            ColorSpec::Named(color) => PhysicalColor::Named(to_physical_ansi(*color)),
            ColorSpec::Rgb { r, g, b } => PhysicalColor::Rgb {
                r: *r,
                g: *g,
                b: *b,
            },
            ColorSpec::Theme(key) => self
                .resolve_theme_color(key.as_str(), context)
                .map_or(PhysicalColor::Default, to_physical_color),
        }
    }
}

fn to_physical_color(color: ThemeColor) -> PhysicalColor {
    match color {
        ThemeColor::Default => PhysicalColor::Default,
        ThemeColor::Named(color) => PhysicalColor::Named(to_physical_ansi(color)),
        ThemeColor::Indexed(value) => PhysicalColor::Indexed(value),
        ThemeColor::Rgb { r, g, b } => PhysicalColor::Rgb { r, g, b },
    }
}

fn to_physical_ansi(color: AnsiColor) -> PhysicalAnsiColor {
    match color {
        AnsiColor::Black => PhysicalAnsiColor::Black,
        AnsiColor::Red => PhysicalAnsiColor::Red,
        AnsiColor::Green => PhysicalAnsiColor::Green,
        AnsiColor::Yellow => PhysicalAnsiColor::Yellow,
        AnsiColor::Blue => PhysicalAnsiColor::Blue,
        AnsiColor::Magenta => PhysicalAnsiColor::Magenta,
        AnsiColor::Cyan => PhysicalAnsiColor::Cyan,
        AnsiColor::Gray => PhysicalAnsiColor::Gray,
        AnsiColor::DarkGray => PhysicalAnsiColor::DarkGray,
        AnsiColor::LightRed => PhysicalAnsiColor::LightRed,
        AnsiColor::LightGreen => PhysicalAnsiColor::LightGreen,
        AnsiColor::LightYellow => PhysicalAnsiColor::LightYellow,
        AnsiColor::LightBlue => PhysicalAnsiColor::LightBlue,
        AnsiColor::LightMagenta => PhysicalAnsiColor::LightMagenta,
        AnsiColor::LightCyan => PhysicalAnsiColor::LightCyan,
        AnsiColor::White => PhysicalAnsiColor::White,
    }
}
