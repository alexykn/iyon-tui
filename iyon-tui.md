# iyon-tui public API

Complete public API inventory for crates/iyon-tui, starting at lib.rs and following only publicly reachable items. [re-export] marks paths introduced by pub use; [definition] marks items defined in the public module. Inherent methods and crate-defined trait methods are listed; blanket, auto-trait, and derived implementation methods are omitted. No public struct fields were present in the reachable surface.

## Public modules

- iyon_tui::prelude — public module [definition]; curated re-export set for common imports.
- iyon_tui::projection — public module [definition]; root-coordinate projection algebra and diagnostics.
- iyon_tui::stream — public module [definition]; source-rooted coordinates used by semantic content projections.
- iyon_tui::text — public module [definition]; semantic text IR, traversal, projectors, and renderers.
- iyon_tui::testing — public module [definition; feature test-util]; deterministic headless application driving and painted-view inspection.

## Prelude re-exports

iyon_tui::prelude re-exports: App, AppCx, Block, Component, ComponentCx, DiffHunk, DiffLine, DiffLineKind, DiffLineNumber, DiffLineOffset, DiffLineTermination, DiffRange, DiffRenderer, EventCx, History, HistoryLayout, Inline, InlineContent, IntoView, MarkdownProjector, Output, PlainTextProjector, Projection, Projector, ProjectorExt, Renderer, Scene, ScrollPane, Smooth, TextContent, TextInput, TextOrigin, TextRenderPolicy, TextRenderer, TextSelector, Theme, and View.

## Root iyon_tui paths

#### `iyon_tui::AnsiColor` — enum [re-export]
- Signature: `pub enum AnsiColor { Black, Red, Green, Yellow, Blue, Magenta, Cyan, Gray, DarkGray, LightRed, LightGreen, LightYellow, LightBlue, LightMagenta, LightCyan, White, }`
- Purpose: Public named ANSI colors supported by terminal backends.
- Variants and fields: `Black, Red, Green, Yellow, Blue, Magenta, Cyan, Gray, DarkGray, LightRed, LightGreen, LightYellow, LightBlue, LightMagenta, LightCyan, White,`
- Variant paths: `iyon_tui::AnsiColor::Black`, `iyon_tui::AnsiColor::Red`, `iyon_tui::AnsiColor::Green`, `iyon_tui::AnsiColor::Yellow`, `iyon_tui::AnsiColor::Blue`, `iyon_tui::AnsiColor::Magenta`, `iyon_tui::AnsiColor::Cyan`, `iyon_tui::AnsiColor::Gray`, `iyon_tui::AnsiColor::DarkGray`, `iyon_tui::AnsiColor::LightRed`, `iyon_tui::AnsiColor::LightGreen`, `iyon_tui::AnsiColor::LightYellow`, `iyon_tui::AnsiColor::LightBlue`, `iyon_tui::AnsiColor::LightMagenta`, `iyon_tui::AnsiColor::LightCyan`, `iyon_tui::AnsiColor::White`.

#### `iyon_tui::AppSendError` — enum [re-export]
- Signature: `pub enum AppSendError<Action> { Full(Action), Closed(Action), }`
- Purpose: Indicates why a nonblocking Action send did not complete.
- Variants and fields: `Full(Action), Closed(Action),`
- Variant paths: `iyon_tui::AppSendError::Full`, `iyon_tui::AppSendError::Closed`.
- `iyon_tui::AppSendError::action` — inherent method; `pub fn action (&self) -> &Action` — Provides the public `action` operation.
- `iyon_tui::AppSendError::into_inner` — inherent method; `pub fn into_inner (self) -> Action` — Converts or exposes this value.
- `iyon_tui::AppSendError::is_full` — inherent method; `pub fn is_full (&self) -> bool` — Reports whether `full` holds.

#### `iyon_tui::BorderStyle` — enum [re-export]
- Signature: `pub enum BorderStyle { Plain, Rounded, Double, }`
- Purpose: Terminal-independent border family used by the convenience constructors.
- Variants and fields: `Plain, Rounded, Double,`
- Variant paths: `iyon_tui::BorderStyle::Plain`, `iyon_tui::BorderStyle::Rounded`, `iyon_tui::BorderStyle::Double`.

#### `iyon_tui::CodeBlockLabelPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum CodeBlockLabelPolicy { Hidden, Language, Info, }`
- Purpose: Optional code-block label presentation.
- Variants and fields: `Hidden, Language, Info,`
- Variant paths: `iyon_tui::CodeBlockLabelPolicy::Hidden`, `iyon_tui::CodeBlockLabelPolicy::Language`, `iyon_tui::CodeBlockLabelPolicy::Info`.

#### `iyon_tui::ColorSpec` — enum [re-export]
- Signature: `pub enum ColorSpec { Theme( ThemeKey ), Named( AnsiColor ), Ansi( u8 ), Rgb { r: u8 , g: u8 , b: u8 , }, }`
- Purpose: Backend-neutral theme, ANSI, or RGB color specification.
- Variants and fields: `Theme( ThemeKey ), Named( AnsiColor ), Ansi( u8 ), Rgb { r: u8 , g: u8 , b: u8 , },`
- Variant paths: `iyon_tui::ColorSpec::Theme`, `iyon_tui::ColorSpec::Named`, `iyon_tui::ColorSpec::Ansi`, `iyon_tui::ColorSpec::Rgb`.
- `iyon_tui::ColorSpec::theme` — inherent method; `pub fn theme (key: impl Into < ThemeKey >) -> Self` — Returns `theme`.
- `iyon_tui::ColorSpec::named` — inherent method; `pub const fn named (color: AnsiColor ) -> Self` — Provides the public `named` operation.
- `iyon_tui::ColorSpec::ansi` — inherent method; `pub const fn ansi (value: u8 ) -> Self` — Provides the public `ansi` operation.
- `iyon_tui::ColorSpec::rgb` — inherent method; `pub const fn rgb (r: u8 , g: u8 , b: u8 ) -> Self` — Provides the public `rgb` operation.

#### `iyon_tui::DiffLineKind` — enum [re-export]
- Signature: `pub enum DiffLineKind { Context, Addition, Deletion, }`
- Purpose: The semantic classification of a changed source line.
- Variants and fields: `Context, Addition, Deletion,`
- Variant paths: `iyon_tui::DiffLineKind::Context`, `iyon_tui::DiffLineKind::Addition`, `iyon_tui::DiffLineKind::Deletion`.

#### `iyon_tui::DiffLineTermination` — enum [re-export]
- Signature: `pub enum DiffLineTermination { Terminated, Unterminated, }`
- Purpose: Whether a source line has a line terminator.
- Variants and fields: `Terminated, Unterminated,`
- Variant paths: `iyon_tui::DiffLineTermination::Terminated`, `iyon_tui::DiffLineTermination::Unterminated`.

#### `iyon_tui::DiffValidationError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum DiffValidationError { RangeOverflow { start: DiffLineOffset , line_count: u64 , }, CoordinateOverflow { index: usize , side: &'static str , }, LineCoordinateMismatch { index: usize , kind: DiffLineKind , expected_old: Option < DiffLineNumber >, actual_old: Option < DiffLineNumber >, expected_new: Option < DiffLineNumber >, actual_new: Option < DiffLineNumber >, }, CountMismatch { expected_old: u64 , consumed_old: u64 , expected_new: u64 , consumed_new: u64 , }, }`
- Purpose: Validation diagnostics for semantic diff ranges and hunks.
- Variants and fields: `RangeOverflow { start: DiffLineOffset , line_count: u64 , }, CoordinateOverflow { index: usize , side: &'static str , }, LineCoordinateMismatch { index: usize , kind: DiffLineKind , expected_old: Option < DiffLineNumber >, actual_old: Option < DiffLineNumber >, expected_new: Option < DiffLineNumber >, actual_new: Option < DiffLineNumber >, }, CountMismatch { expected_old: u64 , consumed_old: u64 , expected_new: u64 , consumed_new: u64 , },`
- Variant paths: `iyon_tui::DiffValidationError::RangeOverflow`, `iyon_tui::DiffValidationError::CoordinateOverflow`, `iyon_tui::DiffValidationError::LineCoordinateMismatch`, `iyon_tui::DiffValidationError::CountMismatch`.

#### `iyon_tui::FlowBoundary` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum FlowBoundary { Default, AttachToPrevious, }`
- Purpose: Relationship between a history unit and its logical predecessor.
- Variants and fields: `Default, AttachToPrevious,`
- Variant paths: `iyon_tui::FlowBoundary::Default`, `iyon_tui::FlowBoundary::AttachToPrevious`.

#### `iyon_tui::HistoryError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum HistoryError { UnitNotFound { unit: HistoryUnitId , }, UnitNotLive { unit: HistoryUnitId , }, LiveMustRemainTail { unit: HistoryUnitId , }, FinalViewContainsComponent { unit: HistoryUnitId , }, }`
- Purpose: Invariant-preserving diagnostics for ordered static/live History units.
- Variants and fields: `UnitNotFound { unit: HistoryUnitId , }, UnitNotLive { unit: HistoryUnitId , }, LiveMustRemainTail { unit: HistoryUnitId , }, FinalViewContainsComponent { unit: HistoryUnitId , },`
- Variant paths: `iyon_tui::HistoryError::UnitNotFound`, `iyon_tui::HistoryError::UnitNotLive`, `iyon_tui::HistoryError::LiveMustRemainTail`, `iyon_tui::HistoryError::FinalViewContainsComponent`.

#### `iyon_tui::HorizontalAlign` — enum [re-export]
- Signature: `pub enum HorizontalAlign { Start, Center, End, }`
- Purpose: Horizontal alignment inside an allocated text track.
- Variants and fields: `Start, Center, End,`
- Variant paths: `iyon_tui::HorizontalAlign::Start`, `iyon_tui::HorizontalAlign::Center`, `iyon_tui::HorizontalAlign::End`.

#### `iyon_tui::InteractionResult` — enum [re-export]
- Signature: `pub enum InteractionResult { Ignored, Consumed, }`
- Purpose: Result of a generic component interaction.
- Variants and fields: `Ignored, Consumed,`
- Variant paths: `iyon_tui::InteractionResult::Ignored`, `iyon_tui::InteractionResult::Consumed`.

#### `iyon_tui::Key` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum Key { Char( char ), Enter, Escape, Backspace, Tab, Delete, Insert, Home, End, PageUp, PageDown, Up, Down, Left, Right, Function( u8 ), Null, CapsLock, ScrollLock, NumLock, PrintScreen, Pause, Menu, KeypadBegin, Media( MediaKey ), Modifier( ModifierKey ), }`
- Purpose: Backend-neutral key identity.
- Variants and fields: `Char( char ), Enter, Escape, Backspace, Tab, Delete, Insert, Home, End, PageUp, PageDown, Up, Down, Left, Right, Function( u8 ), Null, CapsLock, ScrollLock, NumLock, PrintScreen, Pause, Menu, KeypadBegin, Media( MediaKey ), Modifier( ModifierKey ),`
- Variant paths: `iyon_tui::Key::Char`, `iyon_tui::Key::Enter`, `iyon_tui::Key::Escape`, `iyon_tui::Key::Backspace`, `iyon_tui::Key::Tab`, `iyon_tui::Key::Delete`, `iyon_tui::Key::Insert`, `iyon_tui::Key::Home`, `iyon_tui::Key::End`, `iyon_tui::Key::PageUp`, `iyon_tui::Key::PageDown`, `iyon_tui::Key::Up`, `iyon_tui::Key::Down`, `iyon_tui::Key::Left`, `iyon_tui::Key::Right`, `iyon_tui::Key::Function`, `iyon_tui::Key::Null`, `iyon_tui::Key::CapsLock`, `iyon_tui::Key::ScrollLock`, `iyon_tui::Key::NumLock`, `iyon_tui::Key::PrintScreen`, `iyon_tui::Key::Pause`, `iyon_tui::Key::Menu`, `iyon_tui::Key::KeypadBegin`, `iyon_tui::Key::Media`, `iyon_tui::Key::Modifier`.

#### `iyon_tui::MediaKey` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum MediaKey { Play, Pause, PlayPause, Reverse, Stop, FastForward, Rewind, TrackNext, TrackPrevious, Record, LowerVolume, RaiseVolume, MuteVolume, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Play, Pause, PlayPause, Reverse, Stop, FastForward, Rewind, TrackNext, TrackPrevious, Record, LowerVolume, RaiseVolume, MuteVolume,`
- Variant paths: `iyon_tui::MediaKey::Play`, `iyon_tui::MediaKey::Pause`, `iyon_tui::MediaKey::PlayPause`, `iyon_tui::MediaKey::Reverse`, `iyon_tui::MediaKey::Stop`, `iyon_tui::MediaKey::FastForward`, `iyon_tui::MediaKey::Rewind`, `iyon_tui::MediaKey::TrackNext`, `iyon_tui::MediaKey::TrackPrevious`, `iyon_tui::MediaKey::Record`, `iyon_tui::MediaKey::LowerVolume`, `iyon_tui::MediaKey::RaiseVolume`, `iyon_tui::MediaKey::MuteVolume`.

#### `iyon_tui::ModifierKey` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ModifierKey { LeftShift, LeftControl, LeftAlt, LeftSuper, LeftHyper, LeftMeta, RightShift, RightControl, RightAlt, RightSuper, RightHyper, RightMeta, IsoLevel3Shift, IsoLevel5Shift, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `LeftShift, LeftControl, LeftAlt, LeftSuper, LeftHyper, LeftMeta, RightShift, RightControl, RightAlt, RightSuper, RightHyper, RightMeta, IsoLevel3Shift, IsoLevel5Shift,`
- Variant paths: `iyon_tui::ModifierKey::LeftShift`, `iyon_tui::ModifierKey::LeftControl`, `iyon_tui::ModifierKey::LeftAlt`, `iyon_tui::ModifierKey::LeftSuper`, `iyon_tui::ModifierKey::LeftHyper`, `iyon_tui::ModifierKey::LeftMeta`, `iyon_tui::ModifierKey::RightShift`, `iyon_tui::ModifierKey::RightControl`, `iyon_tui::ModifierKey::RightAlt`, `iyon_tui::ModifierKey::RightSuper`, `iyon_tui::ModifierKey::RightHyper`, `iyon_tui::ModifierKey::RightMeta`, `iyon_tui::ModifierKey::IsoLevel3Shift`, `iyon_tui::ModifierKey::IsoLevel5Shift`.

#### `iyon_tui::OverflowIndicator` — enum [re-export]
- Signature: `pub enum OverflowIndicator { None, Ellipsis { style: StyleRef , }, Footer { prefix: String , style: StyleRef , }, }`
- Purpose: Overflow treatment for a structurally clamped view.
- Variants and fields: `None, Ellipsis { style: StyleRef , }, Footer { prefix: String , style: StyleRef , },`
- Variant paths: `iyon_tui::OverflowIndicator::None`, `iyon_tui::OverflowIndicator::Ellipsis`, `iyon_tui::OverflowIndicator::Footer`.

#### `iyon_tui::RunError` — enum [re-export]
- Signature: `pub enum RunError<ApplicationError> { Application(ApplicationError), Runtime( RuntimeError ), }`
- Purpose: Failure returned by super::App::run .
- Variants and fields: `Application(ApplicationError), Runtime( RuntimeError ),`
- Variant paths: `iyon_tui::RunError::Application`, `iyon_tui::RunError::Runtime`.

#### `iyon_tui::SoftBreakPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum SoftBreakPolicy { Space, LineBreak, }`
- Purpose: How a soft line break is presented as ordinary semantic text.
- Variants and fields: `Space, LineBreak,`
- Variant paths: `iyon_tui::SoftBreakPolicy::Space`, `iyon_tui::SoftBreakPolicy::LineBreak`.

#### `iyon_tui::TableColumnSizing` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TableColumnSizing { Content, Flex, }`
- Purpose: Shared-column sizing for generic tables.
- Variants and fields: `Content, Flex,`
- Variant paths: `iyon_tui::TableColumnSizing::Content`, `iyon_tui::TableColumnSizing::Flex`.

#### `iyon_tui::TaskListMarkerPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TaskListMarkerPolicy { TaskOnly, TaskAndList, }`
- Purpose: How task-list items present checkbox chrome relative to the list marker.
- Variants and fields: `TaskOnly, TaskAndList,`
- Variant paths: `iyon_tui::TaskListMarkerPolicy::TaskOnly`, `iyon_tui::TaskListMarkerPolicy::TaskAndList`.

#### `iyon_tui::TextAttribute` — enum [re-export]
- Signature: `pub enum TextAttribute { Bold, Dim, Italic, Underline, Reversed, Strikethrough, }`
- Purpose: Selects a sparse semantic text attribute.
- Variants and fields: `Bold, Dim, Italic, Underline, Reversed, Strikethrough,`
- Variant paths: `iyon_tui::TextAttribute::Bold`, `iyon_tui::TextAttribute::Dim`, `iyon_tui::TextAttribute::Italic`, `iyon_tui::TextAttribute::Underline`, `iyon_tui::TextAttribute::Reversed`, `iyon_tui::TextAttribute::Strikethrough`.

#### `iyon_tui::TextContent` — enum [re-export]
- Signature: `pub enum TextContent { Raw( RawText ), Block( Block ), }`
- Purpose: The closed set of generic text projection values.
- Variants and fields: `Raw( RawText ), Block( Block ),`
- Variant paths: `iyon_tui::TextContent::Raw`, `iyon_tui::TextContent::Block`.
- `iyon_tui::TextContent::raw` — inherent method; `pub fn raw (text: impl Into < Arc < str >>) -> Self` — Provides the public `raw` operation.
- `iyon_tui::TextContent::block` — inherent method; `pub fn block (block: Block ) -> Self` — Provides the public `block` operation.

#### `iyon_tui::TextListKind` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextListKind { Bullet, Ordered, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Bullet, Ordered,`
- Variant paths: `iyon_tui::TextListKind::Bullet`, `iyon_tui::TextListKind::Ordered`.

#### `iyon_tui::TextPart` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextPart { ListMarker, TaskMarker, QuoteMarker, CodeLabel, TableRule, ThematicRule, ImageFallback, }`
- Purpose: Renderer-generated presentation pieces, distinct from semantic TextRole s.
- Variants and fields: `ListMarker, TaskMarker, QuoteMarker, CodeLabel, TableRule, ThematicRule, ImageFallback,`
- Variant paths: `iyon_tui::TextPart::ListMarker`, `iyon_tui::TextPart::TaskMarker`, `iyon_tui::TextPart::QuoteMarker`, `iyon_tui::TextPart::CodeLabel`, `iyon_tui::TextPart::TableRule`, `iyon_tui::TextPart::ThematicRule`, `iyon_tui::TextPart::ImageFallback`.

#### `iyon_tui::TextRole` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextRole { Paragraph, Heading, BlockQuote, List, ListItem, CodeBlock, Table, TableRow, TableCell, ThematicBreak, RawBlock, Container, Strong, Emphasis, Strikethrough, Underline, Superscript, Subscript, SmallCaps, InlineCode, Link, Image, RawInline, }`
- Purpose: Semantic classification used by generic structured-text styling.
- Variants and fields: `Paragraph, Heading, BlockQuote, List, ListItem, CodeBlock, Table, TableRow, TableCell, ThematicBreak, RawBlock, Container, Strong, Emphasis, Strikethrough, Underline, Superscript, Subscript, SmallCaps, InlineCode, Link, Image, RawInline,`
- Variant paths: `iyon_tui::TextRole::Paragraph`, `iyon_tui::TextRole::Heading`, `iyon_tui::TextRole::BlockQuote`, `iyon_tui::TextRole::List`, `iyon_tui::TextRole::ListItem`, `iyon_tui::TextRole::CodeBlock`, `iyon_tui::TextRole::Table`, `iyon_tui::TextRole::TableRow`, `iyon_tui::TextRole::TableCell`, `iyon_tui::TextRole::ThematicBreak`, `iyon_tui::TextRole::RawBlock`, `iyon_tui::TextRole::Container`, `iyon_tui::TextRole::Strong`, `iyon_tui::TextRole::Emphasis`, `iyon_tui::TextRole::Strikethrough`, `iyon_tui::TextRole::Underline`, `iyon_tui::TextRole::Superscript`, `iyon_tui::TextRole::Subscript`, `iyon_tui::TextRole::SmallCaps`, `iyon_tui::TextRole::InlineCode`, `iyon_tui::TextRole::Link`, `iyon_tui::TextRole::Image`, `iyon_tui::TextRole::RawInline`.

#### `iyon_tui::TextTableSection` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextTableSection { Header, Body, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Header, Body,`
- Variant paths: `iyon_tui::TextTableSection::Header`, `iyon_tui::TextTableSection::Body`.

#### `iyon_tui::TextTaskState` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextTaskState { Checked, Unchecked, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Checked, Unchecked,`
- Variant paths: `iyon_tui::TextTaskState::Checked`, `iyon_tui::TextTaskState::Unchecked`.

#### `iyon_tui::ThemeColor` — enum [re-export]
- Signature: `pub enum ThemeColor { Default, Named( AnsiColor ), Indexed( u8 ), Rgb { r: u8 , g: u8 , b: u8 , }, }`
- Purpose: A backend-neutral theme color.
- Variants and fields: `Default, Named( AnsiColor ), Indexed( u8 ), Rgb { r: u8 , g: u8 , b: u8 , },`
- Variant paths: `iyon_tui::ThemeColor::Default`, `iyon_tui::ThemeColor::Named`, `iyon_tui::ThemeColor::Indexed`, `iyon_tui::ThemeColor::Rgb`.

#### `iyon_tui::VerticalAlign` — enum [re-export]
- Signature: `pub enum VerticalAlign { Top, Center, Bottom, }`
- Purpose: Vertical alignment for children in a horizontal composition.
- Variants and fields: `Top, Center, Bottom,`
- Variant paths: `iyon_tui::VerticalAlign::Top`, `iyon_tui::VerticalAlign::Center`, `iyon_tui::VerticalAlign::Bottom`.

#### `iyon_tui::WrapMode` — enum [re-export]
- Signature: `pub enum WrapMode { WordThenGrapheme, Grapheme, NoWrap, }`
- Purpose: Text wrapping behavior for a typed text view.
- Variants and fields: `WordThenGrapheme, Grapheme, NoWrap,`
- Variant paths: `iyon_tui::WrapMode::WordThenGrapheme`, `iyon_tui::WrapMode::Grapheme`, `iyon_tui::WrapMode::NoWrap`.

#### `iyon_tui::App` — struct [re-export]
- Signature: `pub struct App<State, Action, Error, Init, Update, ViewFn> { /* private fields */ }`
- Purpose: A generic standalone application definition.
- `iyon_tui::App::new` — inherent method; `pub fn new (init: Init, update: Update, view: ViewFn) -> Self where Init: FnOnce (&mut AppCx <'_, Action>) -> Result <State, Error>, Update: FnMut ( &mut State , Action, &mut AppCx <'_, Action>) -> Result < () , Error>, ViewFn: Fn ( &State ) -> View ,` — Constructs a value.
- `iyon_tui::App::handle` — inherent method; `pub fn handle (&self) -> AppHandle <Action>` — Returns `handle`.
- `iyon_tui::App::with_theme` — inherent method; `pub fn with_theme (self, theme: Theme ) -> Self` — Returns this value with `theme` configured.
- `iyon_tui::App::run` — inherent method; `pub async fn run (self) -> Result < () , RunError <Error>> where Init: FnOnce (&mut AppCx <'_, Action>) -> Result <State, Error>, Update: FnMut ( &mut State , Action, &mut AppCx <'_, Action>) -> Result < () , Error>, ViewFn: Fn ( &State ) -> View ,` — Performs the requested application operation.
- `iyon_tui::App::with_history` — inherent method; `pub fn with_history (self, history: History ) -> Self` — Returns this value with `history` configured.

#### `iyon_tui::AppClosed` — struct [re-export]
- Signature: `pub struct AppClosed<Action> { /* private fields */ }`
- Purpose: Indicates that an asynchronous Action send reached a closed application.
- `iyon_tui::AppClosed::action` — inherent method; `pub fn action (&self) -> &Action` — Provides the public `action` operation.
- `iyon_tui::AppClosed::into_inner` — inherent method; `pub fn into_inner (self) -> Action` — Converts or exposes this value.

#### `iyon_tui::AppCx` — struct [re-export]
- Signature: `pub struct AppCx<'a, Action> { /* private fields */ }`
- Purpose: Public struct `AppCx`.
- `iyon_tui::AppCx::register` — inherent method; `pub fn register <C>(&mut self, component: C) -> ComponentHandle <C> where C: Component ,` — Provides the public `register` operation.
- `iyon_tui::AppCx::with_component` — inherent method; `pub fn with_component <C, R>( &self, handle: ComponentHandle <C>, access: impl FnOnce ( &C ) -> R, ) -> Option <R> where C: Component ,` — Returns this value with `component` configured.
- `iyon_tui::AppCx::with_component_mut` — inherent method; `pub fn with_component_mut <C, R>( &mut self, handle: ComponentHandle <C>, access: impl FnOnce ( &mut C ) -> R, ) -> Option <R> where C: Component ,` — Returns this value with `component_mut` configured.
- `iyon_tui::AppCx::remove_component` — inherent method; `pub fn remove_component <C>(&mut self, handle: ComponentHandle <C>) -> Option <C> where C: Component ,` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::route` — inherent method; `pub fn route <T: 'static>( &mut self, output: Output <T>, map: impl Fn (T) -> Action + 'static, ) -> Result < () , RouteConflict >` — Provides the public `route` operation.
- `iyon_tui::AppCx::remove_route` — inherent method; `pub fn remove_route <T: 'static>(&mut self, output: Output <T>) -> bool` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::history` — inherent method; `pub fn history (&self) -> Option <& History >` — Returns `history`.
- `iyon_tui::AppCx::history_mut` — inherent method; `pub fn history_mut (&mut self) -> Option <&mut History >` — Provides the public `history_mut` operation.
- `iyon_tui::AppCx::theme` — inherent method; `pub fn theme (&self) -> & Theme` — Returns `theme`.
- `iyon_tui::AppCx::theme_mut` — inherent method; `pub fn theme_mut (&mut self) -> &mut Theme` — Provides the public `theme_mut` operation.
- `iyon_tui::AppCx::now` — inherent method; `pub fn now (&self) -> Instant` — Returns `now`.
- `iyon_tui::AppCx::handle` — inherent method; `pub fn handle (&self) -> AppHandle <Action>` — Returns `handle`.
- `iyon_tui::AppCx::bind_key` — inherent method; `pub fn bind_key ( &mut self, key: KeyStroke , action: impl Fn () -> Action + 'static, )` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::unbind_key` — inherent method; `pub fn unbind_key (&mut self, key: KeyStroke ) -> bool` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::intercept_paste` — inherent method; `pub fn intercept_paste <C>( &mut self, component: ComponentHandle <C>, map: impl Fn ( String ) -> Action + 'static, ) where C: Component ,` — Provides the public `intercept_paste` operation.
- `iyon_tui::AppCx::remove_paste_interceptor` — inherent method; `pub fn remove_paste_interceptor <C>( &mut self, component: ComponentHandle <C>, ) -> bool where C: Component ,` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::forward_paste` — inherent method; `pub fn forward_paste (&mut self, text: impl Into < String >)` — Performs the requested application operation.
- `iyon_tui::AppCx::schedule_after` — inherent method; `pub fn schedule_after (&mut self, delay: Duration , action: Action) -> TimerHandle` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::cancel_timer` — inherent method; `pub fn cancel_timer (&mut self, handle: TimerHandle ) -> bool` — Manages the requested registration or scheduled action.
- `iyon_tui::AppCx::exit` — inherent method; `pub fn exit (&mut self)` — Performs the requested application operation.

#### `iyon_tui::AppHandle` — struct [re-export]
- Signature: `pub struct AppHandle<Action> { /* private fields */ }`
- Purpose: A cloneable producer of application Actions.
- `iyon_tui::AppHandle::send` — inherent method; `pub fn send (&self, action: Action) -> Result < () , AppSendError <Action>>` — Performs the requested application operation.
- `iyon_tui::AppHandle::send_async` — inherent method; `pub async fn send_async (&self, action: Action) -> Result < () , AppClosed <Action>>` — Performs the requested application operation.

#### `iyon_tui::Block` — struct [re-export]
- Signature: `pub struct Block( /* private fields */ );`
- Purpose: Public struct `Block`.
- `iyon_tui::Block::new` — inherent method; `pub fn new (kind: BlockKind ) -> Self` — Constructs a value.
- `iyon_tui::Block::paragraph` — inherent method; `pub fn paragraph (content: impl Into < InlineContent >) -> Self` — Provides the public `paragraph` operation.
- `iyon_tui::Block::heading` — inherent method; `pub fn heading (level: HeadingLevel , content: impl Into < InlineContent >) -> Self` — Provides the public `heading` operation.
- `iyon_tui::Block::block_quote` — inherent method; `pub fn block_quote (blocks: impl IntoIterator <Item = Block >) -> Self` — Provides the public `block_quote` operation.
- `iyon_tui::Block::list` — inherent method; `pub fn list (list: List ) -> Self` — Provides the public `list` operation.
- `iyon_tui::Block::code` — inherent method; `pub fn code (code: CodeBlock ) -> Self` — Provides the public `code` operation.
- `iyon_tui::Block::table` — inherent method; `pub fn table (table: Table ) -> Self` — Provides the public `table` operation.
- `iyon_tui::Block::thematic_break` — inherent method; `pub fn thematic_break () -> Self` — Provides the public `thematic_break` operation.
- `iyon_tui::Block::raw` — inherent method; `pub fn raw (format: FormatId , body: LiteralText ) -> Self` — Provides the public `raw` operation.
- `iyon_tui::Block::container` — inherent method; `pub fn container (blocks: impl IntoIterator <Item = Block >) -> Self` — Provides the public `container` operation.
- `iyon_tui::Block::kind` — inherent method; `pub fn kind (&self) -> & BlockKind` — Returns `kind`.
- `iyon_tui::Block::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::Block::with_annotations` — inherent method; `pub fn with_annotations (&self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::Block::map_annotations` — inherent method; `pub fn map_annotations ( &self, map: impl FnOnce ( Annotations ) -> Annotations , ) -> Self` — Maps or rewrites the contained semantic value.
- `iyon_tui::Block::as_code_block` — inherent method; `pub fn as_code_block (&self) -> Option <& CodeBlock >` — Converts or exposes this value.
- `iyon_tui::Block::as_list` — inherent method; `pub fn as_list (&self) -> Option <& List >` — Converts or exposes this value.
- `iyon_tui::Block::as_container` — inherent method; `pub fn as_container (&self) -> Option <&[ Block ]>` — Converts or exposes this value.
- `iyon_tui::Block::ptr_eq` — inherent method; `pub fn ptr_eq (&self, other: &Self) -> bool` — Provides the public `ptr_eq` operation.
- `iyon_tui::Block::with_origin` — inherent method; `pub fn with_origin (&self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::Block::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::BorderEdges` — struct [re-export]
- Signature: `pub struct BorderEdges { /* private fields */ }`
- Purpose: Which sides of a semantic border are painted.
- `iyon_tui::BorderEdges::NONE` — associated const; `pub const NONE : Self` — Provides the public `NONE` operation.
- `iyon_tui::BorderEdges::ALL` — associated const; `pub const ALL : Self` — Provides the public `ALL` operation.
- `iyon_tui::BorderEdges::TOP_BOTTOM` — associated const; `pub const TOP_BOTTOM : Self` — Provides the public `TOP_BOTTOM` operation.
- `iyon_tui::BorderEdges::new` — inherent method; `pub const fn new (top: bool , right: bool , bottom: bool , left: bool ) -> Self` — Constructs a value.

#### `iyon_tui::BorderGlyphError` — struct [re-export]
- Signature: `pub struct BorderGlyphError { pub field: &'static str , pub width: usize , pub graphemes: usize , }`
- Purpose: Failure to construct a border glyph that occupies exactly one cell.

#### `iyon_tui::BorderGlyphs` — struct [re-export]
- Signature: `pub struct BorderGlyphs { /* private fields */ }`
- Purpose: Custom one-cell border glyphs.
- `iyon_tui::BorderGlyphs::new` — inherent method; `pub fn new ( top: impl Into < String >, right: impl Into < String >, bottom: impl Into < String >, left: impl Into < String >, top_left: impl Into < String >, top_right: impl Into < String >, bottom_left: impl Into < String >, bottom_right: impl Into < String >, ) -> Result <Self, BorderGlyphError >` — Constructs a value.

#### `iyon_tui::BorderSpec` — struct [re-export]
- Signature: `pub struct BorderSpec { /* private fields */ }`
- Purpose: Backend-neutral border description.
- `iyon_tui::BorderSpec::plain` — inherent method; `pub fn plain () -> Self` — Provides the public `plain` operation.
- `iyon_tui::BorderSpec::rounded` — inherent method; `pub fn rounded () -> Self` — Provides the public `rounded` operation.
- `iyon_tui::BorderSpec::double` — inherent method; `pub fn double () -> Self` — Provides the public `double` operation.
- `iyon_tui::BorderSpec::custom` — inherent method; `pub fn custom (glyphs: BorderGlyphs ) -> Self` — Provides the public `custom` operation.
- `iyon_tui::BorderSpec::edges` — inherent method; `pub fn edges (self, edges: BorderEdges ) -> Self` — Provides the public `edges` operation.
- `iyon_tui::BorderSpec::color` — inherent method; `pub fn color (self, color: ColorSpec ) -> Self` — Provides the public `color` operation.
- `iyon_tui::BorderSpec::top_label` — inherent method; `pub fn top_label (self, label: impl Into < String >) -> Self` — Provides the public `top_label` operation.

#### `iyon_tui::ComponentCx` — struct [re-export]
- Signature: `pub struct ComponentCx<'a, C> { /* private fields */ }`
- Purpose: Ephemeral capability declaration context for a mounted component.
- `iyon_tui::ComponentCx::focusable` — inherent method; `pub fn focusable (&mut self)` — Provides the public `focusable` operation.
- `iyon_tui::ComponentCx::modal_scope` — inherent method; `pub fn modal_scope (&mut self)` — Provides the public `modal_scope` operation.
- `iyon_tui::ComponentCx::on_focus_changed` — inherent method; `pub fn on_focus_changed (&mut self, handler: fn ( &mut C , bool )) where C: 'static,` — Provides the public `on_focus_changed` operation.
- `iyon_tui::ComponentCx::on_paste` — inherent method; `pub fn on_paste ( &mut self, handler: for<'paste, 'event> fn ( &mut C , &'paste str , &mut EventCx <'event>) -> InteractionResult , ) where C: 'static,` — Provides the public `on_paste` operation.
- `iyon_tui::ComponentCx::key_commands` — inherent method; `pub fn key_commands <Command: 'static>( &mut self, map: fn ( &C , KeyStroke ) -> Option <Command>, handle: for<'event> fn ( &mut C , Command, &mut EventCx <'event>) -> InteractionResult , ) where C: 'static,` — Performs the requested application operation.
- `iyon_tui::ComponentCx::tick` — inherent method; `pub fn tick ( &mut self, interval: Duration , handler: for<'event> fn ( &mut C , Instant , &mut EventCx <'event>) -> bool , ) where C: 'static,` — Provides the public `tick` operation.

#### `iyon_tui::ComponentHandle` — struct [re-export]
- Signature: `pub struct ComponentHandle<C> { /* private fields */ }`
- Purpose: Typed, non-owning identity for a component in a registry.

#### `iyon_tui::DiffHunk` — struct [re-export]
- Signature: `pub struct DiffHunk { /* private fields */ }`
- Purpose: A validated pair of old/new source ranges and its semantic diff lines.
- `iyon_tui::DiffHunk::new` — inherent method; `pub fn new ( old: DiffRange , new_range: DiffRange , lines: impl IntoIterator <Item = DiffLine >, ) -> Result <Self, DiffValidationError >` — Constructs a value.
- `iyon_tui::DiffHunk::old_range` — inherent method; `pub const fn old_range (&self) -> DiffRange` — Provides the public `old_range` operation.
- `iyon_tui::DiffHunk::new_range` — inherent method; `pub const fn new_range (&self) -> DiffRange` — Provides the public `new_range` operation.
- `iyon_tui::DiffHunk::lines` — inherent method; `pub fn lines (&self) -> &[ DiffLine ]` — Provides the public `lines` operation.
- `iyon_tui::DiffHunk::validate` — inherent method; `pub fn validate (&self) -> Result < () , DiffValidationError >` — Performs the fallible validation or operation.

#### `iyon_tui::DiffLine` — struct [re-export]
- Signature: `pub struct DiffLine { /* private fields */ }`
- Purpose: A semantic diff line.
- `iyon_tui::DiffLine::context` — inherent method; `pub fn context ( old_line: DiffLineNumber , new_line: DiffLineNumber , text: impl Into < Arc < str >>, ) -> Self` — Provides the public `context` operation.
- `iyon_tui::DiffLine::addition` — inherent method; `pub fn addition (new_line: DiffLineNumber , text: impl Into < Arc < str >>) -> Self` — Provides the public `addition` operation.
- `iyon_tui::DiffLine::deletion` — inherent method; `pub fn deletion (old_line: DiffLineNumber , text: impl Into < Arc < str >>) -> Self` — Provides the public `deletion` operation.
- `iyon_tui::DiffLine::kind` — inherent method; `pub const fn kind (&self) -> DiffLineKind` — Returns `kind`.
- `iyon_tui::DiffLine::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::DiffLine::old_line` — inherent method; `pub const fn old_line (&self) -> Option < DiffLineNumber >` — Provides the public `old_line` operation.
- `iyon_tui::DiffLine::new_line` — inherent method; `pub const fn new_line (&self) -> Option < DiffLineNumber >` — Provides the public `new_line` operation.
- `iyon_tui::DiffLine::termination` — inherent method; `pub const fn termination (&self) -> DiffLineTermination` — Provides the public `termination` operation.
- `iyon_tui::DiffLine::with_termination` — inherent method; `pub fn with_termination (self, termination: DiffLineTermination ) -> Self` — Returns this value with `termination` configured.

#### `iyon_tui::DiffLineNumber` — struct [re-export]
- Signature: `pub struct DiffLineNumber( /* private fields */ );`
- Purpose: A one-based concrete source line number.
- `iyon_tui::DiffLineNumber::new` — inherent method; `pub const fn new (line: u64 ) -> Option <Self>` — Constructs a value.
- `iyon_tui::DiffLineNumber::as_u64` — inherent method; `pub const fn as_u64 (self) -> u64` — Converts or exposes this value.

#### `iyon_tui::DiffLineOffset` — struct [re-export]
- Signature: `pub struct DiffLineOffset( /* private fields */ );`
- Purpose: A zero-based source line boundary.
- `iyon_tui::DiffLineOffset::ZERO` — associated const; `pub const ZERO : Self` — Provides the public `ZERO` operation.
- `iyon_tui::DiffLineOffset::new` — inherent method; `pub const fn new (offset: u64 ) -> Self` — Constructs a value.
- `iyon_tui::DiffLineOffset::as_u64` — inherent method; `pub const fn as_u64 (self) -> u64` — Converts or exposes this value.
- `iyon_tui::DiffLineOffset::checked_add` — inherent method; `pub const fn checked_add (self, rhs: u64 ) -> Option <Self>` — Performs bounded arithmetic.
- `iyon_tui::DiffLineOffset::saturating_add` — inherent method; `pub const fn saturating_add (self, rhs: u64 ) -> Self` — Performs bounded arithmetic.

#### `iyon_tui::DiffRange` — struct [re-export]
- Signature: `pub struct DiffRange { /* private fields */ }`
- Purpose: A contiguous range of source lines, represented by its start boundary and number of lines.
- `iyon_tui::DiffRange::new` — inherent method; `pub fn new ( start: DiffLineOffset , line_count: u64 , ) -> Result <Self, DiffValidationError >` — Constructs a value.
- `iyon_tui::DiffRange::start` — inherent method; `pub const fn start (&self) -> DiffLineOffset` — Performs the requested application operation.
- `iyon_tui::DiffRange::line_count` — inherent method; `pub const fn line_count (&self) -> u64` — Provides the public `line_count` operation.
- `iyon_tui::DiffRange::is_empty` — inherent method; `pub const fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::DiffRange::end` — inherent method; `pub fn end (&self) -> DiffLineOffset` — Provides the public `end` operation.

#### `iyon_tui::DiffRenderer` — struct [re-export]
- Signature: `pub struct DiffRenderer;`
- Purpose: Lowers validated semantic diff hunks into geometry-independent Views.
- `iyon_tui::DiffRenderer::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::DiffRenderer::render_hunk` — inherent method; `pub fn render_hunk (&self, hunk: & DiffHunk ) -> View` — Renders the semantic value.

#### `iyon_tui::EventCx` — struct [re-export]
- Signature: `pub struct EventCx<'a> { /* private fields */ }`
- Purpose: Public struct `EventCx`.
- `iyon_tui::EventCx::emit` — inherent method; `pub fn emit <T: 'static>(&mut self, output: Output <T>, value: T)` — Builds or validates the requested projection.

#### `iyon_tui::Grid` — struct [re-export]
- Signature: `pub struct Grid { /* private fields */ }`
- Purpose: Closure-scoped capability for constructing two-dimensional composition.
- `iyon_tui::Grid::columns` — inherent method; `pub fn columns ( &mut self, columns: impl IntoIterator <Item = GridTrack >, ) -> &mut Self` — Returns `columns`.
- `iyon_tui::Grid::column_gap` — inherent method; `pub fn column_gap (&mut self, gap: u16 ) -> &mut Self` — Provides the public `column_gap` operation.
- `iyon_tui::Grid::row_gap` — inherent method; `pub fn row_gap (&mut self, gap: u16 ) -> &mut Self` — Provides the public `row_gap` operation.
- `iyon_tui::Grid::row` — inherent method; `pub fn row (&mut self, build: impl FnOnce (&mut GridRow )) -> &mut Self` — Provides the public `row` operation.
- `iyon_tui::Grid::row_with` — inherent method; `pub fn row_with ( &mut self, track: GridTrack , build: impl FnOnce (&mut GridRow ), ) -> &mut Self` — Provides the public `row_with` operation.

#### `iyon_tui::GridCellSpec` — struct [re-export]
- Signature: `pub struct GridCellSpec { /* private fields */ }`
- Purpose: Placement and alignment for one grid cell.
- `iyon_tui::GridCellSpec::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::GridCellSpec::column_span` — inherent method; `pub fn column_span (self, span: u16 ) -> Self` — Provides the public `column_span` operation.
- `iyon_tui::GridCellSpec::row_span` — inherent method; `pub fn row_span (self, span: u16 ) -> Self` — Returns `row_span`.
- `iyon_tui::GridCellSpec::horizontal_align` — inherent method; `pub fn horizontal_align (self, align: HorizontalAlign ) -> Self` — Provides the public `horizontal_align` operation.
- `iyon_tui::GridCellSpec::vertical_align` — inherent method; `pub fn vertical_align (self, align: VerticalAlign ) -> Self` — Provides the public `vertical_align` operation.

#### `iyon_tui::GridRow` — struct [re-export]
- Signature: `pub struct GridRow { /* private fields */ }`
- Purpose: Closure-scoped capability for constructing one grid source row.
- `iyon_tui::GridRow::cell` — inherent method; `pub fn cell (&mut self, child: impl IntoView ) -> &mut Self` — Provides the public `cell` operation.
- `iyon_tui::GridRow::cell_with` — inherent method; `pub fn cell_with ( &mut self, spec: GridCellSpec , child: impl IntoView , ) -> &mut Self` — Returns compiled or painted information.

#### `iyon_tui::GridTrack` — struct [re-export]
- Signature: `pub struct GridTrack { /* private fields */ }`
- Purpose: A column or row track size.
- `iyon_tui::GridTrack::content` — inherent method; `pub const fn content () -> Self` — Provides the public `content` operation.
- `iyon_tui::GridTrack::content_max` — inherent method; `pub const fn content_max (max: u16 ) -> Self` — Provides the public `content_max` operation.
- `iyon_tui::GridTrack::fixed` — inherent method; `pub const fn fixed (size: u16 ) -> Self` — Provides the public `fixed` operation.
- `iyon_tui::GridTrack::flex` — inherent method; `pub const fn flex () -> Self` — Provides the public `flex` operation.
- `iyon_tui::GridTrack::flex_max` — inherent method; `pub const fn flex_max (max: u16 ) -> Self` — Provides the public `flex_max` operation.

#### `iyon_tui::HeadingLevel` — struct [re-export]
- Signature: `pub struct HeadingLevel( /* private fields */ );`
- Purpose: Validated heading levels.
- `iyon_tui::HeadingLevel::H1` — associated const; `pub const H1 : Self` — Provides the public `H1` operation.
- `iyon_tui::HeadingLevel::H2` — associated const; `pub const H2 : Self` — Provides the public `H2` operation.
- `iyon_tui::HeadingLevel::H3` — associated const; `pub const H3 : Self` — Provides the public `H3` operation.
- `iyon_tui::HeadingLevel::H4` — associated const; `pub const H4 : Self` — Provides the public `H4` operation.
- `iyon_tui::HeadingLevel::H5` — associated const; `pub const H5 : Self` — Provides the public `H5` operation.
- `iyon_tui::HeadingLevel::H6` — associated const; `pub const H6 : Self` — Provides the public `H6` operation.
- `iyon_tui::HeadingLevel::new` — inherent method; `pub fn new (level: u8 ) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::HeadingLevel::get` — inherent method; `pub fn get (self) -> u8` — Provides the public `get` operation.

#### `iyon_tui::History` — struct [re-export]
- Signature: `pub struct History { /* private fields */ }`
- Purpose: An ordered root-level historical/live semantic flow.
- `iyon_tui::History::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::History::push` — inherent method; `pub fn push ( &mut self, view: impl IntoView , ) -> Result < HistoryUnitId , HistoryError >` — Provides the public `push` operation.
- `iyon_tui::History::push_with_boundary` — inherent method; `pub fn push_with_boundary ( &mut self, view: impl IntoView , boundary: FlowBoundary , ) -> Result < HistoryUnitId , HistoryError >` — Provides the public `push_with_boundary` operation.
- `iyon_tui::History::discard_live` — inherent method; `pub fn discard_live (&mut self, unit: HistoryUnitId ) -> Result < () , HistoryError >` — Provides the public `discard_live` operation.
- `iyon_tui::History::freeze` — inherent method; `pub fn freeze ( &mut self, unit: HistoryUnitId , final_view: impl IntoView , ) -> Result < () , HistoryError >` — Provides the public `freeze` operation.
- `iyon_tui::History::layout` — inherent method; `pub fn layout (&self) -> HistoryLayout` — Provides the public `layout` operation.
- `iyon_tui::History::set_layout` — inherent method; `pub fn set_layout (&mut self, layout: HistoryLayout )` — Sets `layout` and returns the previous value when applicable.
- `iyon_tui::History::with_layout` — inherent method; `pub fn with_layout (self, layout: HistoryLayout ) -> Self` — Returns this value with `layout` configured.

#### `iyon_tui::HistoryLayout` — struct [re-export]
- Signature: `pub struct HistoryLayout { /* private fields */ }`
- Purpose: Semantic layout configuration for one History flow.
- `iyon_tui::HistoryLayout::new` — inherent method; `pub const fn new () -> Self` — Constructs a value.
- `iyon_tui::HistoryLayout::from_parts` — inherent method; `pub const fn from_parts (padding: Insets , gap: u16 ) -> Self` — Provides the public `from_parts` operation.
- `iyon_tui::HistoryLayout::with_padding` — inherent method; `pub const fn with_padding (self, padding: Insets ) -> Self` — Returns this value with `padding` configured.
- `iyon_tui::HistoryLayout::with_gap` — inherent method; `pub const fn with_gap (self, gap: u16 ) -> Self` — Returns this value with `gap` configured.
- `iyon_tui::HistoryLayout::padding` — inherent method; `pub const fn padding (self) -> Insets` — Provides the public `padding` operation.
- `iyon_tui::HistoryLayout::gap` — inherent method; `pub const fn gap (self) -> u16` — Provides the public `gap` operation.

#### `iyon_tui::HistoryUnitId` — struct [re-export]
- Signature: `pub struct HistoryUnitId( /* private fields */ );`
- Purpose: Stable identity for one unit in one process’s generic History namespace.

#### `iyon_tui::Horizontal` — struct [re-export]
- Signature: `pub struct Horizontal { /* private fields */ }`
- Purpose: Closure-scoped capability for constructing horizontal semantic composition.
- `iyon_tui::Horizontal::child` — inherent method; `pub fn child (&mut self, child: impl IntoView ) -> &mut Self` — Provides the public `child` operation.
- `iyon_tui::Horizontal::children` — inherent method; `pub fn children <I, V>(&mut self, children: I) -> &mut Self where I: IntoIterator <Item = V>, V: IntoView ,` — Provides the public `children` operation.
- `iyon_tui::Horizontal::fixed` — inherent method; `pub fn fixed (&mut self, width: u16 , child: impl IntoView ) -> &mut Self` — Provides the public `fixed` operation.
- `iyon_tui::Horizontal::flex` — inherent method; `pub fn flex (&mut self, child: impl IntoView ) -> &mut Self` — Provides the public `flex` operation.
- `iyon_tui::Horizontal::gap` — inherent method; `pub fn gap (&mut self, gap: u16 ) -> &mut Self` — Provides the public `gap` operation.
- `iyon_tui::Horizontal::vertical_align` — inherent method; `pub fn vertical_align (&mut self, align: VerticalAlign ) -> &mut Self` — Provides the public `vertical_align` operation.

#### `iyon_tui::Inline` — struct [re-export]
- Signature: `pub struct Inline( /* private fields */ );`
- Purpose: Immutable inline semantic value.
- `iyon_tui::Inline::new` — inherent method; `pub fn new (kind: InlineKind ) -> Self` — Constructs a value.
- `iyon_tui::Inline::text` — inherent method; `pub fn text (run: impl Into < TextRun >) -> Self` — Returns `text`.
- `iyon_tui::Inline::break_` — inherent method; `pub fn break_ (kind: BreakKind ) -> Self` — Provides the public `break_` operation.
- `iyon_tui::Inline::image` — inherent method; `pub fn image (image: Image ) -> Self` — Provides the public `image` operation.
- `iyon_tui::Inline::raw` — inherent method; `pub fn raw (format: FormatId , body: LiteralText ) -> Self` — Provides the public `raw` operation.
- `iyon_tui::Inline::kind` — inherent method; `pub fn kind (&self) -> & InlineKind` — Returns `kind`.
- `iyon_tui::Inline::marks` — inherent method; `pub fn marks (&self) -> & MarkSet` — Returns `marks`.
- `iyon_tui::Inline::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::Inline::as_text` — inherent method; `pub fn as_text (&self) -> Option <& TextRun >` — Converts or exposes this value.
- `iyon_tui::Inline::with_mark` — inherent method; `pub fn with_mark (&self, mark: Mark ) -> Result <Self, TextIrError >` — Returns this value with `mark` configured.
- `iyon_tui::Inline::strong` — inherent method; `pub fn strong (&self) -> Self` — Provides the public `strong` operation.
- `iyon_tui::Inline::emphasis` — inherent method; `pub fn emphasis (&self) -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::Inline::strikethrough` — inherent method; `pub fn strikethrough (&self) -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::Inline::underline` — inherent method; `pub fn underline (&self) -> Self` — Provides the public `underline` operation.
- `iyon_tui::Inline::code` — inherent method; `pub fn code (&self) -> Self` — Provides the public `code` operation.
- `iyon_tui::Inline::with_link` — inherent method; `pub fn with_link (&self, target: LinkTarget ) -> Result <Self, TextIrError >` — Returns this value with `link` configured.
- `iyon_tui::Inline::with_marks` — inherent method; `pub fn with_marks (&self, marks: MarkSet ) -> Self` — Returns this value with `marks` configured.
- `iyon_tui::Inline::with_annotations` — inherent method; `pub fn with_annotations (&self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::Inline::ptr_eq` — inherent method; `pub fn ptr_eq (&self, other: &Self) -> bool` — Provides the public `ptr_eq` operation.
- `iyon_tui::Inline::map_annotations` — inherent method; `pub fn map_annotations ( &self, map: impl FnOnce ( Annotations ) -> Annotations , ) -> Self` — Maps or rewrites the contained semantic value.
- `iyon_tui::Inline::with_origin` — inherent method; `pub fn with_origin (&self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::Inline::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::InlineContent` — struct [re-export]
- Signature: `pub struct InlineContent { /* private fields */ }`
- Purpose: Immutable ordered inline content.
- `iyon_tui::InlineContent::new` — inherent method; `pub fn new (items: impl IntoIterator <Item = Inline >) -> Self` — Constructs a value.
- `iyon_tui::InlineContent::empty` — inherent method; `pub fn empty () -> Self` — Constructs a value.
- `iyon_tui::InlineContent::items` — inherent method; `pub fn items (&self) -> &[ Inline ]` — Returns `items`.
- `iyon_tui::InlineContent::iter` — inherent method; `pub fn iter (&self) -> impl Iterator <Item = & Inline >` — Returns `iter`.
- `iyon_tui::InlineContent::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::InlineContent::len` — inherent method; `pub fn len (&self) -> usize` — Returns the number of contained items.
- `iyon_tui::InlineContent::with_mark` — inherent method; `pub fn with_mark (&self, mark: Mark ) -> Result <Self, TextIrError >` — Returns this value with `mark` configured.
- `iyon_tui::InlineContent::strong` — inherent method; `pub fn strong (&self) -> Self` — Provides the public `strong` operation.
- `iyon_tui::InlineContent::emphasis` — inherent method; `pub fn emphasis (&self) -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::InlineContent::code` — inherent method; `pub fn code (&self) -> Self` — Provides the public `code` operation.

#### `iyon_tui::Insets` — struct [re-export]
- Signature: `pub struct Insets { /* private fields */ }`
- Purpose: Insets applied to a semantic view’s surface.
- `iyon_tui::Insets::ZERO` — associated const; `pub const ZERO : Self` — Provides the public `ZERO` operation.
- `iyon_tui::Insets::all` — inherent method; `pub const fn all (value: u16 ) -> Self` — Provides the public `all` operation.
- `iyon_tui::Insets::vertical` — inherent method; `pub const fn vertical (value: u16 ) -> Self` — Provides the public `vertical` operation.
- `iyon_tui::Insets::horizontal` — inherent method; `pub const fn horizontal (value: u16 ) -> Self` — Provides the public `horizontal` operation.
- `iyon_tui::Insets::new` — inherent method; `pub const fn new (top: u16 , right: u16 , bottom: u16 , left: u16 ) -> Self` — Constructs a value.

#### `iyon_tui::KeyStroke` — struct [re-export]
- Signature: `pub struct KeyStroke { /* private fields */ }`
- Purpose: A normalized keyboard actuation used by framework command routing.
- `iyon_tui::KeyStroke::new` — inherent method; `pub const fn new (key: Key ) -> Self` — Constructs a value.
- `iyon_tui::KeyStroke::with_modifiers` — inherent method; `pub const fn with_modifiers (key: Key , modifiers: Modifiers ) -> Self` — Returns this value with `modifiers` configured.
- `iyon_tui::KeyStroke::key` — inherent method; `pub const fn key (self) -> Key` — Returns `key`.
- `iyon_tui::KeyStroke::modifiers` — inherent method; `pub const fn modifiers (self) -> Modifiers` — Returns `modifiers`.

#### `iyon_tui::MarkdownOptions` — struct [re-export]
- Signature: `pub struct MarkdownOptions { /* private fields */ }`
- Purpose: Explicitly selected Markdown extensions supported by super::MarkdownProjector .
- `iyon_tui::MarkdownOptions::commonmark` — inherent method; `pub const fn commonmark () -> Self` — Constructs a value.
- `iyon_tui::MarkdownOptions::gfm` — inherent method; `pub const fn gfm () -> Self` — Constructs a value.
- `iyon_tui::MarkdownOptions::with_tables` — inherent method; `pub const fn with_tables (self, enabled: bool ) -> Self` — Returns this value with `tables` configured.
- `iyon_tui::MarkdownOptions::with_strikethrough` — inherent method; `pub const fn with_strikethrough (self, enabled: bool ) -> Self` — Returns this value with `strikethrough` configured.
- `iyon_tui::MarkdownOptions::with_task_lists` — inherent method; `pub const fn with_task_lists (self, enabled: bool ) -> Self` — Returns this value with `task_lists` configured.
- `iyon_tui::MarkdownOptions::with_live_table_stabilization` — inherent method; `pub const fn with_live_table_stabilization (self, enabled: bool ) -> Self` — Returns this value with `live_table_stabilization` configured.
- `iyon_tui::MarkdownOptions::tables` — inherent method; `pub const fn tables (self) -> bool` — Provides the public `tables` operation.
- `iyon_tui::MarkdownOptions::strikethrough` — inherent method; `pub const fn strikethrough (self) -> bool` — Provides the public `strikethrough` operation.
- `iyon_tui::MarkdownOptions::task_lists` — inherent method; `pub const fn task_lists (self) -> bool` — Provides the public `task_lists` operation.
- `iyon_tui::MarkdownOptions::live_table_stabilization` — inherent method; `pub const fn live_table_stabilization (self) -> bool` — Provides the public `live_table_stabilization` operation.

#### `iyon_tui::MarkdownProjector` — struct [re-export]
- Signature: `pub struct MarkdownProjector { /* private fields */ }`
- Purpose: Stateful, non-temporal CommonMark-to-TextContent projector.
- `iyon_tui::MarkdownProjector::new` — inherent method; `pub fn new (options: MarkdownOptions ) -> Self` — Constructs a value.
- `iyon_tui::MarkdownProjector::options` — inherent method; `pub fn options (&self) -> MarkdownOptions` — Returns `options`.
- `iyon_tui::MarkdownProjector::parser_work` — inherent method [feature `test-util`; doc-hidden]; `pub fn parser_work (&self) -> (usize, usize)` — Returns parser invocation and byte-work counters for tests.

#### `iyon_tui::Modifiers` — struct [re-export]
- Signature: `pub struct Modifiers { /* private fields */ }`
- Purpose: Framework-owned keyboard modifier bitset.
- `iyon_tui::Modifiers::NONE` — associated const; `pub const NONE : Self` — Provides the public `NONE` operation.
- `iyon_tui::Modifiers::SHIFT` — associated const; `pub const SHIFT : Self` — Provides the public `SHIFT` operation.
- `iyon_tui::Modifiers::CONTROL` — associated const; `pub const CONTROL : Self` — Provides the public `CONTROL` operation.
- `iyon_tui::Modifiers::ALT` — associated const; `pub const ALT : Self` — Returns `alt`.
- `iyon_tui::Modifiers::SUPER` — associated const; `pub const SUPER : Self` — Provides the public `SUPER` operation.
- `iyon_tui::Modifiers::HYPER` — associated const; `pub const HYPER : Self` — Provides the public `HYPER` operation.
- `iyon_tui::Modifiers::META` — associated const; `pub const META : Self` — Provides the public `META` operation.
- `iyon_tui::Modifiers::contains` — inherent method; `pub const fn contains (self, modifiers: Self) -> bool` — Reports whether the value contains the requested item.
- `iyon_tui::Modifiers::union` — inherent method; `pub const fn union (self, modifiers: Self) -> Self` — Provides the public `union` operation.

#### `iyon_tui::Output` — struct [re-export]
- Signature: `pub struct Output<T: 'static> { /* private fields */ }`
- Purpose: Opaque typed identity for a semantic output channel.
- `iyon_tui::Output::new` — inherent method; `pub fn new () -> Self` — Constructs a value.

#### `iyon_tui::OutputRouter` — struct [re-export]
- Signature: `pub struct OutputRouter<A> { /* private fields */ }`
- Purpose: Public struct `OutputRouter`.
- `iyon_tui::OutputRouter::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::OutputRouter::route` — inherent method; `pub fn route <T: 'static>( &mut self, output: Output <T>, map: impl Fn (T) -> A + 'static, ) -> Result < () , RouteConflict >` — Provides the public `route` operation.
- `iyon_tui::OutputRouter::remove` — inherent method; `pub fn remove <T: 'static>(&mut self, output: Output <T>) -> bool` — Manages the requested registration or scheduled action.

#### `iyon_tui::PlainTextProjector` — struct [re-export]
- Signature: `pub struct PlainTextProjector;`
- Purpose: A projector that claims each consecutive Raw domain as literal prose.
- `iyon_tui::PlainTextProjector::new` — inherent method; `pub fn new () -> Self` — Constructs a value.

#### `iyon_tui::RawText` — struct [re-export]
- Signature: `pub struct RawText( /* private fields */ );`
- Purpose: Exact, unclaimed text at the root of a text projection.
- `iyon_tui::RawText::new` — inherent method; `pub fn new (text: impl Into < Arc < str >>) -> Self` — Constructs a value.
- `iyon_tui::RawText::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::RawText::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::RawText::len` — inherent method; `pub fn len (&self) -> usize` — Returns the number of contained items.
- `iyon_tui::RawText::exact_slice` — inherent method; `pub fn exact_slice ( &self, owner: StreamRange , local: Range < usize >, ) -> Result < TextRun , TextIrError >` — Provides the public `exact_slice` operation.

#### `iyon_tui::RouteConflict` — struct [re-export]
- Signature: `pub struct RouteConflict;`
- Purpose: Failure to add a second route for an output channel.

#### `iyon_tui::RuntimeError` — struct [re-export]
- Signature: `pub struct RuntimeError { /* private fields */ }`
- Purpose: Opaque runtime failure.

#### `iyon_tui::Scene` — struct [re-export]
- Signature: `pub struct Scene { /* private fields */ }`
- Purpose: The semantic root of a terminal application.
- `iyon_tui::Scene::new` — inherent method; `pub fn new (body: impl IntoView ) -> Self` — Constructs a value.
- `iyon_tui::Scene::with_history` — inherent method; `pub fn with_history (history: History , body: impl IntoView ) -> Self` — Returns this value with `history` configured.
- `iyon_tui::Scene::history` — inherent method; `pub fn history (&self) -> Option <& History >` — Returns `history`.
- `iyon_tui::Scene::history_mut` — inherent method; `pub fn history_mut (&mut self) -> Option <&mut History >` — Provides the public `history_mut` operation.
- `iyon_tui::Scene::body` — inherent method; `pub fn body (&self) -> & View` — Provides the public `body` operation.
- `iyon_tui::Scene::set_body` — inherent method; `pub fn set_body (&mut self, body: impl IntoView )` — Sets `body` and returns the previous value when applicable.

#### `iyon_tui::ScrollPane` — struct [re-export]
- Signature: `pub struct ScrollPane { /* private fields */ }`
- Purpose: A focusable retained viewport for arbitrary semantic content.
- `iyon_tui::ScrollPane::new` — inherent method; `pub fn new (content: impl IntoView ) -> Self` — Constructs a value.
- `iyon_tui::ScrollPane::set_content` — inherent method; `pub fn set_content (&mut self, content: impl IntoView )` — Sets `content` and returns the previous value when applicable.
- `iyon_tui::ScrollPane::scroll_up` — inherent method; `pub fn scroll_up (&mut self, rows: usize ) -> bool` — Provides the public `scroll_up` operation.
- `iyon_tui::ScrollPane::scroll_down` — inherent method; `pub fn scroll_down (&mut self, rows: usize ) -> bool` — Provides the public `scroll_down` operation.
- `iyon_tui::ScrollPane::page_up` — inherent method; `pub fn page_up (&mut self) -> bool` — Provides the public `page_up` operation.
- `iyon_tui::ScrollPane::page_down` — inherent method; `pub fn page_down (&mut self) -> bool` — Provides the public `page_down` operation.
- `iyon_tui::ScrollPane::scroll_to_start` — inherent method; `pub fn scroll_to_start (&mut self)` — Provides the public `scroll_to_start` operation.
- `iyon_tui::ScrollPane::follow_end` — inherent method; `pub fn follow_end (&mut self)` — Provides the public `follow_end` operation.
- `iyon_tui::ScrollPane::is_following_end` — inherent method; `pub fn is_following_end (&self) -> bool` — Reports whether `following_end` holds.

#### `iyon_tui::StyleRef` — struct [re-export]
- Signature: `pub struct StyleRef { /* private fields */ }`
- Purpose: A semantic named style plus a sparse local override.
- `iyon_tui::StyleRef::direct` — inherent method; `pub fn direct (style: StyleSpec ) -> Self` — Provides the public `direct` operation.
- `iyon_tui::StyleRef::theme` — inherent method; `pub fn theme (key: impl Into < ThemeKey >) -> Self` — Returns `theme`.
- `iyon_tui::StyleRef::themed` — inherent method; `pub fn themed (key: impl Into < ThemeKey >, overrides: StyleSpec ) -> Self` — Provides the public `themed` operation.
- `iyon_tui::StyleRef::overrides` — inherent method; `pub fn overrides (self, patch: StyleSpec ) -> Self` — Provides the public `overrides` operation.
- `iyon_tui::StyleRef::set_foreground` — inherent method; `pub fn set_foreground (&mut self, color: ColorSpec )` — Sets `foreground` and returns the previous value when applicable.
- `iyon_tui::StyleRef::set_background` — inherent method; `pub fn set_background (&mut self, color: ColorSpec )` — Sets `background` and returns the previous value when applicable.
- `iyon_tui::StyleRef::set_attribute` — inherent method; `pub fn set_attribute (&mut self, attribute: TextAttribute , enabled: bool )` — Sets `attribute` and returns the previous value when applicable.
- `iyon_tui::StyleRef::attribute_value` — inherent method; `pub fn attribute_value (&self, attribute: TextAttribute ) -> Option < bool >` — Provides the public `attribute_value` operation.

#### `iyon_tui::StyleSelector` — struct [re-export]
- Signature: `pub struct StyleSelector { /* private fields */ }`
- Purpose: A positive conjunction of framework interaction predicates and semantic application-owned key/value requirements.
- `iyon_tui::StyleSelector::focused` — inherent method; `pub fn focused () -> Self` — Provides the public `focused` operation.
- `iyon_tui::StyleSelector::focus_within` — inherent method; `pub fn focus_within () -> Self` — Provides the public `focus_within` operation.
- `iyon_tui::StyleSelector::state` — inherent method; `pub fn state ( key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `state` operation.
- `iyon_tui::StyleSelector::and_focused` — inherent method; `pub fn and_focused (self) -> Self` — Provides the public `and_focused` operation.
- `iyon_tui::StyleSelector::and_focus_within` — inherent method; `pub fn and_focus_within (self) -> Self` — Provides the public `and_focus_within` operation.
- `iyon_tui::StyleSelector::and_state` — inherent method; `pub fn and_state ( self, key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `and_state` operation.

#### `iyon_tui::StyleSpec` — struct [re-export]
- Signature: `pub struct StyleSpec { /* private fields */ }`
- Purpose: Sparse backend-neutral text-style intent.
- `iyon_tui::StyleSpec::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::StyleSpec::plain` — inherent method; `pub fn plain () -> Self` — Provides the public `plain` operation.
- `iyon_tui::StyleSpec::foreground` — inherent method; `pub fn foreground (self, color: ColorSpec ) -> Self` — Provides the public `foreground` operation.
- `iyon_tui::StyleSpec::background` — inherent method; `pub fn background (self, color: ColorSpec ) -> Self` — Provides the public `background` operation.
- `iyon_tui::StyleSpec::bold` — inherent method; `pub fn bold (self) -> Self` — Provides the public `bold` operation.
- `iyon_tui::StyleSpec::dim` — inherent method; `pub fn dim (self) -> Self` — Provides the public `dim` operation.
- `iyon_tui::StyleSpec::italic` — inherent method; `pub fn italic (self) -> Self` — Provides the public `italic` operation.
- `iyon_tui::StyleSpec::underline` — inherent method; `pub fn underline (self) -> Self` — Provides the public `underline` operation.
- `iyon_tui::StyleSpec::reversed` — inherent method; `pub fn reversed (self) -> Self` — Provides the public `reversed` operation.
- `iyon_tui::StyleSpec::strikethrough` — inherent method; `pub fn strikethrough (self) -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::StyleSpec::attribute` — inherent method; `pub fn attribute (self, attribute: TextAttribute , enabled: bool ) -> Self` — Provides the public `attribute` operation.
- `iyon_tui::StyleSpec::set_foreground` — inherent method; `pub fn set_foreground (&mut self, color: ColorSpec )` — Sets `foreground` and returns the previous value when applicable.
- `iyon_tui::StyleSpec::set_background` — inherent method; `pub fn set_background (&mut self, color: ColorSpec )` — Sets `background` and returns the previous value when applicable.
- `iyon_tui::StyleSpec::set_attribute` — inherent method; `pub fn set_attribute (&mut self, attribute: TextAttribute , enabled: bool )` — Sets `attribute` and returns the previous value when applicable.
- `iyon_tui::StyleSpec::with_attributes` — inherent method; `pub fn with_attributes (self, attributes: TextAttributeSpec ) -> Self` — Returns this value with `attributes` configured.
- `iyon_tui::StyleSpec::attribute_value` — inherent method; `pub fn attribute_value (&self, attribute: TextAttribute ) -> Option < bool >` — Provides the public `attribute_value` operation.

#### `iyon_tui::StyleStateKey` — struct [re-export]
- Signature: `pub struct StyleStateKey( /* private fields */ );`
- Purpose: An application-owned semantic styling dimension name.
- `iyon_tui::StyleStateKey::from_static` — inherent method; `pub const fn from_static (value: &'static str ) -> Self` — Provides the public `from_static` operation.
- `iyon_tui::StyleStateKey::new` — inherent method; `pub fn new (value: impl Into < String >) -> Self` — Constructs a value.
- `iyon_tui::StyleStateKey::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::StyleStateValue` — struct [re-export]
- Signature: `pub struct StyleStateValue( /* private fields */ );`
- Purpose: An application-owned semantic styling value.
- `iyon_tui::StyleStateValue::from_static` — inherent method; `pub const fn from_static (value: &'static str ) -> Self` — Provides the public `from_static` operation.
- `iyon_tui::StyleStateValue::new` — inherent method; `pub fn new (value: impl Into < String >) -> Self` — Constructs a value.
- `iyon_tui::StyleStateValue::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::Text` — struct [re-export]
- Signature: `pub struct Text { /* private fields */ }`
- Purpose: Typed backend-neutral text construction backed by the crate’s owned semantic View .
- `iyon_tui::Text::wrap` — inherent method; `pub fn wrap (self, wrap: WrapMode ) -> Self` — Provides the public `wrap` operation.
- `iyon_tui::Text::no_wrap` — inherent method; `pub fn no_wrap (self) -> Self` — Provides the public `no_wrap` operation.
- `iyon_tui::Text::text_align` — inherent method; `pub fn text_align (self, align: HorizontalAlign ) -> Self` — Provides the public `text_align` operation.
- `iyon_tui::Text::style` — inherent method; `pub fn style (self, style: impl Into < StyleRef >) -> Self` — Provides the public `style` operation.
- `iyon_tui::Text::style_state` — inherent method; `pub fn style_state ( self, key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `style_state` operation.
- `iyon_tui::Text::style_states` — inherent method; `pub fn style_states ( self, states: impl IntoIterator <Item = ( StyleStateKey , StyleStateValue )>, ) -> Self` — Provides the public `style_states` operation.
- `iyon_tui::Text::padding` — inherent method; `pub fn padding (self, padding: impl Into < Insets >) -> Self` — Provides the public `padding` operation.
- `iyon_tui::Text::background` — inherent method; `pub fn background (self, color: ColorSpec ) -> Self` — Provides the public `background` operation.
- `iyon_tui::Text::foreground` — inherent method; `pub fn foreground (self, color: ColorSpec ) -> Self` — Provides the public `foreground` operation.
- `iyon_tui::Text::border` — inherent method; `pub fn border (self, border: BorderSpec ) -> Self` — Provides the public `border` operation.
- `iyon_tui::Text::text_attribute` — inherent method; `pub fn text_attribute (self, attribute: TextAttribute , enabled: bool ) -> Self` — Provides the public `text_attribute` operation.
- `iyon_tui::Text::bold` — inherent method; `pub fn bold (self) -> Self` — Provides the public `bold` operation.
- `iyon_tui::Text::dim` — inherent method; `pub fn dim (self) -> Self` — Provides the public `dim` operation.
- `iyon_tui::Text::italic` — inherent method; `pub fn italic (self) -> Self` — Provides the public `italic` operation.
- `iyon_tui::Text::underline` — inherent method; `pub fn underline (self) -> Self` — Provides the public `underline` operation.
- `iyon_tui::Text::reversed` — inherent method; `pub fn reversed (self) -> Self` — Provides the public `reversed` operation.
- `iyon_tui::Text::strikethrough` — inherent method; `pub fn strikethrough (self) -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::Text::container` — inherent method; `pub fn container (self) -> View` — Provides the public `container` operation.
- `iyon_tui::Text::clamp_rows` — inherent method; `pub fn clamp_rows (self, max_rows: u16 , overflow: OverflowIndicator ) -> View` — Performs bounded arithmetic.
- `iyon_tui::Text::fit_width` — inherent method; `pub fn fit_width (self) -> Self` — Provides the public `fit_width` operation.
- `iyon_tui::Text::fill_width` — inherent method; `pub fn fill_width (self) -> Self` — Provides the public `fill_width` operation.
- `iyon_tui::Text::fit_height` — inherent method; `pub fn fit_height (self) -> Self` — Provides the public `fit_height` operation.
- `iyon_tui::Text::fill_height` — inherent method; `pub fn fill_height (self) -> Self` — Provides the public `fill_height` operation.

#### `iyon_tui::TextAttributeSpec` — struct [re-export]
- Signature: `pub struct TextAttributeSpec { /* private fields */ }`
- Purpose: Sparse text-attribute intent used by semantic style patches.
- `iyon_tui::TextAttributeSpec::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::TextAttributeSpec::attribute` — inherent method; `pub fn attribute (self, attribute: TextAttribute , enabled: bool ) -> Self` — Provides the public `attribute` operation.

#### `iyon_tui::TextChange` — struct [re-export]
- Signature: `pub struct TextChange<'a> { /* private fields */ }`
- Purpose: A borrowed snapshot of a TextInput after a user text mutation.
- `iyon_tui::TextChange::text` — inherent method; `pub fn text (self) -> &'a str` — Returns `text`.
- `iyon_tui::TextChange::cursor_bytes` — inherent method; `pub fn cursor_bytes (self) -> usize` — Provides the public `cursor_bytes` operation.
- `iyon_tui::TextChange::is_empty` — inherent method; `pub fn is_empty (self) -> bool` — Reports whether `empty` holds.

#### `iyon_tui::TextInput` — struct [re-export]
- Signature: `pub struct TextInput { /* private fields */ }`
- Purpose: Retained generic Unicode text editor state.
- `iyon_tui::TextInput::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::TextInput::multiline` — inherent method; `pub fn multiline (self, enabled: bool ) -> Self` — Provides the public `multiline` operation.
- `iyon_tui::TextInput::border` — inherent method; `pub fn border (self, border: BorderSpec ) -> Self` — Provides the public `border` operation.
- `iyon_tui::TextInput::set_multiline` — inherent method; `pub fn set_multiline (&mut self, enabled: bool )` — Sets `multiline` and returns the previous value when applicable.
- `iyon_tui::TextInput::is_multiline` — inherent method; `pub fn is_multiline (&self) -> bool` — Reports whether `multiline` holds.
- `iyon_tui::TextInput::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::TextInput::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::TextInput::cursor_bytes` — inherent method; `pub fn cursor_bytes (&self) -> usize` — Provides the public `cursor_bytes` operation.
- `iyon_tui::TextInput::set_text` — inherent method; `pub fn set_text (&mut self, text: impl AsRef < str >)` — Sets `text` and returns the previous value when applicable.
- `iyon_tui::TextInput::clear` — inherent method; `pub fn clear (&mut self)` — Provides the public `clear` operation.
- `iyon_tui::TextInput::submitted` — inherent method; `pub fn submitted (&self) -> Output < String >` — Provides the public `submitted` operation.
- `iyon_tui::TextInput::output_on_change` — inherent method; `pub fn output_on_change <R: 'static>( &mut self, project: impl for<'change> Fn ( TextChange <'change>) -> R + 'static, ) -> Output <R>` — Provides the public `output_on_change` operation.

#### `iyon_tui::TextOrigin` — struct [re-export]
- Signature: `pub struct TextOrigin( /* private fields */ );`
- Purpose: Identifies the syntax/projector that claimed a semantic text value.
- `iyon_tui::TextOrigin::MARKDOWN` — associated const; `pub const MARKDOWN : Self` — Provides the public `MARKDOWN` operation.
- `iyon_tui::TextOrigin::PLAIN_TEXT` — associated const; `pub const PLAIN_TEXT : Self` — Provides the public `PLAIN_TEXT` operation.
- `iyon_tui::TextOrigin::markdown` — inherent method; `pub fn markdown () -> Self` — Provides the public `markdown` operation.
- `iyon_tui::TextOrigin::plain_text` — inherent method; `pub fn plain_text () -> Self` — Provides the public `plain_text` operation.
- `iyon_tui::TextOrigin::new` — inherent method; `pub fn new (value: impl Into < Arc < str >>) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::TextOrigin::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::TextRenderPolicy` — struct [re-export]
- Signature: `pub struct TextRenderPolicy { /* private fields */ }`
- Purpose: Structural-only policy for generic text-to-View lowering.
- `iyon_tui::TextRenderPolicy::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::TextRenderPolicy::block_gap` — inherent method; `pub fn block_gap (&self) -> u16` — Provides the public `block_gap` operation.
- `iyon_tui::TextRenderPolicy::with_block_gap` — inherent method; `pub fn with_block_gap (self, gap: u16 ) -> Self` — Returns this value with `block_gap` configured.
- `iyon_tui::TextRenderPolicy::soft_break` — inherent method; `pub fn soft_break (&self) -> SoftBreakPolicy` — Provides the public `soft_break` operation.
- `iyon_tui::TextRenderPolicy::with_soft_break` — inherent method; `pub fn with_soft_break (self, policy: SoftBreakPolicy ) -> Self` — Returns this value with `soft_break` configured.
- `iyon_tui::TextRenderPolicy::table_column_gap` — inherent method; `pub fn table_column_gap (&self) -> u16` — Provides the public `table_column_gap` operation.
- `iyon_tui::TextRenderPolicy::with_table_column_gap` — inherent method; `pub fn with_table_column_gap (self, gap: u16 ) -> Self` — Returns this value with `table_column_gap` configured.
- `iyon_tui::TextRenderPolicy::table_row_gap` — inherent method; `pub fn table_row_gap (&self) -> u16` — Provides the public `table_row_gap` operation.
- `iyon_tui::TextRenderPolicy::with_table_row_gap` — inherent method; `pub fn with_table_row_gap (self, gap: u16 ) -> Self` — Returns this value with `table_row_gap` configured.
- `iyon_tui::TextRenderPolicy::table_column_sizing` — inherent method; `pub fn table_column_sizing (&self) -> TableColumnSizing` — Provides the public `table_column_sizing` operation.
- `iyon_tui::TextRenderPolicy::with_table_column_sizing` — inherent method; `pub fn with_table_column_sizing (self, sizing: TableColumnSizing ) -> Self` — Returns this value with `table_column_sizing` configured.
- `iyon_tui::TextRenderPolicy::task_list_marker` — inherent method; `pub fn task_list_marker (&self) -> TaskListMarkerPolicy` — Provides the public `task_list_marker` operation.
- `iyon_tui::TextRenderPolicy::with_task_list_marker` — inherent method; `pub fn with_task_list_marker (self, policy: TaskListMarkerPolicy ) -> Self` — Returns this value with `task_list_marker` configured.
- `iyon_tui::TextRenderPolicy::code_block_label` — inherent method; `pub fn code_block_label (&self) -> CodeBlockLabelPolicy` — Provides the public `code_block_label` operation.
- `iyon_tui::TextRenderPolicy::with_code_block_label` — inherent method; `pub fn with_code_block_label (self, policy: CodeBlockLabelPolicy ) -> Self` — Returns this value with `code_block_label` configured.
- `iyon_tui::TextRenderPolicy::code_block_gap` — inherent method; `pub fn code_block_gap (&self) -> u16` — Provides the public `code_block_gap` operation.
- `iyon_tui::TextRenderPolicy::with_code_block_gap` — inherent method; `pub fn with_code_block_gap (self, gap: u16 ) -> Self` — Returns this value with `code_block_gap` configured.
- `iyon_tui::TextRenderPolicy::code_wrap` — inherent method; `pub fn code_wrap (&self) -> WrapMode` — Provides the public `code_wrap` operation.
- `iyon_tui::TextRenderPolicy::with_code_wrap` — inherent method; `pub fn with_code_wrap (self, wrap: WrapMode ) -> Self` — Returns this value with `code_wrap` configured.

#### `iyon_tui::TextRenderer` — struct [re-export]
- Signature: `pub struct TextRenderer { /* private fields */ }`
- Purpose: The one generic renderer for the frozen text IR.
- `iyon_tui::TextRenderer::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::TextRenderer::with_policy` — inherent method; `pub fn with_policy (policy: TextRenderPolicy ) -> Self` — Returns this value with `policy` configured.
- `iyon_tui::TextRenderer::policy` — inherent method; `pub fn policy (&self) -> & TextRenderPolicy` — Returns `policy`.
- `iyon_tui::TextRenderer::render_block` — inherent method; `pub fn render_block (&self, block: & Block ) -> View` — Renders the semantic value.

#### `iyon_tui::TextSelector` — struct [re-export]
- Signature: `pub struct TextSelector { /* private fields */ }`
- Purpose: Typed selector for generic structured-text presentation.
- `iyon_tui::TextSelector::any` — inherent method; `pub fn any () -> Self` — Constructs a value.
- `iyon_tui::TextSelector::role` — inherent method; `pub fn role (role: TextRole ) -> Self` — Provides the public `role` operation.
- `iyon_tui::TextSelector::part` — inherent method; `pub fn part (part: TextPart ) -> Self` — Provides the public `part` operation.
- `iyon_tui::TextSelector::annotation` — inherent method; `pub fn annotation (tag: & SemanticTag ) -> Self` — Provides the public `annotation` operation.
- `iyon_tui::TextSelector::paragraph` — inherent method; `pub fn paragraph () -> Self` — Provides the public `paragraph` operation.
- `iyon_tui::TextSelector::heading` — inherent method; `pub fn heading () -> Self` — Provides the public `heading` operation.
- `iyon_tui::TextSelector::block_quote` — inherent method; `pub fn block_quote () -> Self` — Provides the public `block_quote` operation.
- `iyon_tui::TextSelector::list` — inherent method; `pub fn list () -> Self` — Provides the public `list` operation.
- `iyon_tui::TextSelector::list_item` — inherent method; `pub fn list_item () -> Self` — Provides the public `list_item` operation.
- `iyon_tui::TextSelector::code_block` — inherent method; `pub fn code_block () -> Self` — Provides the public `code_block` operation.
- `iyon_tui::TextSelector::table` — inherent method; `pub fn table () -> Self` — Provides the public `table` operation.
- `iyon_tui::TextSelector::table_row` — inherent method; `pub fn table_row () -> Self` — Provides the public `table_row` operation.
- `iyon_tui::TextSelector::table_cell` — inherent method; `pub fn table_cell () -> Self` — Provides the public `table_cell` operation.
- `iyon_tui::TextSelector::thematic_break` — inherent method; `pub fn thematic_break () -> Self` — Provides the public `thematic_break` operation.
- `iyon_tui::TextSelector::strong` — inherent method; `pub fn strong () -> Self` — Provides the public `strong` operation.
- `iyon_tui::TextSelector::emphasis` — inherent method; `pub fn emphasis () -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::TextSelector::strikethrough` — inherent method; `pub fn strikethrough () -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::TextSelector::underline` — inherent method; `pub fn underline () -> Self` — Provides the public `underline` operation.
- `iyon_tui::TextSelector::inline_code` — inherent method; `pub fn inline_code () -> Self` — Provides the public `inline_code` operation.
- `iyon_tui::TextSelector::link` — inherent method; `pub fn link () -> Self` — Provides the public `link` operation.
- `iyon_tui::TextSelector::and_role` — inherent method; `pub fn and_role (self, role: TextRole ) -> Self` — Provides the public `and_role` operation.
- `iyon_tui::TextSelector::level` — inherent method; `pub fn level (self, level: HeadingLevel ) -> Self` — Provides the public `level` operation.
- `iyon_tui::TextSelector::origin` — inherent method; `pub fn origin (self, origin: TextOrigin ) -> Self` — Returns `origin`.
- `iyon_tui::TextSelector::list_kind` — inherent method; `pub fn list_kind (self, kind: TextListKind ) -> Self` — Provides the public `list_kind` operation.
- `iyon_tui::TextSelector::task_state` — inherent method; `pub fn task_state (self, state: TextTaskState ) -> Self` — Provides the public `task_state` operation.
- `iyon_tui::TextSelector::table_section` — inherent method; `pub fn table_section (self, section: TextTableSection ) -> Self` — Provides the public `table_section` operation.
- `iyon_tui::TextSelector::language` — inherent method; `pub fn language (self, language: & LanguageId ) -> Self` — Provides the public `language` operation.
- `iyon_tui::TextSelector::format` — inherent method; `pub fn format (self, format: & FormatId ) -> Self` — Provides the public `format` operation.
- `iyon_tui::TextSelector::and_annotation` — inherent method; `pub fn and_annotation (self, tag: & SemanticTag ) -> Self` — Provides the public `and_annotation` operation.
- `iyon_tui::TextSelector::and_focused` — inherent method; `pub fn and_focused (self) -> Self` — Provides the public `and_focused` operation.
- `iyon_tui::TextSelector::and_focus_within` — inherent method; `pub fn and_focus_within (self) -> Self` — Provides the public `and_focus_within` operation.
- `iyon_tui::TextSelector::and_state` — inherent method; `pub fn and_state ( self, key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `and_state` operation.

#### `iyon_tui::TextSpan` — struct [re-export]
- Signature: `pub struct TextSpan { /* private fields */ }`
- Purpose: A semantic text span with optional text-cell styling.
- `iyon_tui::TextSpan::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::TextSpan::text_mut` — inherent method; `pub fn text_mut (&mut self) -> &mut String` — Provides the public `text_mut` operation.
- `iyon_tui::TextSpan::style` — inherent method; `pub fn style (&self) -> & StyleRef` — Provides the public `style` operation.
- `iyon_tui::TextSpan::style_mut` — inherent method; `pub fn style_mut (&mut self) -> &mut StyleRef` — Provides the public `style_mut` operation.
- `iyon_tui::TextSpan::plain` — inherent method; `pub fn plain (text: impl Into < String >) -> Self` — Provides the public `plain` operation.
- `iyon_tui::TextSpan::styled` — inherent method; `pub fn styled (text: impl Into < String >, style: impl Into < StyleRef >) -> Self` — Provides the public `styled` operation.

#### `iyon_tui::Theme` — struct [re-export]
- Signature: `pub struct Theme { /* private fields */ }`
- Purpose: Public struct `Theme`.
- `iyon_tui::Theme::with_text_style` — inherent method; `pub fn with_text_style (self, selector: TextSelector , style: StyleSpec ) -> Self` — Returns this value with `text_style` configured.
- `iyon_tui::Theme::set_text_style` — inherent method; `pub fn set_text_style ( &mut self, selector: TextSelector , style: StyleSpec , ) -> Option < StyleSpec >` — Sets `text_style` and returns the previous value when applicable.
- `iyon_tui::Theme::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::Theme::with_color` — inherent method; `pub fn with_color (self, key: impl Into < ThemeKey >, color: ThemeColor ) -> Self` — Returns this value with `color` configured.
- `iyon_tui::Theme::with_color_variant` — inherent method; `pub fn with_color_variant ( self, key: impl Into < ThemeKey >, selector: StyleSelector , color: ThemeColor , ) -> Self` — Returns this value with `color_variant` configured.
- `iyon_tui::Theme::with_style` — inherent method; `pub fn with_style (self, key: impl Into < ThemeKey >, style: StyleSpec ) -> Self` — Returns this value with `style` configured.
- `iyon_tui::Theme::with_style_variant` — inherent method; `pub fn with_style_variant ( self, key: impl Into < ThemeKey >, selector: StyleSelector , style: StyleSpec , ) -> Self` — Returns this value with `style_variant` configured.
- `iyon_tui::Theme::set_color` — inherent method; `pub fn set_color ( &mut self, key: impl Into < ThemeKey >, color: ThemeColor , ) -> Option < ThemeColor >` — Sets `color` and returns the previous value when applicable.
- `iyon_tui::Theme::set_color_variant` — inherent method; `pub fn set_color_variant ( &mut self, key: impl Into < ThemeKey >, selector: StyleSelector , color: ThemeColor , ) -> Option < ThemeColor >` — Sets `color_variant` and returns the previous value when applicable.
- `iyon_tui::Theme::set_style` — inherent method; `pub fn set_style ( &mut self, key: impl Into < ThemeKey >, style: StyleSpec , ) -> Option < StyleSpec >` — Sets `style` and returns the previous value when applicable.
- `iyon_tui::Theme::set_style_variant` — inherent method; `pub fn set_style_variant ( &mut self, key: impl Into < ThemeKey >, selector: StyleSelector , style: StyleSpec , ) -> Option < StyleSpec >` — Sets `style_variant` and returns the previous value when applicable.
- `iyon_tui::Theme::color` — inherent method; `pub fn color (&self, key: & str ) -> Option < ThemeColor >` — Provides the public `color` operation.
- `iyon_tui::Theme::style` — inherent method; `pub fn style (&self, key: & str ) -> Option <& StyleSpec >` — Provides the public `style` operation.

#### `iyon_tui::ThemeKey` — struct [re-export]
- Signature: `pub struct ThemeKey( /* private fields */ );`
- Purpose: Opaque semantic key resolved by the host theme.
- `iyon_tui::ThemeKey::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::TimerHandle` — struct [re-export]
- Signature: `pub struct TimerHandle { /* private fields */ }`
- Purpose: Opaque identity for one application-owned one-shot timer.

#### `iyon_tui::Vertical` — struct [re-export]
- Signature: `pub struct Vertical { /* private fields */ }`
- Purpose: Closure-scoped capability for constructing vertical semantic composition.
- `iyon_tui::Vertical::child` — inherent method; `pub fn child (&mut self, child: impl IntoView ) -> &mut Self` — Provides the public `child` operation.
- `iyon_tui::Vertical::fixed` — inherent method; `pub fn fixed (&mut self, height: u16 , child: impl IntoView ) -> &mut Self` — Provides the public `fixed` operation.
- `iyon_tui::Vertical::content_max` — inherent method; `pub fn content_max (&mut self, max_rows: u16 , child: impl IntoView ) -> &mut Self` — Provides the public `content_max` operation.
- `iyon_tui::Vertical::flex` — inherent method; `pub fn flex (&mut self, child: impl IntoView ) -> &mut Self` — Provides the public `flex` operation.
- `iyon_tui::Vertical::flex_max` — inherent method; `pub fn flex_max (&mut self, max_rows: u16 , child: impl IntoView ) -> &mut Self` — Provides the public `flex_max` operation.
- `iyon_tui::Vertical::children` — inherent method; `pub fn children <I, V>(&mut self, children: I) -> &mut Self where I: IntoIterator <Item = V>, V: IntoView ,` — Provides the public `children` operation.
- `iyon_tui::Vertical::gap` — inherent method; `pub fn gap (&mut self, gap: u16 ) -> &mut Self` — Provides the public `gap` operation.

#### `iyon_tui::View` — struct [re-export]
- Signature: `pub struct View { /* private fields */ }`
- Purpose: An owned backend-neutral semantic view.
- `iyon_tui::View::component` — inherent method; `pub fn component <C>(handle: ComponentHandle <C>) -> Self` — Provides the public `component` operation.
- `iyon_tui::View::text` — inherent method; `pub fn text (text: impl Into < String >) -> Text` — Returns `text`.
- `iyon_tui::View::styled_text` — inherent method; `pub fn styled_text (spans: impl IntoIterator <Item = TextSpan >) -> Text` — Provides the public `styled_text` operation.
- `iyon_tui::View::horizontal` — inherent method; `pub fn horizontal (build: impl FnOnce (&mut Horizontal )) -> Self` — Provides the public `horizontal` operation.
- `iyon_tui::View::vertical` — inherent method; `pub fn vertical (build: impl FnOnce (&mut Vertical )) -> Self` — Provides the public `vertical` operation.
- `iyon_tui::View::grid` — inherent method; `pub fn grid (build: impl FnOnce (&mut Grid )) -> Self` — Provides the public `grid` operation.
- `iyon_tui::View::hanging` — inherent method; `pub fn hanging ( prefix: impl IntoView , continuation_prefix: impl IntoView , body: impl IntoView , ) -> Self` — Provides the public `hanging` operation.
- `iyon_tui::View::container` — inherent method; `pub fn container (self) -> Self` — Provides the public `container` operation.
- `iyon_tui::View::clamp_rows` — inherent method; `pub fn clamp_rows (self, max_rows: u16 , overflow: OverflowIndicator ) -> Self` — Performs bounded arithmetic.
- `iyon_tui::View::spacer` — inherent method; `pub fn spacer (rows: u16 ) -> Self` — Provides the public `spacer` operation.
- `iyon_tui::View::style_state` — inherent method; `pub fn style_state ( self, key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `style_state` operation.
- `iyon_tui::View::style_states` — inherent method; `pub fn style_states ( self, states: impl IntoIterator <Item = ( StyleStateKey , StyleStateValue )>, ) -> Self` — Provides the public `style_states` operation.
- `iyon_tui::View::padding` — inherent method; `pub fn padding (self, padding: impl Into < Insets >) -> Self` — Provides the public `padding` operation.
- `iyon_tui::View::background` — inherent method; `pub fn background (self, color: ColorSpec ) -> Self` — Provides the public `background` operation.
- `iyon_tui::View::style` — inherent method; `pub fn style (self, style: impl Into < StyleRef >) -> Self` — Provides the public `style` operation.
- `iyon_tui::View::foreground` — inherent method; `pub fn foreground (self, color: ColorSpec ) -> Self` — Provides the public `foreground` operation.
- `iyon_tui::View::border` — inherent method; `pub fn border (self, border: BorderSpec ) -> Self` — Provides the public `border` operation.
- `iyon_tui::View::text_attribute` — inherent method; `pub fn text_attribute (self, attribute: TextAttribute , enabled: bool ) -> Self` — Provides the public `text_attribute` operation.
- `iyon_tui::View::bold` — inherent method; `pub fn bold (self) -> Self` — Provides the public `bold` operation.
- `iyon_tui::View::dim` — inherent method; `pub fn dim (self) -> Self` — Provides the public `dim` operation.
- `iyon_tui::View::italic` — inherent method; `pub fn italic (self) -> Self` — Provides the public `italic` operation.
- `iyon_tui::View::underline` — inherent method; `pub fn underline (self) -> Self` — Provides the public `underline` operation.
- `iyon_tui::View::reversed` — inherent method; `pub fn reversed (self) -> Self` — Provides the public `reversed` operation.
- `iyon_tui::View::strikethrough` — inherent method; `pub fn strikethrough (self) -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::View::fit_width` — inherent method; `pub fn fit_width (self) -> Self` — Provides the public `fit_width` operation.
- `iyon_tui::View::fill_width` — inherent method; `pub fn fill_width (self) -> Self` — Provides the public `fill_width` operation.
- `iyon_tui::View::fit_height` — inherent method; `pub fn fit_height (self) -> Self` — Provides the public `fit_height` operation.
- `iyon_tui::View::fill_height` — inherent method; `pub fn fill_height (self) -> Self` — Provides the public `fill_height` operation.
- `iyon_tui::View::min_width` — inherent method; `pub fn min_width (self, width: u16 ) -> Self` — Provides the public `min_width` operation.
- `iyon_tui::View::max_width` — inherent method; `pub fn max_width (self, width: u16 ) -> Self` — Provides the public `max_width` operation.
- `iyon_tui::View::min_height` — inherent method; `pub fn min_height (self, height: u16 ) -> Self` — Provides the public `min_height` operation.
- `iyon_tui::View::max_height` — inherent method; `pub fn max_height (self, height: u16 ) -> Self` — Provides the public `max_height` operation.

#### `iyon_tui::Component` — trait [re-export]
- Signature: `pub trait Component: 'static { // Required method fn view (&self) -> View ; // Provided method fn capabilities (&self, _cx: &mut ComponentCx <'_, Self>) where Self: Sized { ... } }`
- Purpose: Public retained-state rendering and capability declaration contract.
- `iyon_tui::Component::view` — required method; `fn view (&self) -> View` — Provides the public `view` operation.
- `iyon_tui::Component::capabilities` — provided method; `fn capabilities (&self, _cx: &mut ComponentCx <'_, Self>) where Self: Sized ,` — Provides the public `capabilities` operation.

#### `iyon_tui::IntoView` — trait [re-export]
- Signature: `pub trait IntoView { // Required method fn into_view (self) -> View ; }`
- Purpose: Explicit conversion from semantic construction values into the canonical owned View representation.
- `iyon_tui::IntoView::into_view` — required method; `fn into_view (self) -> View` — Converts or exposes this value.

#### `iyon_tui::Renderer` — trait [re-export]
- Signature: `pub trait Renderer<Input: ? Sized > { // Required method fn render (&self, input: &Input ) -> View ; }`
- Purpose: Converts a semantic value into the generic presentation View .
- `iyon_tui::Renderer::render` — required method; `fn render (&self, input: &Input ) -> View` — Renders the semantic value.

## iyon_tui::projection

#### `iyon_tui::projection::ProjectionRelationError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ProjectionRelationError { SourceBaseMismatch, OutputEndBeyondInput, OutputStabilityBeyondInput, OutputSealedBeforeInput, SealedOutputNotCaughtUp, }`
- Purpose: Failures when checking an output projection against its input projection.
- Variants and fields: `SourceBaseMismatch, OutputEndBeyondInput, OutputStabilityBeyondInput, OutputSealedBeforeInput, SealedOutputNotCaughtUp,`
- Variant paths: `iyon_tui::projection::ProjectionRelationError::SourceBaseMismatch`, `iyon_tui::projection::ProjectionRelationError::OutputEndBeyondInput`, `iyon_tui::projection::ProjectionRelationError::OutputStabilityBeyondInput`, `iyon_tui::projection::ProjectionRelationError::OutputSealedBeforeInput`, `iyon_tui::projection::ProjectionRelationError::SealedOutputNotCaughtUp`.

#### `iyon_tui::projection::ProjectionTransitionError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ProjectionTransitionError { SourceBaseRegressed, SourceBaseBeyondPreviousStability, SourceEndRegressed, StabilityRegressed, StablePrefixChanged, UnsealedAfterSeal, ChangedAfterSeal, }`
- Purpose: Failures when comparing successive snapshots of one projection stage.
- Variants and fields: `SourceBaseRegressed, SourceBaseBeyondPreviousStability, SourceEndRegressed, StabilityRegressed, StablePrefixChanged, UnsealedAfterSeal, ChangedAfterSeal,`
- Variant paths: `iyon_tui::projection::ProjectionTransitionError::SourceBaseRegressed`, `iyon_tui::projection::ProjectionTransitionError::SourceBaseBeyondPreviousStability`, `iyon_tui::projection::ProjectionTransitionError::SourceEndRegressed`, `iyon_tui::projection::ProjectionTransitionError::StabilityRegressed`, `iyon_tui::projection::ProjectionTransitionError::StablePrefixChanged`, `iyon_tui::projection::ProjectionTransitionError::UnsealedAfterSeal`, `iyon_tui::projection::ProjectionTransitionError::ChangedAfterSeal`.

#### `iyon_tui::projection::ProjectionValidationError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ProjectionValidationError { InvalidFrontier, EmptySpan, FirstSpanDoesNotStartAtBase, GapOrOverlap, SpanBeyondSourceEnd, TrailingUncoveredSource, StableFrontierInsideSpan, SealedBeforeStableEnd, }`
- Purpose: Construction failures for a projection.
- Variants and fields: `InvalidFrontier, EmptySpan, FirstSpanDoesNotStartAtBase, GapOrOverlap, SpanBeyondSourceEnd, TrailingUncoveredSource, StableFrontierInsideSpan, SealedBeforeStableEnd,`
- Variant paths: `iyon_tui::projection::ProjectionValidationError::InvalidFrontier`, `iyon_tui::projection::ProjectionValidationError::EmptySpan`, `iyon_tui::projection::ProjectionValidationError::FirstSpanDoesNotStartAtBase`, `iyon_tui::projection::ProjectionValidationError::GapOrOverlap`, `iyon_tui::projection::ProjectionValidationError::SpanBeyondSourceEnd`, `iyon_tui::projection::ProjectionValidationError::TrailingUncoveredSource`, `iyon_tui::projection::ProjectionValidationError::StableFrontierInsideSpan`, `iyon_tui::projection::ProjectionValidationError::SealedBeforeStableEnd`.

#### `iyon_tui::projection::SmoothConfigError` — enum [re-export]
- Signature: `pub enum SmoothConfigError { ZeroTickInterval, NonFiniteSpring, NegativeSpring, NonFiniteRate, NegativeRate, MinimumExceedsMaximum, NoProgressRate, }`
- Purpose: Invalid temporal smoothing configuration.
- Variants and fields: `ZeroTickInterval, NonFiniteSpring, NegativeSpring, NonFiniteRate, NegativeRate, MinimumExceedsMaximum, NoProgressRate,`
- Variant paths: `iyon_tui::projection::SmoothConfigError::ZeroTickInterval`, `iyon_tui::projection::SmoothConfigError::NonFiniteSpring`, `iyon_tui::projection::SmoothConfigError::NegativeSpring`, `iyon_tui::projection::SmoothConfigError::NonFiniteRate`, `iyon_tui::projection::SmoothConfigError::NegativeRate`, `iyon_tui::projection::SmoothConfigError::MinimumExceedsMaximum`, `iyon_tui::projection::SmoothConfigError::NoProgressRate`.

#### `iyon_tui::projection::ThenError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ThenError<A, B> { First(A), FirstRelation( ProjectionRelationError ), Second(B), SecondRelation( ProjectionRelationError ), }`
- Purpose: Errors from either projector or either stage’s relation contract.
- Variants and fields: `First(A), FirstRelation( ProjectionRelationError ), Second(B), SecondRelation( ProjectionRelationError ),`
- Variant paths: `iyon_tui::projection::ThenError::First`, `iyon_tui::projection::ThenError::FirstRelation`, `iyon_tui::projection::ThenError::Second`, `iyon_tui::projection::ThenError::SecondRelation`.

#### `iyon_tui::projection::validate_projection_relation` — free function [re-export]
- Signature: `pub fn validate_projection_relation<I, O>( input: & Projection <I>, output: & Projection <O>, ) -> Result < () , ProjectionRelationError >`
- Purpose: Validates that an output stage accounts for a prefix of its input stage.

#### `iyon_tui::projection::validate_projection_transition` — free function [re-export]
- Signature: `pub fn validate_projection_transition<T: PartialEq >( previous: & Projection <T>, next: & Projection <T>, ) -> Result < () , ProjectionTransitionError >`
- Purpose: Validates the monotonic transition between two snapshots of one stage.

#### `iyon_tui::projection::Projection` — struct [re-export]
- Signature: `pub struct Projection<T> { /* private fields */ }`
- Purpose: A validated projection of a contiguous interval in one root source space.
- `iyon_tui::projection::Projection::rebuild` — inherent method; `pub fn rebuild <U>(&self) -> ProjectionBuilder <U>` — Builds or validates the requested projection.
- `iyon_tui::projection::Projection::source_base` — inherent method; `pub fn source_base (&self) -> StreamOffset` — Returns `source_base`.
- `iyon_tui::projection::Projection::stable_through` — inherent method; `pub fn stable_through (&self) -> StreamOffset` — Returns `stable_through`.
- `iyon_tui::projection::Projection::source_end` — inherent method; `pub fn source_end (&self) -> StreamOffset` — Returns `source_end`.
- `iyon_tui::projection::Projection::is_sealed` — inherent method; `pub fn is_sealed (&self) -> bool` — Reports whether `sealed` holds.
- `iyon_tui::projection::Projection::spans` — inherent method; `pub fn spans (&self) -> &[ ProjectionSpan <T>]` — Returns `spans`.
- `iyon_tui::projection::Projection::map_ref` — inherent method; `pub fn map_ref <U>(&self, map: impl FnMut ( &T ) -> U) -> Projection <U>` — Maps or rewrites the contained semantic value.
- `iyon_tui::projection::Projection::try_map_ref` — inherent method; `pub fn try_map_ref <U, E>( &self, map: impl FnMut ( &T ) -> Result <U, E>, ) -> Result < Projection <U>, E>` — Performs the fallible validation or operation.
- `iyon_tui::projection::Projection::map_spans` — inherent method; `pub fn map_spans <U>( &self, map: impl FnMut (& ProjectionSpan <T>) -> Vec <U>, ) -> Projection <U>` — Maps or rewrites the contained semantic value.
- `iyon_tui::projection::Projection::try_map_spans` — inherent method; `pub fn try_map_spans <U, E>( &self, map: impl FnMut (& ProjectionSpan <T>) -> Result < Vec <U>, E>, ) -> Result < Projection <U>, E>` — Performs the fallible validation or operation.
- `iyon_tui::projection::Projection::map` — inherent method; `pub fn map <U>(self, map: impl FnMut (T) -> U) -> Projection <U>` — Provides the public `map` operation.

#### `iyon_tui::projection::ProjectionBuilder` — struct [re-export]
- Signature: `pub struct ProjectionBuilder<T> { /* private fields */ }`
- Purpose: Validated construction boundary for a Projection .
- `iyon_tui::projection::ProjectionBuilder::new` — inherent method; `pub fn new ( source_base: StreamOffset , stable_through: StreamOffset , source_end: StreamOffset , sealed: bool , ) -> Self` — Constructs a value.
- `iyon_tui::projection::ProjectionBuilder::emit` — inherent method; `pub fn emit (self, source: StreamRange , value: T) -> Self` — Builds or validates the requested projection.
- `iyon_tui::projection::ProjectionBuilder::emit_many` — inherent method; `pub fn emit_many ( self, source: StreamRange , values: impl IntoIterator <Item = T>, ) -> Self` — Builds or validates the requested projection.
- `iyon_tui::projection::ProjectionBuilder::elide` — inherent method; `pub fn elide (self, source: StreamRange ) -> Self` — Builds or validates the requested projection.
- `iyon_tui::projection::ProjectionBuilder::finish` — inherent method; `pub fn finish (self) -> Result < Projection <T>, ProjectionValidationError >` — Builds or validates the requested projection.

#### `iyon_tui::projection::ProjectionSpan` — struct [re-export]
- Signature: `pub struct ProjectionSpan<T> { /* private fields */ }`
- Purpose: One contiguous source interval and the values projected from it.
- `iyon_tui::projection::ProjectionSpan::source` — inherent method; `pub fn source (&self) -> StreamRange` — Returns `source`.
- `iyon_tui::projection::ProjectionSpan::values` — inherent method; `pub fn values (&self) -> & [T]` — Returns `values`.

#### `iyon_tui::projection::Smooth` — struct [re-export]
- Signature: `pub struct Smooth { /* private fields */ }`
- Purpose: Delays publication of complete upstream-stable spans without transforming values.
- `iyon_tui::projection::Smooth::new` — inherent method; `pub fn new (config: SmoothConfig ) -> Self` — Constructs a value.
- `iyon_tui::projection::Smooth::config` — inherent method; `pub fn config (&self) -> SmoothConfig` — Returns `config`.
- `iyon_tui::projection::Smooth::next_wakeup` — inherent method; `pub fn next_wakeup (&self) -> Option < Instant >` — Advances state or returns a temporal coordinate.
- `iyon_tui::projection::Smooth::published_through` — inherent method; `pub fn published_through (&self) -> StreamOffset` — Advances state or returns a temporal coordinate.
- `iyon_tui::projection::Smooth::advance` — inherent method; `pub fn advance (&mut self, now: Instant ) -> bool` — Advances state or returns a temporal coordinate.

#### `iyon_tui::projection::SmoothConfig` — struct [re-export]
- Signature: `pub struct SmoothConfig { /* private fields */ }`
- Purpose: Configuration for Smooth .
- `iyon_tui::projection::SmoothConfig::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::projection::SmoothConfig::try_from_parts` — inherent method; `pub fn try_from_parts ( tick_interval: Duration , spring: f32 , min_units_per_second: f32 , max_units_per_second: f32 , ) -> Result <Self, SmoothConfigError >` — Performs the fallible validation or operation.
- `iyon_tui::projection::SmoothConfig::tick_interval` — inherent method; `pub fn tick_interval (self) -> Duration` — Provides the public `tick_interval` operation.
- `iyon_tui::projection::SmoothConfig::spring` — inherent method; `pub fn spring (self) -> f32` — Provides the public `spring` operation.
- `iyon_tui::projection::SmoothConfig::min_units_per_second` — inherent method; `pub fn min_units_per_second (self) -> f32` — Provides the public `min_units_per_second` operation.
- `iyon_tui::projection::SmoothConfig::max_units_per_second` — inherent method; `pub fn max_units_per_second (self) -> f32` — Provides the public `max_units_per_second` operation.
- `iyon_tui::projection::SmoothConfig::with_tick_interval` — inherent method; `pub fn with_tick_interval ( self, value: Duration , ) -> Result <Self, SmoothConfigError >` — Returns this value with `tick_interval` configured.
- `iyon_tui::projection::SmoothConfig::with_spring` — inherent method; `pub fn with_spring (self, value: f32 ) -> Result <Self, SmoothConfigError >` — Returns this value with `spring` configured.
- `iyon_tui::projection::SmoothConfig::with_unit_rates` — inherent method; `pub fn with_unit_rates ( self, minimum: f32 , maximum: f32 , ) -> Result <Self, SmoothConfigError >` — Returns this value with `unit_rates` configured.

#### `iyon_tui::projection::Then` — struct [re-export]
- Signature: `pub struct Then<A, B> { /* private fields */ }`
- Purpose: A statically typed two-stage projector composition.

#### `iyon_tui::projection::Projector` — trait [re-export]
- Signature: `pub trait Projector<Input> { type Output ; type Error ; // Required method fn project ( &mut self, input: & Projection <Input>, ) -> Result < Projection <Self:: Output >, Self:: Error >; // Provided methods fn restart_from (&self, output_from: StreamOffset ) -> StreamOffset { ... } fn next_wakeup (&self) -> Option < Instant > { ... } fn advance (&mut self, _now: Instant ) -> bool { ... } }`
- Purpose: Transforms one root-coordinate projection into another projection.
- `iyon_tui::projection::Projector::Output` — associated type; `type Output` — Projected output type.
- `iyon_tui::projection::Projector::Error` — associated type; `type Error` — Projection error type.
- `iyon_tui::projection::Projector::project` — required method; `fn project ( &mut self, input: & Projection <Input>, ) -> Result < Projection <Self:: Output >, Self:: Error >` — Projects the input value.
- `iyon_tui::projection::Projector::restart_from` — provided method; `fn restart_from (&self, output_from: StreamOffset ) -> StreamOffset` — Advances state or returns a temporal coordinate.
- `iyon_tui::projection::Projector::next_wakeup` — provided method; `fn next_wakeup (&self) -> Option < Instant >` — Advances state or returns a temporal coordinate.
- `iyon_tui::projection::Projector::advance` — provided method; `fn advance (&mut self, _now: Instant ) -> bool` — Advances state or returns a temporal coordinate.

#### `iyon_tui::projection::ProjectorExt` — trait [re-export]
- Signature: `pub trait ProjectorExt<Input>: Projector <Input> + Sized { // Provided method fn then <P>(self, next: P) -> Then <Self, P> where P: Projector <Self:: Output > { ... } }`
- Purpose: Extension methods for statically composing projectors.
- `iyon_tui::projection::ProjectorExt::then` — provided method; `fn then <P>(self, next: P) -> Then <Self, P> where P: Projector <Self:: Output >,` — Provides the public `then` operation.

## iyon_tui::stream

#### `iyon_tui::stream::ProjectedValidationError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum ProjectedValidationError { InvalidContentRange, InvalidHangingPrefix, NonContiguousRun, EmptyRun, RunBeyondContent, InvalidVisibleRange, VisibleLengthMismatch, IncompleteSourceCoverage, HangingWidthMismatch, }`
- Purpose: Public failures in projected source/display mapping.
- Variants and fields: `InvalidContentRange, InvalidHangingPrefix, NonContiguousRun, EmptyRun, RunBeyondContent, InvalidVisibleRange, VisibleLengthMismatch, IncompleteSourceCoverage, HangingWidthMismatch,`
- Variant paths: `iyon_tui::stream::ProjectedValidationError::InvalidContentRange`, `iyon_tui::stream::ProjectedValidationError::InvalidHangingPrefix`, `iyon_tui::stream::ProjectedValidationError::NonContiguousRun`, `iyon_tui::stream::ProjectedValidationError::EmptyRun`, `iyon_tui::stream::ProjectedValidationError::RunBeyondContent`, `iyon_tui::stream::ProjectedValidationError::InvalidVisibleRange`, `iyon_tui::stream::ProjectedValidationError::VisibleLengthMismatch`, `iyon_tui::stream::ProjectedValidationError::IncompleteSourceCoverage`, `iyon_tui::stream::ProjectedValidationError::HangingWidthMismatch`.

#### `iyon_tui::stream::StreamOffset` — struct [re-export]
- Signature: `pub struct StreamOffset( /* private fields */ );`
- Purpose: Opaque monotonic coordinate within one stream’s root source space.
- `iyon_tui::stream::StreamOffset::ZERO` — associated const; `pub const ZERO : Self` — Provides the public `ZERO` operation.
- `iyon_tui::stream::StreamOffset::new` — inherent method; `pub const fn new (offset: u64 ) -> Self` — Constructs a value.
- `iyon_tui::stream::StreamOffset::as_u64` — inherent method; `pub const fn as_u64 (self) -> u64` — Converts or exposes this value.
- `iyon_tui::stream::StreamOffset::checked_add` — inherent method; `pub const fn checked_add (self, rhs: u64 ) -> Option <Self>` — Performs bounded arithmetic.
- `iyon_tui::stream::StreamOffset::saturating_add` — inherent method; `pub const fn saturating_add (self, rhs: u64 ) -> Self` — Performs bounded arithmetic.

#### `iyon_tui::stream::StreamRange` — struct [re-export]
- Signature: `pub struct StreamRange { /* private fields */ }`
- Purpose: A half-open range [start, end) in one stream’s root coordinate space.
- `iyon_tui::stream::StreamRange::new` — inherent method; `pub const fn new (start: StreamOffset , end: StreamOffset ) -> Self` — Constructs a value.
- `iyon_tui::stream::StreamRange::try_new` — inherent method; `pub const fn try_new (start: StreamOffset , end: StreamOffset ) -> Option <Self>` — Performs the fallible validation or operation.
- `iyon_tui::stream::StreamRange::start` — inherent method; `pub const fn start (self) -> StreamOffset` — Performs the requested application operation.
- `iyon_tui::stream::StreamRange::end` — inherent method; `pub const fn end (self) -> StreamOffset` — Provides the public `end` operation.
- `iyon_tui::stream::StreamRange::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::stream::StreamRange::len` — inherent method; `pub fn len (&self) -> u64` — Returns the number of contained items.
- `iyon_tui::stream::StreamRange::contains_offset` — inherent method; `pub fn contains_offset (&self, offset: StreamOffset ) -> bool` — Reports whether the value contains the requested item.

#### `iyon_tui::text::Alignment` — enum [re-export]
- Signature: `pub enum Alignment { Default, Start, Center, End, }`
- Purpose: Public enum `Alignment`.
- Variants and fields: `Default, Start, Center, End,`
- Variant paths: `iyon_tui::text::Alignment::Default`, `iyon_tui::text::Alignment::Start`, `iyon_tui::text::Alignment::Center`, `iyon_tui::text::Alignment::End`.

#### `iyon_tui::text::BlockKind` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum BlockKind { Paragraph( InlineContent ), Heading { level: HeadingLevel , content: InlineContent , }, BlockQuote { blocks: Arc <[ Block ]>, }, List( List ), CodeBlock( CodeBlock ), Table( Table ), ThematicBreak, RawBlock { format: FormatId , body: LiteralText , }, Container { blocks: Arc <[ Block ]>, }, }`
- Purpose: Generic block-level semantic kind.
- Variants and fields: `Paragraph( InlineContent ), Heading { level: HeadingLevel , content: InlineContent , }, BlockQuote { blocks: Arc <[ Block ]>, }, List( List ), CodeBlock( CodeBlock ), Table( Table ), ThematicBreak, RawBlock { format: FormatId , body: LiteralText , }, Container { blocks: Arc <[ Block ]>, },`
- Variant paths: `iyon_tui::text::BlockKind::Paragraph`, `iyon_tui::text::BlockKind::Heading`, `iyon_tui::text::BlockKind::BlockQuote`, `iyon_tui::text::BlockKind::List`, `iyon_tui::text::BlockKind::CodeBlock`, `iyon_tui::text::BlockKind::Table`, `iyon_tui::text::BlockKind::ThematicBreak`, `iyon_tui::text::BlockKind::RawBlock`, `iyon_tui::text::BlockKind::Container`.

#### `iyon_tui::text::BreakKind` — enum [re-export]
- Signature: `pub enum BreakKind { Soft, Hard, }`
- Purpose: Inline line-break semantics.
- Variants and fields: `Soft, Hard,`
- Variant paths: `iyon_tui::text::BreakKind::Soft`, `iyon_tui::text::BreakKind::Hard`.

#### `iyon_tui::text::CodeBlockLabelPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum CodeBlockLabelPolicy { Hidden, Language, Info, }`
- Purpose: Optional code-block label presentation.
- Variants and fields: `Hidden, Language, Info,`
- Variant paths: `iyon_tui::text::CodeBlockLabelPolicy::Hidden`, `iyon_tui::text::CodeBlockLabelPolicy::Language`, `iyon_tui::text::CodeBlockLabelPolicy::Info`.

#### `iyon_tui::text::InlineKind` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum InlineKind { Text( TextRun ), Break( BreakKind ), Image( Image ), RawInline { format: FormatId , body: LiteralText , }, }`
- Purpose: Generic inline semantic kind.
- Variants and fields: `Text( TextRun ), Break( BreakKind ), Image( Image ), RawInline { format: FormatId , body: LiteralText , },`
- Variant paths: `iyon_tui::text::InlineKind::Text`, `iyon_tui::text::InlineKind::Break`, `iyon_tui::text::InlineKind::Image`, `iyon_tui::text::InlineKind::RawInline`.

#### `iyon_tui::text::ListMarker` — enum [re-export]
- Signature: `pub enum ListMarker { Bullet, Ordered { start: u64 , style: NumberStyle , delimiter: NumberDelimiter , }, }`
- Purpose: Generic list marker semantics.
- Variants and fields: `Bullet, Ordered { start: u64 , style: NumberStyle , delimiter: NumberDelimiter , },`
- Variant paths: `iyon_tui::text::ListMarker::Bullet`, `iyon_tui::text::ListMarker::Ordered`.

#### `iyon_tui::text::Mark` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum Mark { Emphasis, Strong, Strikethrough, Underline, Superscript, Subscript, SmallCaps, Code, Link( LinkTarget ), }`
- Purpose: A generic inline formatting mark.
- Variants and fields: `Emphasis, Strong, Strikethrough, Underline, Superscript, Subscript, SmallCaps, Code, Link( LinkTarget ),`
- Variant paths: `iyon_tui::text::Mark::Emphasis`, `iyon_tui::text::Mark::Strong`, `iyon_tui::text::Mark::Strikethrough`, `iyon_tui::text::Mark::Underline`, `iyon_tui::text::Mark::Superscript`, `iyon_tui::text::Mark::Subscript`, `iyon_tui::text::Mark::SmallCaps`, `iyon_tui::text::Mark::Code`, `iyon_tui::text::Mark::Link`.

#### `iyon_tui::text::MarkdownProjectionError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum MarkdownProjectionError { Text( TextProjectionError ), Ir( TextIrError ), InvalidSourceMap { context: &'static str , }, InvalidNesting { context: &'static str , }, InsufficientRestartContext { source_base: StreamOffset , required_from: StreamOffset , }, ParserInvariant { context: &'static str , }, }`
- Purpose: Errors raised while converting CommonMark events to generic text IR.
- Variants and fields: `Text( TextProjectionError ), Ir( TextIrError ), InvalidSourceMap { context: &'static str , }, InvalidNesting { context: &'static str , }, InsufficientRestartContext { source_base: StreamOffset , required_from: StreamOffset , }, ParserInvariant { context: &'static str , },`
- Variant paths: `iyon_tui::text::MarkdownProjectionError::Text`, `iyon_tui::text::MarkdownProjectionError::Ir`, `iyon_tui::text::MarkdownProjectionError::InvalidSourceMap`, `iyon_tui::text::MarkdownProjectionError::InvalidNesting`, `iyon_tui::text::MarkdownProjectionError::InsufficientRestartContext`, `iyon_tui::text::MarkdownProjectionError::ParserInvariant`.

#### `iyon_tui::text::NumberDelimiter` — enum [re-export]
- Signature: `pub enum NumberDelimiter { Period, Paren, TwoParens, }`
- Purpose: Public enum `NumberDelimiter`.
- Variants and fields: `Period, Paren, TwoParens,`
- Variant paths: `iyon_tui::text::NumberDelimiter::Period`, `iyon_tui::text::NumberDelimiter::Paren`, `iyon_tui::text::NumberDelimiter::TwoParens`.

#### `iyon_tui::text::NumberStyle` — enum [re-export]
- Signature: `pub enum NumberStyle { Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman, }`
- Purpose: Public enum `NumberStyle`.
- Variants and fields: `Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman,`
- Variant paths: `iyon_tui::text::NumberStyle::Decimal`, `iyon_tui::text::NumberStyle::LowerAlpha`, `iyon_tui::text::NumberStyle::UpperAlpha`, `iyon_tui::text::NumberStyle::LowerRoman`, `iyon_tui::text::NumberStyle::UpperRoman`.

#### `iyon_tui::text::RewriteProjectionError` — enum [re-export]
- Signature: `pub enum RewriteProjectionError<E> { Rewrite(E), Invalid( TextProjectionError ), }`
- Purpose: Persistent recursive rewriting with identity preservation for unchanged paths.
- Variants and fields: `Rewrite(E), Invalid( TextProjectionError ),`
- Variant paths: `iyon_tui::text::RewriteProjectionError::Rewrite`, `iyon_tui::text::RewriteProjectionError::Invalid`.

#### `iyon_tui::text::SemanticValue` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum SemanticValue { Bool( bool ), Integer( i64 ), Text( Arc < str >), TextList( Arc <[ Arc < str >]>), }`
- Purpose: Small, typed annotation values.
- Variants and fields: `Bool( bool ), Integer( i64 ), Text( Arc < str >), TextList( Arc <[ Arc < str >]>),`
- Variant paths: `iyon_tui::text::SemanticValue::Bool`, `iyon_tui::text::SemanticValue::Integer`, `iyon_tui::text::SemanticValue::Text`, `iyon_tui::text::SemanticValue::TextList`.

#### `iyon_tui::text::SoftBreakPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum SoftBreakPolicy { Space, LineBreak, }`
- Purpose: How a soft line break is presented as ordinary semantic text.
- Variants and fields: `Space, LineBreak,`
- Variant paths: `iyon_tui::text::SoftBreakPolicy::Space`, `iyon_tui::text::SoftBreakPolicy::LineBreak`.

#### `iyon_tui::text::TableColumnSizing` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TableColumnSizing { Content, Flex, }`
- Purpose: Shared-column sizing for generic tables.
- Variants and fields: `Content, Flex,`
- Variant paths: `iyon_tui::text::TableColumnSizing::Content`, `iyon_tui::text::TableColumnSizing::Flex`.

#### `iyon_tui::text::TaskListMarkerPolicy` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TaskListMarkerPolicy { TaskOnly, TaskAndList, }`
- Purpose: How task-list items present checkbox chrome relative to the list marker.
- Variants and fields: `TaskOnly, TaskAndList,`
- Variant paths: `iyon_tui::text::TaskListMarkerPolicy::TaskOnly`, `iyon_tui::text::TaskListMarkerPolicy::TaskAndList`.

#### `iyon_tui::text::TextContent` — enum [re-export]
- Signature: `pub enum TextContent { Raw( RawText ), Block( Block ), }`
- Purpose: The closed set of generic text projection values.
- Variants and fields: `Raw( RawText ), Block( Block ),`
- Variant paths: `iyon_tui::text::TextContent::Raw`, `iyon_tui::text::TextContent::Block`.
- `iyon_tui::text::TextContent::raw` — inherent method; `pub fn raw (text: impl Into < Arc < str >>) -> Self` — Provides the public `raw` operation.
- `iyon_tui::text::TextContent::block` — inherent method; `pub fn block (block: Block ) -> Self` — Provides the public `block` operation.

#### `iyon_tui::text::TextIrError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextIrError { InvalidName, InvalidExactLength { text_len: u64 , range_len: u64 , }, InvalidHeadingLevel, InvalidTableHeaderRows { header_rows: usize , row_count: usize , }, TableCellDoesNotFit { row: usize , cell: usize , }, TableCellOverlaps { row: usize , cell: usize , column: usize , }, TableSpanExceedsRows { row: usize , cell: usize , }, InvalidSourceSlice { owner: StreamRange , local: StreamRange , }, DuplicateLinkMark, InvalidListConfiguration, NotCharBoundary, }`
- Purpose: Errors raised while constructing generic text semantic values.
- Variants and fields: `InvalidName, InvalidExactLength { text_len: u64 , range_len: u64 , }, InvalidHeadingLevel, InvalidTableHeaderRows { header_rows: usize , row_count: usize , }, TableCellDoesNotFit { row: usize , cell: usize , }, TableCellOverlaps { row: usize , cell: usize , column: usize , }, TableSpanExceedsRows { row: usize , cell: usize , }, InvalidSourceSlice { owner: StreamRange , local: StreamRange , }, DuplicateLinkMark, InvalidListConfiguration, NotCharBoundary,`
- Variant paths: `iyon_tui::text::TextIrError::InvalidName`, `iyon_tui::text::TextIrError::InvalidExactLength`, `iyon_tui::text::TextIrError::InvalidHeadingLevel`, `iyon_tui::text::TextIrError::InvalidTableHeaderRows`, `iyon_tui::text::TextIrError::TableCellDoesNotFit`, `iyon_tui::text::TextIrError::TableCellOverlaps`, `iyon_tui::text::TextIrError::TableSpanExceedsRows`, `iyon_tui::text::TextIrError::InvalidSourceSlice`, `iyon_tui::text::TextIrError::DuplicateLinkMark`, `iyon_tui::text::TextIrError::InvalidListConfiguration`, `iyon_tui::text::TextIrError::NotCharBoundary`.

#### `iyon_tui::text::TextListKind` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextListKind { Bullet, Ordered, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Bullet, Ordered,`
- Variant paths: `iyon_tui::text::TextListKind::Bullet`, `iyon_tui::text::TextListKind::Ordered`.

#### `iyon_tui::text::TextPart` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextPart { ListMarker, TaskMarker, QuoteMarker, CodeLabel, TableRule, ThematicRule, ImageFallback, }`
- Purpose: Renderer-generated presentation pieces, distinct from semantic TextRole s.
- Variants and fields: `ListMarker, TaskMarker, QuoteMarker, CodeLabel, TableRule, ThematicRule, ImageFallback,`
- Variant paths: `iyon_tui::text::TextPart::ListMarker`, `iyon_tui::text::TextPart::TaskMarker`, `iyon_tui::text::TextPart::QuoteMarker`, `iyon_tui::text::TextPart::CodeLabel`, `iyon_tui::text::TextPart::TableRule`, `iyon_tui::text::TextPart::ThematicRule`, `iyon_tui::text::TextPart::ImageFallback`.

#### `iyon_tui::text::TextProjectionError` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextProjectionError { Projection( ProjectionValidationError ), Ir( TextIrError ), RawMustBeSoleValue { source: StreamRange , }, RawByteLengthMismatch { source: StreamRange , text_len: u64 , }, NestedRangeOutsideSpan { owner: StreamRange , range: StreamRange , }, ExactLengthMismatch { range: StreamRange , text_len: u64 , }, }`
- Purpose: Errors raised when a crate::Projection contains invalid text IR.
- Variants and fields: `Projection( ProjectionValidationError ), Ir( TextIrError ), RawMustBeSoleValue { source: StreamRange , }, RawByteLengthMismatch { source: StreamRange , text_len: u64 , }, NestedRangeOutsideSpan { owner: StreamRange , range: StreamRange , }, ExactLengthMismatch { range: StreamRange , text_len: u64 , },`
- Variant paths: `iyon_tui::text::TextProjectionError::Projection`, `iyon_tui::text::TextProjectionError::Ir`, `iyon_tui::text::TextProjectionError::RawMustBeSoleValue`, `iyon_tui::text::TextProjectionError::RawByteLengthMismatch`, `iyon_tui::text::TextProjectionError::NestedRangeOutsideSpan`, `iyon_tui::text::TextProjectionError::ExactLengthMismatch`.

#### `iyon_tui::text::TextProvenance` — enum [re-export]
- Signature: `pub enum TextProvenance { Exact( StreamRange ), Derived( StreamRange ), Synthetic, }`
- Purpose: How an inline text run relates to root source bytes.
- Variants and fields: `Exact( StreamRange ), Derived( StreamRange ), Synthetic,`
- Variant paths: `iyon_tui::text::TextProvenance::Exact`, `iyon_tui::text::TextProvenance::Derived`, `iyon_tui::text::TextProvenance::Synthetic`.

#### `iyon_tui::text::TextRole` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextRole { Paragraph, Heading, BlockQuote, List, ListItem, CodeBlock, Table, TableRow, TableCell, ThematicBreak, RawBlock, Container, Strong, Emphasis, Strikethrough, Underline, Superscript, Subscript, SmallCaps, InlineCode, Link, Image, RawInline, }`
- Purpose: Semantic classification used by generic structured-text styling.
- Variants and fields: `Paragraph, Heading, BlockQuote, List, ListItem, CodeBlock, Table, TableRow, TableCell, ThematicBreak, RawBlock, Container, Strong, Emphasis, Strikethrough, Underline, Superscript, Subscript, SmallCaps, InlineCode, Link, Image, RawInline,`
- Variant paths: `iyon_tui::text::TextRole::Paragraph`, `iyon_tui::text::TextRole::Heading`, `iyon_tui::text::TextRole::BlockQuote`, `iyon_tui::text::TextRole::List`, `iyon_tui::text::TextRole::ListItem`, `iyon_tui::text::TextRole::CodeBlock`, `iyon_tui::text::TextRole::Table`, `iyon_tui::text::TextRole::TableRow`, `iyon_tui::text::TextRole::TableCell`, `iyon_tui::text::TextRole::ThematicBreak`, `iyon_tui::text::TextRole::RawBlock`, `iyon_tui::text::TextRole::Container`, `iyon_tui::text::TextRole::Strong`, `iyon_tui::text::TextRole::Emphasis`, `iyon_tui::text::TextRole::Strikethrough`, `iyon_tui::text::TextRole::Underline`, `iyon_tui::text::TextRole::Superscript`, `iyon_tui::text::TextRole::Subscript`, `iyon_tui::text::TextRole::SmallCaps`, `iyon_tui::text::TextRole::InlineCode`, `iyon_tui::text::TextRole::Link`, `iyon_tui::text::TextRole::Image`, `iyon_tui::text::TextRole::RawInline`.

#### `iyon_tui::text::TextTableSection` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextTableSection { Header, Body, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Header, Body,`
- Variant paths: `iyon_tui::text::TextTableSection::Header`, `iyon_tui::text::TextTableSection::Body`.

#### `iyon_tui::text::TextTaskState` — enum [re-export]
- Signature: `#[non_exhaustive] pub enum TextTaskState { Checked, Unchecked, }`
- Purpose: Therefore, when matching against variants of non-exhaustive enums, an extra wildcard arm must be added to account for any future variants.
- Variants and fields: `Checked, Unchecked,`
- Variant paths: `iyon_tui::text::TextTaskState::Checked`, `iyon_tui::text::TextTaskState::Unchecked`.

#### `iyon_tui::text::validate_text_content` — free function [re-export]
- Signature: `pub fn validate_text_content( content: & TextContent , owner: StreamRange , ) -> Result < () , TextProjectionError >`
- Purpose: Validates a standalone generic text content value within a source span.

#### `iyon_tui::text::validate_text_projection` — free function [re-export]
- Signature: `pub fn validate_text_projection( projection: & Projection < TextContent >, ) -> Result < () , TextProjectionError >`
- Purpose: Validates P1 projection contracts plus generic text IR provenance rules.

#### `iyon_tui::text::walk_block` — free function [re-export]
- Signature: `pub fn walk_block<V: TextVisitor + ? Sized >(visitor: &mut V , block: & Block )`
- Purpose: Public free function `walk_block`.

#### `iyon_tui::text::walk_content` — free function [re-export]
- Signature: `pub fn walk_content<V: TextVisitor + ? Sized >( visitor: &mut V , content: & TextContent , )`
- Purpose: Public free function `walk_content`.

#### `iyon_tui::text::walk_inline` — free function [re-export]
- Signature: `pub fn walk_inline<V: TextVisitor + ? Sized >(visitor: &mut V , inline: & Inline )`
- Purpose: Public free function `walk_inline`.

#### `iyon_tui::text::walk_inline_content` — free function [re-export]
- Signature: `pub fn walk_inline_content<V: TextVisitor + ? Sized >( visitor: &mut V , content: & InlineContent , )`
- Purpose: Public free function `walk_inline_content`.

#### `iyon_tui::text::walk_literal` — free function [re-export]
- Signature: `pub fn walk_literal<V: TextVisitor + ? Sized >( visitor: &mut V , literal: & LiteralText , )`
- Purpose: Public free function `walk_literal`.

#### `iyon_tui::text::walk_rewrite_block` — free function [re-export]
- Signature: `pub fn walk_rewrite_block<R: TextRewriter + ? Sized >( rewriter: &mut R , block: Block , ) -> Result < Block , R:: Error >`
- Purpose: Public free function `walk_rewrite_block`.

#### `iyon_tui::text::walk_rewrite_blocks` — free function [re-export]
- Signature: `pub fn walk_rewrite_blocks<R: TextRewriter + ? Sized >( rewriter: &mut R , blocks: Vec < Block >, ) -> Result < Vec < Block >, R:: Error >`
- Purpose: Public free function `walk_rewrite_blocks`.

#### `iyon_tui::text::walk_rewrite_content` — free function [re-export]
- Signature: `pub fn walk_rewrite_content<R: TextRewriter + ? Sized >( rewriter: &mut R , content: TextContent , ) -> Result < TextContent , R:: Error >`
- Purpose: Public free function `walk_rewrite_content`.

#### `iyon_tui::text::walk_rewrite_inline` — free function [re-export]
- Signature: `pub fn walk_rewrite_inline<R: TextRewriter + ? Sized >( rewriter: &mut R , inline: Inline , ) -> Result < Inline , R:: Error >`
- Purpose: Public free function `walk_rewrite_inline`.

#### `iyon_tui::text::walk_rewrite_inline_content` — free function [re-export]
- Signature: `pub fn walk_rewrite_inline_content<R: TextRewriter + ? Sized >( rewriter: &mut R , content: InlineContent , ) -> Result < InlineContent , R:: Error >`
- Purpose: Public free function `walk_rewrite_inline_content`.

#### `iyon_tui::text::walk_rewrite_literal` — free function [re-export]
- Signature: `pub fn walk_rewrite_literal<R: TextRewriter + ? Sized >( _rewriter: &mut R , literal: LiteralText , ) -> Result < LiteralText , R:: Error >`
- Purpose: Public free function `walk_rewrite_literal`.

#### `iyon_tui::text::Annotations` — struct [re-export]
- Signature: `pub struct Annotations { /* private fields */ }`
- Purpose: Immutable canonical semantic annotations.
- `iyon_tui::text::Annotations::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::text::Annotations::tags` — inherent method; `pub fn tags (&self) -> &[ SemanticTag ]` — Provides the public `tags` operation.
- `iyon_tui::text::Annotations::properties` — inherent method; `pub fn properties (&self) -> &[( SemanticKey , SemanticValue )]` — Provides the public `properties` operation.
- `iyon_tui::text::Annotations::add_tag` — inherent method; `pub fn add_tag (&self, tag: SemanticTag ) -> Self` — Provides the public `add_tag` operation.
- `iyon_tui::text::Annotations::with_tag` — inherent method; `pub fn with_tag (self, tag: SemanticTag ) -> Self` — Returns this value with `tag` configured.
- `iyon_tui::text::Annotations::set_property` — inherent method; `pub fn set_property ( &self, key: SemanticKey , value: impl Into < SemanticValue >, ) -> Self` — Sets `property` and returns the previous value when applicable.
- `iyon_tui::text::Annotations::with_property` — inherent method; `pub fn with_property ( self, key: SemanticKey , value: impl Into < SemanticValue >, ) -> Self` — Returns this value with `property` configured.
- `iyon_tui::text::Annotations::contains_tag` — inherent method; `pub fn contains_tag (&self, tag: & SemanticTag ) -> bool` — Reports whether the value contains the requested item.
- `iyon_tui::text::Annotations::property` — inherent method; `pub fn property (&self, key: & SemanticKey ) -> Option <& SemanticValue >` — Provides the public `property` operation.
- `iyon_tui::text::Annotations::with_origin` — inherent method; `pub fn with_origin (self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::Annotations::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::Block` — struct [re-export]
- Signature: `pub struct Block( /* private fields */ );`
- Purpose: Public struct `Block`.
- `iyon_tui::text::Block::new` — inherent method; `pub fn new (kind: BlockKind ) -> Self` — Constructs a value.
- `iyon_tui::text::Block::paragraph` — inherent method; `pub fn paragraph (content: impl Into < InlineContent >) -> Self` — Provides the public `paragraph` operation.
- `iyon_tui::text::Block::heading` — inherent method; `pub fn heading (level: HeadingLevel , content: impl Into < InlineContent >) -> Self` — Provides the public `heading` operation.
- `iyon_tui::text::Block::block_quote` — inherent method; `pub fn block_quote (blocks: impl IntoIterator <Item = Block >) -> Self` — Provides the public `block_quote` operation.
- `iyon_tui::text::Block::list` — inherent method; `pub fn list (list: List ) -> Self` — Provides the public `list` operation.
- `iyon_tui::text::Block::code` — inherent method; `pub fn code (code: CodeBlock ) -> Self` — Provides the public `code` operation.
- `iyon_tui::text::Block::table` — inherent method; `pub fn table (table: Table ) -> Self` — Provides the public `table` operation.
- `iyon_tui::text::Block::thematic_break` — inherent method; `pub fn thematic_break () -> Self` — Provides the public `thematic_break` operation.
- `iyon_tui::text::Block::raw` — inherent method; `pub fn raw (format: FormatId , body: LiteralText ) -> Self` — Provides the public `raw` operation.
- `iyon_tui::text::Block::container` — inherent method; `pub fn container (blocks: impl IntoIterator <Item = Block >) -> Self` — Provides the public `container` operation.
- `iyon_tui::text::Block::kind` — inherent method; `pub fn kind (&self) -> & BlockKind` — Returns `kind`.
- `iyon_tui::text::Block::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::Block::with_annotations` — inherent method; `pub fn with_annotations (&self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::Block::map_annotations` — inherent method; `pub fn map_annotations ( &self, map: impl FnOnce ( Annotations ) -> Annotations , ) -> Self` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::Block::as_code_block` — inherent method; `pub fn as_code_block (&self) -> Option <& CodeBlock >` — Converts or exposes this value.
- `iyon_tui::text::Block::as_list` — inherent method; `pub fn as_list (&self) -> Option <& List >` — Converts or exposes this value.
- `iyon_tui::text::Block::as_container` — inherent method; `pub fn as_container (&self) -> Option <&[ Block ]>` — Converts or exposes this value.
- `iyon_tui::text::Block::ptr_eq` — inherent method; `pub fn ptr_eq (&self, other: &Self) -> bool` — Provides the public `ptr_eq` operation.
- `iyon_tui::text::Block::with_origin` — inherent method; `pub fn with_origin (&self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::Block::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::CodeBlock` — struct [re-export]
- Signature: `pub struct CodeBlock { /* private fields */ }`
- Purpose: Public struct `CodeBlock`.
- `iyon_tui::text::CodeBlock::new` — inherent method; `pub fn new ( language: Option < LanguageId >, info: Option <impl Into < Arc < str >>>, body: impl Into < LiteralText >, ) -> Self` — Constructs a value.
- `iyon_tui::text::CodeBlock::language` — inherent method; `pub fn language (&self) -> Option <& LanguageId >` — Provides the public `language` operation.
- `iyon_tui::text::CodeBlock::info` — inherent method; `pub fn info (&self) -> Option <& str >` — Provides the public `info` operation.
- `iyon_tui::text::CodeBlock::body` — inherent method; `pub fn body (&self) -> & LiteralText` — Provides the public `body` operation.

#### `iyon_tui::text::FormatId` — struct [re-export]
- Signature: `pub struct FormatId( /* private fields */ );`
- Purpose: Format identifier for a literal embedded language.
- `iyon_tui::text::FormatId::new` — inherent method; `pub fn new (value: impl Into < Arc < str >>) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::FormatId::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::text::HeadingLevel` — struct [re-export]
- Signature: `pub struct HeadingLevel( /* private fields */ );`
- Purpose: Validated heading levels.
- `iyon_tui::text::HeadingLevel::H1` — associated const; `pub const H1 : Self` — Provides the public `H1` operation.
- `iyon_tui::text::HeadingLevel::H2` — associated const; `pub const H2 : Self` — Provides the public `H2` operation.
- `iyon_tui::text::HeadingLevel::H3` — associated const; `pub const H3 : Self` — Provides the public `H3` operation.
- `iyon_tui::text::HeadingLevel::H4` — associated const; `pub const H4 : Self` — Provides the public `H4` operation.
- `iyon_tui::text::HeadingLevel::H5` — associated const; `pub const H5 : Self` — Provides the public `H5` operation.
- `iyon_tui::text::HeadingLevel::H6` — associated const; `pub const H6 : Self` — Provides the public `H6` operation.
- `iyon_tui::text::HeadingLevel::new` — inherent method; `pub fn new (level: u8 ) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::HeadingLevel::get` — inherent method; `pub fn get (self) -> u8` — Provides the public `get` operation.

#### `iyon_tui::text::Image` — struct [re-export]
- Signature: `pub struct Image { /* private fields */ }`
- Purpose: A terminal image value with semantic alt content.
- `iyon_tui::text::Image::new` — inherent method; `pub fn new ( destination: impl Into < Arc < str >>, title: Option <impl Into < Arc < str >>>, alt: impl Into < InlineContent >, ) -> Self` — Constructs a value.
- `iyon_tui::text::Image::destination` — inherent method; `pub fn destination (&self) -> & str` — Returns `destination`.
- `iyon_tui::text::Image::title` — inherent method; `pub fn title (&self) -> Option <& str >` — Returns `title`.
- `iyon_tui::text::Image::alt` — inherent method; `pub fn alt (&self) -> & InlineContent` — Returns `alt`.

#### `iyon_tui::text::Inline` — struct [re-export]
- Signature: `pub struct Inline( /* private fields */ );`
- Purpose: Immutable inline semantic value.
- `iyon_tui::text::Inline::new` — inherent method; `pub fn new (kind: InlineKind ) -> Self` — Constructs a value.
- `iyon_tui::text::Inline::text` — inherent method; `pub fn text (run: impl Into < TextRun >) -> Self` — Returns `text`.
- `iyon_tui::text::Inline::break_` — inherent method; `pub fn break_ (kind: BreakKind ) -> Self` — Provides the public `break_` operation.
- `iyon_tui::text::Inline::image` — inherent method; `pub fn image (image: Image ) -> Self` — Provides the public `image` operation.
- `iyon_tui::text::Inline::raw` — inherent method; `pub fn raw (format: FormatId , body: LiteralText ) -> Self` — Provides the public `raw` operation.
- `iyon_tui::text::Inline::kind` — inherent method; `pub fn kind (&self) -> & InlineKind` — Returns `kind`.
- `iyon_tui::text::Inline::marks` — inherent method; `pub fn marks (&self) -> & MarkSet` — Returns `marks`.
- `iyon_tui::text::Inline::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::Inline::as_text` — inherent method; `pub fn as_text (&self) -> Option <& TextRun >` — Converts or exposes this value.
- `iyon_tui::text::Inline::with_mark` — inherent method; `pub fn with_mark (&self, mark: Mark ) -> Result <Self, TextIrError >` — Returns this value with `mark` configured.
- `iyon_tui::text::Inline::strong` — inherent method; `pub fn strong (&self) -> Self` — Provides the public `strong` operation.
- `iyon_tui::text::Inline::emphasis` — inherent method; `pub fn emphasis (&self) -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::text::Inline::strikethrough` — inherent method; `pub fn strikethrough (&self) -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::text::Inline::underline` — inherent method; `pub fn underline (&self) -> Self` — Provides the public `underline` operation.
- `iyon_tui::text::Inline::code` — inherent method; `pub fn code (&self) -> Self` — Provides the public `code` operation.
- `iyon_tui::text::Inline::with_link` — inherent method; `pub fn with_link (&self, target: LinkTarget ) -> Result <Self, TextIrError >` — Returns this value with `link` configured.
- `iyon_tui::text::Inline::with_marks` — inherent method; `pub fn with_marks (&self, marks: MarkSet ) -> Self` — Returns this value with `marks` configured.
- `iyon_tui::text::Inline::with_annotations` — inherent method; `pub fn with_annotations (&self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::Inline::ptr_eq` — inherent method; `pub fn ptr_eq (&self, other: &Self) -> bool` — Provides the public `ptr_eq` operation.
- `iyon_tui::text::Inline::map_annotations` — inherent method; `pub fn map_annotations ( &self, map: impl FnOnce ( Annotations ) -> Annotations , ) -> Self` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::Inline::with_origin` — inherent method; `pub fn with_origin (&self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::Inline::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::InlineContent` — struct [re-export]
- Signature: `pub struct InlineContent { /* private fields */ }`
- Purpose: Immutable ordered inline content.
- `iyon_tui::text::InlineContent::new` — inherent method; `pub fn new (items: impl IntoIterator <Item = Inline >) -> Self` — Constructs a value.
- `iyon_tui::text::InlineContent::empty` — inherent method; `pub fn empty () -> Self` — Constructs a value.
- `iyon_tui::text::InlineContent::items` — inherent method; `pub fn items (&self) -> &[ Inline ]` — Returns `items`.
- `iyon_tui::text::InlineContent::iter` — inherent method; `pub fn iter (&self) -> impl Iterator <Item = & Inline >` — Returns `iter`.
- `iyon_tui::text::InlineContent::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::text::InlineContent::len` — inherent method; `pub fn len (&self) -> usize` — Returns the number of contained items.
- `iyon_tui::text::InlineContent::with_mark` — inherent method; `pub fn with_mark (&self, mark: Mark ) -> Result <Self, TextIrError >` — Returns this value with `mark` configured.
- `iyon_tui::text::InlineContent::strong` — inherent method; `pub fn strong (&self) -> Self` — Provides the public `strong` operation.
- `iyon_tui::text::InlineContent::emphasis` — inherent method; `pub fn emphasis (&self) -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::text::InlineContent::code` — inherent method; `pub fn code (&self) -> Self` — Provides the public `code` operation.

#### `iyon_tui::text::LanguageId` — struct [re-export]
- Signature: `pub struct LanguageId( /* private fields */ );`
- Purpose: Language identifier used for nested code projectors.
- `iyon_tui::text::LanguageId::new` — inherent method; `pub fn new (value: impl Into < Arc < str >>) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::LanguageId::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::text::LinkTarget` — struct [re-export]
- Signature: `pub struct LinkTarget { /* private fields */ }`
- Purpose: A resolved link target.
- `iyon_tui::text::LinkTarget::new` — inherent method; `pub fn new ( destination: impl Into < Arc < str >>, title: Option <impl Into < Arc < str >>>, ) -> Self` — Constructs a value.
- `iyon_tui::text::LinkTarget::destination` — inherent method; `pub fn destination (&self) -> & str` — Returns `destination`.
- `iyon_tui::text::LinkTarget::title` — inherent method; `pub fn title (&self) -> Option <& str >` — Returns `title`.

#### `iyon_tui::text::List` — struct [re-export]
- Signature: `pub struct List { /* private fields */ }`
- Purpose: Public struct `List`.
- `iyon_tui::text::List::bulleted` — inherent method; `pub fn bulleted (items: impl IntoIterator <Item = ListItem >) -> Self` — Provides the public `bulleted` operation.
- `iyon_tui::text::List::ordered` — inherent method; `pub fn ordered (start: u64 , items: impl IntoIterator <Item = ListItem >) -> Self` — Provides the public `ordered` operation.
- `iyon_tui::text::List::new` — inherent method; `pub fn new ( marker: ListMarker , tight: bool , items: impl IntoIterator <Item = ListItem >, ) -> Self` — Constructs a value.
- `iyon_tui::text::List::marker` — inherent method; `pub fn marker (&self) -> ListMarker` — Returns `marker`.
- `iyon_tui::text::List::tight` — inherent method; `pub fn tight (&self) -> bool` — Returns `tight`.
- `iyon_tui::text::List::items` — inherent method; `pub fn items (&self) -> &[ ListItem ]` — Returns `items`.
- `iyon_tui::text::List::with_tight` — inherent method; `pub fn with_tight (self, tight: bool ) -> Self` — Returns this value with `tight` configured.
- `iyon_tui::text::List::with_number_style` — inherent method; `pub fn with_number_style (self, style: NumberStyle ) -> Result <Self, TextIrError >` — Returns this value with `number_style` configured.
- `iyon_tui::text::List::with_delimiter` — inherent method; `pub fn with_delimiter ( self, delimiter: NumberDelimiter , ) -> Result <Self, TextIrError >` — Returns this value with `delimiter` configured.

#### `iyon_tui::text::ListItem` — struct [re-export]
- Signature: `pub struct ListItem { /* private fields */ }`
- Purpose: Public struct `ListItem`.
- `iyon_tui::text::ListItem::paragraph` — inherent method; `pub fn paragraph (content: impl Into < InlineContent >) -> Self` — Provides the public `paragraph` operation.
- `iyon_tui::text::ListItem::task` — inherent method; `pub fn task (content: impl Into < InlineContent >, checked: bool ) -> Self` — Provides the public `task` operation.
- `iyon_tui::text::ListItem::new` — inherent method; `pub fn new (blocks: impl IntoIterator <Item = Block >) -> Self` — Constructs a value.
- `iyon_tui::text::ListItem::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::ListItem::checked` — inherent method; `pub fn checked (&self) -> Option < bool >` — Provides the public `checked` operation.
- `iyon_tui::text::ListItem::blocks` — inherent method; `pub fn blocks (&self) -> &[ Block ]` — Returns `blocks`.
- `iyon_tui::text::ListItem::with_annotations` — inherent method; `pub fn with_annotations (self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::ListItem::with_checked` — inherent method; `pub fn with_checked (self, checked: Option < bool >) -> Self` — Returns this value with `checked` configured.
- `iyon_tui::text::ListItem::with_origin` — inherent method; `pub fn with_origin (self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::ListItem::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::LiteralText` — struct [re-export]
- Signature: `pub struct LiteralText { /* private fields */ }`
- Purpose: Text intentionally exposed to a nested-language projector.
- `iyon_tui::text::LiteralText::new` — inherent method; `pub fn new (runs: impl IntoIterator <Item = TextRun >) -> Self` — Constructs a value.
- `iyon_tui::text::LiteralText::from_exact` — inherent method; `pub fn from_exact ( text: impl Into < Arc < str >>, range: StreamRange , ) -> Result <Self, TextIrError >` — Provides the public `from_exact` operation.
- `iyon_tui::text::LiteralText::runs` — inherent method; `pub fn runs (&self) -> &[ TextRun ]` — Performs the requested application operation.
- `iyon_tui::text::LiteralText::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::text::LiteralText::text` — inherent method; `pub fn text (&self) -> String` — Returns `text`.

#### `iyon_tui::text::MarkSet` — struct [re-export]
- Signature: `pub struct MarkSet( /* private fields */ );`
- Purpose: Canonical, order-independent inline marks.
- `iyon_tui::text::MarkSet::new` — inherent method; `pub fn new (marks: impl IntoIterator <Item = Mark >) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::MarkSet::empty` — inherent method; `pub fn empty () -> Self` — Constructs a value.
- `iyon_tui::text::MarkSet::marks` — inherent method; `pub fn marks (&self) -> &[ Mark ]` — Returns `marks`.
- `iyon_tui::text::MarkSet::with_mark` — inherent method; `pub fn with_mark (&self, mark: Mark ) -> Result <Self, TextIrError >` — Returns this value with `mark` configured.
- `iyon_tui::text::MarkSet::contains` — inherent method; `pub fn contains (&self, mark: & Mark ) -> bool` — Reports whether the value contains the requested item.

#### `iyon_tui::text::MarkdownOptions` — struct [re-export]
- Signature: `pub struct MarkdownOptions { /* private fields */ }`
- Purpose: Explicitly selected Markdown extensions supported by super::MarkdownProjector .
- `iyon_tui::text::MarkdownOptions::commonmark` — inherent method; `pub const fn commonmark () -> Self` — Constructs a value.
- `iyon_tui::text::MarkdownOptions::gfm` — inherent method; `pub const fn gfm () -> Self` — Constructs a value.
- `iyon_tui::text::MarkdownOptions::with_tables` — inherent method; `pub const fn with_tables (self, enabled: bool ) -> Self` — Returns this value with `tables` configured.
- `iyon_tui::text::MarkdownOptions::with_strikethrough` — inherent method; `pub const fn with_strikethrough (self, enabled: bool ) -> Self` — Returns this value with `strikethrough` configured.
- `iyon_tui::text::MarkdownOptions::with_task_lists` — inherent method; `pub const fn with_task_lists (self, enabled: bool ) -> Self` — Returns this value with `task_lists` configured.
- `iyon_tui::text::MarkdownOptions::with_live_table_stabilization` — inherent method; `pub const fn with_live_table_stabilization (self, enabled: bool ) -> Self` — Returns this value with `live_table_stabilization` configured.
- `iyon_tui::text::MarkdownOptions::tables` — inherent method; `pub const fn tables (self) -> bool` — Provides the public `tables` operation.
- `iyon_tui::text::MarkdownOptions::strikethrough` — inherent method; `pub const fn strikethrough (self) -> bool` — Provides the public `strikethrough` operation.
- `iyon_tui::text::MarkdownOptions::task_lists` — inherent method; `pub const fn task_lists (self) -> bool` — Provides the public `task_lists` operation.
- `iyon_tui::text::MarkdownOptions::live_table_stabilization` — inherent method; `pub const fn live_table_stabilization (self) -> bool` — Provides the public `live_table_stabilization` operation.

#### `iyon_tui::text::MarkdownProjector` — struct [re-export]
- Signature: `pub struct MarkdownProjector { /* private fields */ }`
- Purpose: Stateful, non-temporal CommonMark-to-TextContent projector.
- `iyon_tui::text::MarkdownProjector::new` — inherent method; `pub fn new (options: MarkdownOptions ) -> Self` — Constructs a value.
- `iyon_tui::text::MarkdownProjector::options` — inherent method; `pub fn options (&self) -> MarkdownOptions` — Returns `options`.

#### `iyon_tui::text::PlainTextProjector` — struct [re-export]
- Signature: `pub struct PlainTextProjector;`
- Purpose: A projector that claims each consecutive Raw domain as literal prose.
- `iyon_tui::text::PlainTextProjector::new` — inherent method; `pub fn new () -> Self` — Constructs a value.

#### `iyon_tui::text::RawText` — struct [re-export]
- Signature: `pub struct RawText( /* private fields */ );`
- Purpose: Exact, unclaimed text at the root of a text projection.
- `iyon_tui::text::RawText::new` — inherent method; `pub fn new (text: impl Into < Arc < str >>) -> Self` — Constructs a value.
- `iyon_tui::text::RawText::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::text::RawText::is_empty` — inherent method; `pub fn is_empty (&self) -> bool` — Reports whether `empty` holds.
- `iyon_tui::text::RawText::len` — inherent method; `pub fn len (&self) -> usize` — Returns the number of contained items.
- `iyon_tui::text::RawText::exact_slice` — inherent method; `pub fn exact_slice ( &self, owner: StreamRange , local: Range < usize >, ) -> Result < TextRun , TextIrError >` — Provides the public `exact_slice` operation.

#### `iyon_tui::text::RewriteProjector` — struct [re-export]
- Signature: `pub struct RewriteProjector<R> { /* private fields */ }`
- Purpose: Explicit adapter for envelope-preserving TextRewriter values.
- `iyon_tui::text::RewriteProjector::new` — inherent method; `pub fn new (rewriter: R) -> Self` — Constructs a value.

#### `iyon_tui::text::SemanticKey` — struct [re-export]
- Signature: `pub struct SemanticKey { /* private fields */ }`
- Purpose: A namespaced semantic property key.
- `iyon_tui::text::SemanticKey::new` — inherent method; `pub fn new ( namespace: impl Into < Arc < str >>, name: impl Into < Arc < str >>, ) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::SemanticKey::namespace` — inherent method; `pub fn namespace (&self) -> & str` — Returns `namespace`.
- `iyon_tui::text::SemanticKey::name` — inherent method; `pub fn name (&self) -> & str` — Returns `name`.

#### `iyon_tui::text::SemanticTag` — struct [re-export]
- Signature: `pub struct SemanticTag { /* private fields */ }`
- Purpose: A namespaced semantic tag.
- `iyon_tui::text::SemanticTag::new` — inherent method; `pub fn new ( namespace: impl Into < Arc < str >>, name: impl Into < Arc < str >>, ) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::SemanticTag::namespace` — inherent method; `pub fn namespace (&self) -> & str` — Returns `namespace`.
- `iyon_tui::text::SemanticTag::name` — inherent method; `pub fn name (&self) -> & str` — Returns `name`.

#### `iyon_tui::text::Table` — struct [re-export]
- Signature: `pub struct Table { /* private fields */ }`
- Purpose: Public struct `Table`.
- `iyon_tui::text::Table::new` — inherent method; `pub fn new ( caption: Option <impl IntoIterator <Item = Block >>, columns: impl IntoIterator <Item = TableColumn >, header_rows: usize , rows: impl IntoIterator <Item = TableRow >, ) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::Table::validate` — inherent method; `pub fn validate (&self) -> Result < () , TextIrError >` — Performs the fallible validation or operation.
- `iyon_tui::text::Table::caption` — inherent method; `pub fn caption (&self) -> Option <&[ Block ]>` — Returns `caption`.
- `iyon_tui::text::Table::columns` — inherent method; `pub fn columns (&self) -> &[ TableColumn ]` — Returns `columns`.
- `iyon_tui::text::Table::header_rows` — inherent method; `pub fn header_rows (&self) -> usize` — Returns `header_rows`.
- `iyon_tui::text::Table::rows` — inherent method; `pub fn rows (&self) -> &[ TableRow ]` — Returns `rows`.

#### `iyon_tui::text::TableCell` — struct [re-export]
- Signature: `pub struct TableCell { /* private fields */ }`
- Purpose: Public struct `TableCell`.
- `iyon_tui::text::TableCell::text` — inherent method; `pub fn text (content: impl Into < InlineContent >) -> Self` — Returns `text`.
- `iyon_tui::text::TableCell::new` — inherent method; `pub fn new ( blocks: impl IntoIterator <Item = Block >, alignment: Option < Alignment >, row_span: NonZeroU16 , col_span: NonZeroU16 , ) -> Self` — Constructs a value.
- `iyon_tui::text::TableCell::plain` — inherent method; `pub fn plain (blocks: impl IntoIterator <Item = Block >) -> Self` — Provides the public `plain` operation.
- `iyon_tui::text::TableCell::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::TableCell::alignment` — inherent method; `pub fn alignment (&self) -> Option < Alignment >` — Returns `alignment`.
- `iyon_tui::text::TableCell::row_span` — inherent method; `pub fn row_span (&self) -> NonZeroU16` — Returns `row_span`.
- `iyon_tui::text::TableCell::col_span` — inherent method; `pub fn col_span (&self) -> NonZeroU16` — Returns `col_span`.
- `iyon_tui::text::TableCell::blocks` — inherent method; `pub fn blocks (&self) -> &[ Block ]` — Returns `blocks`.
- `iyon_tui::text::TableCell::with_annotations` — inherent method; `pub fn with_annotations (self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::TableCell::with_origin` — inherent method; `pub fn with_origin (self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::TableCell::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::TableColumn` — struct [re-export]
- Signature: `pub struct TableColumn { /* private fields */ }`
- Purpose: Public struct `TableColumn`.
- `iyon_tui::text::TableColumn::start` — inherent method; `pub const fn start () -> Self` — Performs the requested application operation.
- `iyon_tui::text::TableColumn::center` — inherent method; `pub const fn center () -> Self` — Provides the public `center` operation.
- `iyon_tui::text::TableColumn::end` — inherent method; `pub const fn end () -> Self` — Provides the public `end` operation.
- `iyon_tui::text::TableColumn::new` — inherent method; `pub const fn new (alignment: Alignment ) -> Self` — Constructs a value.
- `iyon_tui::text::TableColumn::alignment` — inherent method; `pub fn alignment (&self) -> Alignment` — Returns `alignment`.

#### `iyon_tui::text::TableRow` — struct [re-export]
- Signature: `pub struct TableRow { /* private fields */ }`
- Purpose: Public struct `TableRow`.
- `iyon_tui::text::TableRow::new` — inherent method; `pub fn new (cells: impl IntoIterator <Item = TableCell >) -> Self` — Constructs a value.
- `iyon_tui::text::TableRow::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::TableRow::cells` — inherent method; `pub fn cells (&self) -> &[ TableCell ]` — Returns `cells`.
- `iyon_tui::text::TableRow::with_annotations` — inherent method; `pub fn with_annotations (self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::TableRow::with_origin` — inherent method; `pub fn with_origin (self, origin: TextOrigin ) -> Self` — Returns this value with `origin` configured.
- `iyon_tui::text::TableRow::origin` — inherent method; `pub fn origin (&self) -> Option < TextOrigin >` — Returns `origin`.

#### `iyon_tui::text::TextOrigin` — struct [re-export]
- Signature: `pub struct TextOrigin( /* private fields */ );`
- Purpose: Identifies the syntax/projector that claimed a semantic text value.
- `iyon_tui::text::TextOrigin::MARKDOWN` — associated const; `pub const MARKDOWN : Self` — Provides the public `MARKDOWN` operation.
- `iyon_tui::text::TextOrigin::PLAIN_TEXT` — associated const; `pub const PLAIN_TEXT : Self` — Provides the public `PLAIN_TEXT` operation.
- `iyon_tui::text::TextOrigin::markdown` — inherent method; `pub fn markdown () -> Self` — Provides the public `markdown` operation.
- `iyon_tui::text::TextOrigin::plain_text` — inherent method; `pub fn plain_text () -> Self` — Provides the public `plain_text` operation.
- `iyon_tui::text::TextOrigin::new` — inherent method; `pub fn new (value: impl Into < Arc < str >>) -> Result <Self, TextIrError >` — Constructs a value.
- `iyon_tui::text::TextOrigin::as_str` — inherent method; `pub fn as_str (&self) -> & str` — Converts or exposes this value.

#### `iyon_tui::text::TextRenderPolicy` — struct [re-export]
- Signature: `pub struct TextRenderPolicy { /* private fields */ }`
- Purpose: Structural-only policy for generic text-to-View lowering.
- `iyon_tui::text::TextRenderPolicy::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::text::TextRenderPolicy::block_gap` — inherent method; `pub fn block_gap (&self) -> u16` — Provides the public `block_gap` operation.
- `iyon_tui::text::TextRenderPolicy::with_block_gap` — inherent method; `pub fn with_block_gap (self, gap: u16 ) -> Self` — Returns this value with `block_gap` configured.
- `iyon_tui::text::TextRenderPolicy::soft_break` — inherent method; `pub fn soft_break (&self) -> SoftBreakPolicy` — Provides the public `soft_break` operation.
- `iyon_tui::text::TextRenderPolicy::with_soft_break` — inherent method; `pub fn with_soft_break (self, policy: SoftBreakPolicy ) -> Self` — Returns this value with `soft_break` configured.
- `iyon_tui::text::TextRenderPolicy::table_column_gap` — inherent method; `pub fn table_column_gap (&self) -> u16` — Provides the public `table_column_gap` operation.
- `iyon_tui::text::TextRenderPolicy::with_table_column_gap` — inherent method; `pub fn with_table_column_gap (self, gap: u16 ) -> Self` — Returns this value with `table_column_gap` configured.
- `iyon_tui::text::TextRenderPolicy::table_row_gap` — inherent method; `pub fn table_row_gap (&self) -> u16` — Provides the public `table_row_gap` operation.
- `iyon_tui::text::TextRenderPolicy::with_table_row_gap` — inherent method; `pub fn with_table_row_gap (self, gap: u16 ) -> Self` — Returns this value with `table_row_gap` configured.
- `iyon_tui::text::TextRenderPolicy::table_column_sizing` — inherent method; `pub fn table_column_sizing (&self) -> TableColumnSizing` — Provides the public `table_column_sizing` operation.
- `iyon_tui::text::TextRenderPolicy::with_table_column_sizing` — inherent method; `pub fn with_table_column_sizing (self, sizing: TableColumnSizing ) -> Self` — Returns this value with `table_column_sizing` configured.
- `iyon_tui::text::TextRenderPolicy::task_list_marker` — inherent method; `pub fn task_list_marker (&self) -> TaskListMarkerPolicy` — Provides the public `task_list_marker` operation.
- `iyon_tui::text::TextRenderPolicy::with_task_list_marker` — inherent method; `pub fn with_task_list_marker (self, policy: TaskListMarkerPolicy ) -> Self` — Returns this value with `task_list_marker` configured.
- `iyon_tui::text::TextRenderPolicy::code_block_label` — inherent method; `pub fn code_block_label (&self) -> CodeBlockLabelPolicy` — Provides the public `code_block_label` operation.
- `iyon_tui::text::TextRenderPolicy::with_code_block_label` — inherent method; `pub fn with_code_block_label (self, policy: CodeBlockLabelPolicy ) -> Self` — Returns this value with `code_block_label` configured.
- `iyon_tui::text::TextRenderPolicy::code_block_gap` — inherent method; `pub fn code_block_gap (&self) -> u16` — Provides the public `code_block_gap` operation.
- `iyon_tui::text::TextRenderPolicy::with_code_block_gap` — inherent method; `pub fn with_code_block_gap (self, gap: u16 ) -> Self` — Returns this value with `code_block_gap` configured.
- `iyon_tui::text::TextRenderPolicy::code_wrap` — inherent method; `pub fn code_wrap (&self) -> WrapMode` — Provides the public `code_wrap` operation.
- `iyon_tui::text::TextRenderPolicy::with_code_wrap` — inherent method; `pub fn with_code_wrap (self, wrap: WrapMode ) -> Self` — Returns this value with `code_wrap` configured.

#### `iyon_tui::text::TextRenderer` — struct [re-export]
- Signature: `pub struct TextRenderer { /* private fields */ }`
- Purpose: The one generic renderer for the frozen text IR.
- `iyon_tui::text::TextRenderer::new` — inherent method; `pub fn new () -> Self` — Constructs a value.
- `iyon_tui::text::TextRenderer::with_policy` — inherent method; `pub fn with_policy (policy: TextRenderPolicy ) -> Self` — Returns this value with `policy` configured.
- `iyon_tui::text::TextRenderer::policy` — inherent method; `pub fn policy (&self) -> & TextRenderPolicy` — Returns `policy`.
- `iyon_tui::text::TextRenderer::render_block` — inherent method; `pub fn render_block (&self, block: & Block ) -> View` — Renders the semantic value.

#### `iyon_tui::text::TextRun` — struct [re-export]
- Signature: `pub struct TextRun { /* private fields */ }`
- Purpose: Immutable text with provenance and optional semantic annotations.
- `iyon_tui::text::TextRun::exact` — inherent method; `pub fn exact ( text: impl Into < Arc < str >>, range: StreamRange , ) -> Result <Self, TextIrError >` — Provides the public `exact` operation.
- `iyon_tui::text::TextRun::derived` — inherent method; `pub fn derived (text: impl Into < Arc < str >>, range: StreamRange ) -> Self` — Provides the public `derived` operation.
- `iyon_tui::text::TextRun::synthetic` — inherent method; `pub fn synthetic (text: impl Into < Arc < str >>) -> Self` — Provides the public `synthetic` operation.
- `iyon_tui::text::TextRun::ptr_eq` — inherent method; `pub fn ptr_eq (&self, other: &Self) -> bool` — Provides the public `ptr_eq` operation.
- `iyon_tui::text::TextRun::text` — inherent method; `pub fn text (&self) -> & str` — Returns `text`.
- `iyon_tui::text::TextRun::provenance` — inherent method; `pub fn provenance (&self) -> & TextProvenance` — Provides the public `provenance` operation.
- `iyon_tui::text::TextRun::annotations` — inherent method; `pub fn annotations (&self) -> & Annotations` — Returns `annotations`.
- `iyon_tui::text::TextRun::map_annotations` — inherent method; `pub fn map_annotations ( self, map: impl FnOnce ( Annotations ) -> Annotations , ) -> Self` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRun::with_annotations` — inherent method; `pub fn with_annotations (self, annotations: Annotations ) -> Self` — Returns this value with `annotations` configured.
- `iyon_tui::text::TextRun::split_at` — inherent method; `pub fn split_at (&self, byte_offset: usize ) -> Result <(Self, Self), TextIrError >` — Splits the value at the requested boundary.

#### `iyon_tui::text::TextSelector` — struct [re-export]
- Signature: `pub struct TextSelector { /* private fields */ }`
- Purpose: Typed selector for generic structured-text presentation.
- `iyon_tui::text::TextSelector::any` — inherent method; `pub fn any () -> Self` — Constructs a value.
- `iyon_tui::text::TextSelector::role` — inherent method; `pub fn role (role: TextRole ) -> Self` — Provides the public `role` operation.
- `iyon_tui::text::TextSelector::part` — inherent method; `pub fn part (part: TextPart ) -> Self` — Provides the public `part` operation.
- `iyon_tui::text::TextSelector::annotation` — inherent method; `pub fn annotation (tag: & SemanticTag ) -> Self` — Provides the public `annotation` operation.
- `iyon_tui::text::TextSelector::paragraph` — inherent method; `pub fn paragraph () -> Self` — Provides the public `paragraph` operation.
- `iyon_tui::text::TextSelector::heading` — inherent method; `pub fn heading () -> Self` — Provides the public `heading` operation.
- `iyon_tui::text::TextSelector::block_quote` — inherent method; `pub fn block_quote () -> Self` — Provides the public `block_quote` operation.
- `iyon_tui::text::TextSelector::list` — inherent method; `pub fn list () -> Self` — Provides the public `list` operation.
- `iyon_tui::text::TextSelector::list_item` — inherent method; `pub fn list_item () -> Self` — Provides the public `list_item` operation.
- `iyon_tui::text::TextSelector::code_block` — inherent method; `pub fn code_block () -> Self` — Provides the public `code_block` operation.
- `iyon_tui::text::TextSelector::table` — inherent method; `pub fn table () -> Self` — Provides the public `table` operation.
- `iyon_tui::text::TextSelector::table_row` — inherent method; `pub fn table_row () -> Self` — Provides the public `table_row` operation.
- `iyon_tui::text::TextSelector::table_cell` — inherent method; `pub fn table_cell () -> Self` — Provides the public `table_cell` operation.
- `iyon_tui::text::TextSelector::thematic_break` — inherent method; `pub fn thematic_break () -> Self` — Provides the public `thematic_break` operation.
- `iyon_tui::text::TextSelector::strong` — inherent method; `pub fn strong () -> Self` — Provides the public `strong` operation.
- `iyon_tui::text::TextSelector::emphasis` — inherent method; `pub fn emphasis () -> Self` — Provides the public `emphasis` operation.
- `iyon_tui::text::TextSelector::strikethrough` — inherent method; `pub fn strikethrough () -> Self` — Provides the public `strikethrough` operation.
- `iyon_tui::text::TextSelector::underline` — inherent method; `pub fn underline () -> Self` — Provides the public `underline` operation.
- `iyon_tui::text::TextSelector::inline_code` — inherent method; `pub fn inline_code () -> Self` — Provides the public `inline_code` operation.
- `iyon_tui::text::TextSelector::link` — inherent method; `pub fn link () -> Self` — Provides the public `link` operation.
- `iyon_tui::text::TextSelector::and_role` — inherent method; `pub fn and_role (self, role: TextRole ) -> Self` — Provides the public `and_role` operation.
- `iyon_tui::text::TextSelector::level` — inherent method; `pub fn level (self, level: HeadingLevel ) -> Self` — Provides the public `level` operation.
- `iyon_tui::text::TextSelector::origin` — inherent method; `pub fn origin (self, origin: TextOrigin ) -> Self` — Returns `origin`.
- `iyon_tui::text::TextSelector::list_kind` — inherent method; `pub fn list_kind (self, kind: TextListKind ) -> Self` — Provides the public `list_kind` operation.
- `iyon_tui::text::TextSelector::task_state` — inherent method; `pub fn task_state (self, state: TextTaskState ) -> Self` — Provides the public `task_state` operation.
- `iyon_tui::text::TextSelector::table_section` — inherent method; `pub fn table_section (self, section: TextTableSection ) -> Self` — Provides the public `table_section` operation.
- `iyon_tui::text::TextSelector::language` — inherent method; `pub fn language (self, language: & LanguageId ) -> Self` — Provides the public `language` operation.
- `iyon_tui::text::TextSelector::format` — inherent method; `pub fn format (self, format: & FormatId ) -> Self` — Provides the public `format` operation.
- `iyon_tui::text::TextSelector::and_annotation` — inherent method; `pub fn and_annotation (self, tag: & SemanticTag ) -> Self` — Provides the public `and_annotation` operation.
- `iyon_tui::text::TextSelector::and_focused` — inherent method; `pub fn and_focused (self) -> Self` — Provides the public `and_focused` operation.
- `iyon_tui::text::TextSelector::and_focus_within` — inherent method; `pub fn and_focus_within (self) -> Self` — Provides the public `and_focus_within` operation.
- `iyon_tui::text::TextSelector::and_state` — inherent method; `pub fn and_state ( self, key: impl Into < StyleStateKey >, value: impl Into < StyleStateValue >, ) -> Self` — Provides the public `and_state` operation.

#### `iyon_tui::text::Renderer` — trait [re-export]
- Signature: `pub trait Renderer<Input: ? Sized > { // Required method fn render (&self, input: &Input ) -> View ; }`
- Purpose: Converts a semantic value into the generic presentation View .
- `iyon_tui::text::Renderer::render` — required method; `fn render (&self, input: &Input ) -> View` — Renders the semantic value.

#### `iyon_tui::text::TextRewriter` — trait [re-export]
- Signature: `pub trait TextRewriter { type Error ; // Provided methods fn into_projector (self) -> RewriteProjector <Self> where Self: Sized { ... } fn rewrite_content ( &mut self, content: TextContent , ) -> Result < TextContent , Self:: Error > { ... } fn rewrite_block (&mut self, block: Block ) -> Result < Block , Self:: Error > { ... } fn rewrite_inline (&mut self, inline: Inline ) -> Result < Inline , Self:: Error > { ... } fn rewrite_inline_content ( &mut self, content: InlineContent , ) -> Result < InlineContent , Self:: Error > { ... } fn rewrite_blocks ( &mut self, blocks: Vec < Block >, ) -> Result < Vec < Block >, Self:: Error > { ... } fn rewrite_literal ( &mut self, literal: LiteralText , ) -> Result < LiteralText , Self:: Error > { ... } }`
- Purpose: Public trait `TextRewriter`.
- `iyon_tui::text::TextRewriter::Error` — associated type; `type Error` — Rewrite error type.
- `iyon_tui::text::TextRewriter::into_projector` — provided method; `fn into_projector (self) -> RewriteProjector <Self> where Self: Sized ,` — Converts or exposes this value.
- `iyon_tui::text::TextRewriter::rewrite_content` — provided method; `fn rewrite_content ( &mut self, content: TextContent , ) -> Result < TextContent , Self:: Error >` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRewriter::rewrite_block` — provided method; `fn rewrite_block (&mut self, block: Block ) -> Result < Block , Self:: Error >` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRewriter::rewrite_inline` — provided method; `fn rewrite_inline (&mut self, inline: Inline ) -> Result < Inline , Self:: Error >` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRewriter::rewrite_inline_content` — provided method; `fn rewrite_inline_content ( &mut self, content: InlineContent , ) -> Result < InlineContent , Self:: Error >` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRewriter::rewrite_blocks` — provided method; `fn rewrite_blocks ( &mut self, blocks: Vec < Block >, ) -> Result < Vec < Block >, Self:: Error >` — Maps or rewrites the contained semantic value.
- `iyon_tui::text::TextRewriter::rewrite_literal` — provided method; `fn rewrite_literal ( &mut self, literal: LiteralText , ) -> Result < LiteralText , Self:: Error >` — Maps or rewrites the contained semantic value.

#### `iyon_tui::text::TextVisitor` — trait [re-export]
- Signature: `pub trait TextVisitor { // Provided methods fn visit_content (&mut self, content: & TextContent ) { ... } fn visit_raw (&mut self, _raw: & RawText ) { ... } fn visit_text_run (&mut self, _run: & TextRun ) { ... } fn visit_block (&mut self, block: & Block ) { ... } fn visit_inline (&mut self, inline: & Inline ) { ... } fn visit_literal (&mut self, literal: & LiteralText ) { ... } }`
- Purpose: Read-only traversal over all generic text IR descendants.
- `iyon_tui::text::TextVisitor::visit_content` — provided method; `fn visit_content (&mut self, content: & TextContent )` — Traverses the semantic structure.
- `iyon_tui::text::TextVisitor::visit_raw` — provided method; `fn visit_raw (&mut self, _raw: & RawText )` — Traverses the semantic structure.
- `iyon_tui::text::TextVisitor::visit_text_run` — provided method; `fn visit_text_run (&mut self, _run: & TextRun )` — Traverses the semantic structure.
- `iyon_tui::text::TextVisitor::visit_block` — provided method; `fn visit_block (&mut self, block: & Block )` — Traverses the semantic structure.
- `iyon_tui::text::TextVisitor::visit_inline` — provided method; `fn visit_inline (&mut self, inline: & Inline )` — Traverses the semantic structure.
- `iyon_tui::text::TextVisitor::visit_literal` — provided method; `fn visit_literal (&mut self, literal: & LiteralText )` — Traverses the semantic structure.

## iyon_tui::testing — feature test-util

#### `iyon_tui::testing::start` — free function [definition; feature `test-util`]
- Signature: `pub fn start<State, Action, Error, Init, Update, ViewFn>( app: App <State, Action, Error, Init, Update, ViewFn>, width: u16 , height: u16 , ) -> Result < AppHarness <State, Action, Error, Update, ViewFn>, RunError <Error>> where Init: FnOnce (&mut AppCx <'_, Action>) -> Result <State, Error>, Update: FnMut ( &mut State , Action, &mut AppCx <'_, Action>) -> Result < () , Error>, ViewFn: Fn ( &State ) -> View ,`
- Purpose: Starts an application using the same initialization and initial-frame ordering as the production runtime.

#### `iyon_tui::testing::AppHarness` — struct [definition; feature `test-util`]
- Signature: `pub struct AppHarness<State, Action, Error, Update, ViewFn> { /* private fields */ }`
- Purpose: A deterministic semantic application driver for integration tests.
- `iyon_tui::testing::AppHarness::handle` — inherent method; `pub fn handle (&self) -> AppHandle <Action>` — Returns `handle`.
- `iyon_tui::testing::AppHarness::key` — inherent method; `pub fn key (&mut self, key: KeyStroke ) -> Result < () , RunError <Error>>` — Returns `key`.
- `iyon_tui::testing::AppHarness::paste` — inherent method; `pub fn paste (&mut self, text: & str ) -> Result < () , RunError <Error>>` — Performs the requested application operation.
- `iyon_tui::testing::AppHarness::step` — inherent method; `pub fn step (&mut self) -> Result < bool , RunError <Error>>` — Performs the requested application operation.
- `iyon_tui::testing::AppHarness::advance_time` — inherent method; `pub fn advance_time ( &mut self, duration: Duration , ) -> Result < bool , RunError <Error>>` — Advances state or returns a temporal coordinate.
- `iyon_tui::testing::AppHarness::resize` — inherent method; `pub fn resize (&mut self, width: u16 , height: u16 ) -> Result < () , RunError <Error>>` — Performs the requested application operation.
- `iyon_tui::testing::AppHarness::screen_lines` — inherent method; `pub fn screen_lines (&self) -> Vec < String >` — Returns compiled or painted information.
- `iyon_tui::testing::AppHarness::native_history_lines` — inherent method; `pub fn native_history_lines (&self) -> Vec < String >` — Returns compiled or painted information.
- `iyon_tui::testing::AppHarness::is_exiting` — inherent method; `pub fn is_exiting (&self) -> bool` — Reports whether `exiting` holds.

## Surface notes

- No reachable public type aliases, free const items, free static items, or public struct fields were found.
- Root paths are exposed through pub use statements in lib.rs; their entries above are marked [re-export].
- iyon_tui::testing and its items exist only with the crate feature test-util.
