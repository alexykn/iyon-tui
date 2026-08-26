//! Generic temporal publication of stable projection spans.

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::stream::{StreamOffset, StreamRange};

use super::{Projection, ProjectionBuilder, Projector};

const DEFAULT_TICK_INTERVAL: Duration = Duration::from_millis(16);
const DEFAULT_SPRING: f32 = 2.0;
const DEFAULT_MIN_UNITS_PER_SECOND: f32 = 20.0;
const DEFAULT_MAX_UNITS_PER_SECOND: f32 = 800.0;

/// Configuration for [`Smooth`]. Rates are measured in projected values per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmoothConfig {
    tick_interval: Duration,
    spring: f32,
    min_units_per_second: f32,
    max_units_per_second: f32,
}

impl Default for SmoothConfig {
    fn default() -> Self {
        Self {
            tick_interval: DEFAULT_TICK_INTERVAL,
            spring: DEFAULT_SPRING,
            min_units_per_second: DEFAULT_MIN_UNITS_PER_SECOND,
            max_units_per_second: DEFAULT_MAX_UNITS_PER_SECOND,
        }
    }
}

/// Invalid temporal smoothing configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SmoothConfigError {
    ZeroTickInterval,
    NonFiniteSpring,
    NegativeSpring,
    NonFiniteRate,
    NegativeRate,
    MinimumExceedsMaximum,
    NoProgressRate,
}

impl std::fmt::Display for SmoothConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Smooth configuration: {self:?}")
    }
}

impl std::error::Error for SmoothConfigError {}

impl SmoothConfig {
    pub const fn new() -> Self {
        Self {
            tick_interval: DEFAULT_TICK_INTERVAL,
            spring: DEFAULT_SPRING,
            min_units_per_second: DEFAULT_MIN_UNITS_PER_SECOND,
            max_units_per_second: DEFAULT_MAX_UNITS_PER_SECOND,
        }
    }

    pub fn try_from_parts(
        tick_interval: Duration,
        spring: f32,
        min_units_per_second: f32,
        max_units_per_second: f32,
    ) -> Result<Self, SmoothConfigError> {
        let config = Self {
            tick_interval,
            spring,
            min_units_per_second,
            max_units_per_second,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn tick_interval(self) -> Duration {
        self.tick_interval
    }
    pub fn spring(self) -> f32 {
        self.spring
    }
    pub fn min_units_per_second(self) -> f32 {
        self.min_units_per_second
    }
    pub fn max_units_per_second(self) -> f32 {
        self.max_units_per_second
    }

    pub fn with_tick_interval(mut self, value: Duration) -> Result<Self, SmoothConfigError> {
        self.tick_interval = value;
        self.validate()?;
        Ok(self)
    }

    pub fn with_spring(mut self, value: f32) -> Result<Self, SmoothConfigError> {
        self.spring = value;
        self.validate()?;
        Ok(self)
    }

    pub fn with_unit_rates(
        mut self,
        minimum: f32,
        maximum: f32,
    ) -> Result<Self, SmoothConfigError> {
        self.min_units_per_second = minimum;
        self.max_units_per_second = maximum;
        self.validate()?;
        Ok(self)
    }

    fn validate(self) -> Result<(), SmoothConfigError> {
        if self.tick_interval.is_zero() {
            return Err(SmoothConfigError::ZeroTickInterval);
        }
        if !self.spring.is_finite() {
            return Err(SmoothConfigError::NonFiniteSpring);
        }
        if self.spring < 0.0 {
            return Err(SmoothConfigError::NegativeSpring);
        }
        if !self.min_units_per_second.is_finite() || !self.max_units_per_second.is_finite() {
            return Err(SmoothConfigError::NonFiniteRate);
        }
        if self.min_units_per_second < 0.0 || self.max_units_per_second < 0.0 {
            return Err(SmoothConfigError::NegativeRate);
        }
        if self.min_units_per_second > self.max_units_per_second {
            return Err(SmoothConfigError::MinimumExceedsMaximum);
        }
        if self.max_units_per_second <= 0.0
            || (self.spring <= 0.0 && self.min_units_per_second <= 0.0)
        {
            return Err(SmoothConfigError::NoProgressRate);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingSpan {
    source: StreamRange,
    weight: usize,
}

/// Delays publication of complete upstream-stable spans without transforming values.
///
/// `Smooth` never splits a span. Callers choose pacing granularity by choosing the
/// upstream projection spans; pacing is based on `values.len()`, never coordinates or
/// display width. Only source through the input stable frontier can be published.
#[derive(Debug)]
pub struct Smooth {
    config: SmoothConfig,
    published_end: StreamOffset,
    input_sealed: bool,
    input_base: Option<StreamOffset>,
    input_stable: StreamOffset,
    input_end: StreamOffset,
    queued_through: StreamOffset,
    pending: VecDeque<PendingSpan>,
    pending_units: usize,
    credit_units: f64,
    last_advance: Option<Instant>,
    next_wakeup: Option<Instant>,
    episode_active: bool,
}

impl Default for Smooth {
    fn default() -> Self {
        Self::new(SmoothConfig::default())
    }
}

impl Smooth {
    pub fn new(config: SmoothConfig) -> Self {
        Self {
            config,
            published_end: StreamOffset::ZERO,
            input_sealed: false,
            input_base: None,
            input_stable: StreamOffset::ZERO,
            input_end: StreamOffset::ZERO,
            queued_through: StreamOffset::ZERO,
            pending: VecDeque::new(),
            pending_units: 0,
            credit_units: 0.0,
            last_advance: None,
            next_wakeup: None,
            episode_active: false,
        }
    }

    pub fn config(&self) -> SmoothConfig {
        self.config
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        self.next_wakeup
    }

    pub fn published_through(&self) -> StreamOffset {
        self.published_end
    }

    pub fn advance(&mut self, now: Instant) -> bool {
        if self.input_sealed || self.pending.is_empty() {
            self.next_wakeup = None;
            self.last_advance = None;
            self.credit_units = 0.0;
            self.episode_active = false;
            return false;
        }
        let Some(deadline) = self.next_wakeup else {
            self.last_advance = Some(now);
            self.next_wakeup = now.checked_add(self.config.tick_interval);
            return false;
        };
        if now < deadline {
            return false;
        }

        let previous = self.published_end;
        let elapsed = self
            .last_advance
            .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
        self.last_advance = Some(now);
        let rate = (self.pending_units as f32 * self.config.spring).clamp(
            self.config.min_units_per_second,
            self.config.max_units_per_second,
        );
        self.credit_units += f64::from(elapsed.as_secs_f32() * rate);
        self.release_budget();
        if self.pending.is_empty() {
            self.next_wakeup = None;
            self.last_advance = None;
            self.credit_units = 0.0;
            self.episode_active = false;
        } else {
            self.next_wakeup = now.checked_add(self.config.tick_interval);
        }
        self.published_end > previous
    }

    fn release_budget(&mut self) {
        while let Some(span) = self.pending.front().copied() {
            if span.weight != 0 && self.credit_units < span.weight as f64 {
                break;
            }
            if span.weight != 0 {
                self.credit_units -= span.weight as f64;
            }
            self.published_end = span.source.end();
            self.pending_units = self.pending_units.saturating_sub(span.weight);
            self.pending.pop_front();
        }
    }

    fn release_immediate(&mut self) {
        let mut weighted = false;
        while let Some(span) = self.pending.front().copied() {
            if span.weight != 0 {
                if weighted {
                    break;
                }
                weighted = true;
            }
            self.published_end = span.source.end();
            self.pending_units = self.pending_units.saturating_sub(span.weight);
            self.pending.pop_front();
        }
    }

    fn rebuild_pending<T>(&mut self, input: &Projection<T>) {
        let metadata_unchanged = self.input_base == Some(input.source_base())
            && self.input_stable == input.stable_through()
            && self.input_end == input.source_end();
        if metadata_unchanged && self.input_sealed == input.is_sealed() {
            return;
        }
        let reset_queue = self.input_base != Some(input.source_base())
            || input.source_base() > self.queued_through;
        if reset_queue {
            self.pending.clear();
            self.pending_units = 0;
            self.queued_through = input.source_base().max(self.published_end);
        }
        let mut added_units: usize = 0;
        for span in input.spans_from(self.queued_through) {
            if span.source().start() < self.queued_through
                || span.source().end() <= self.published_end
                || span.source().end() > input.stable_through()
            {
                continue;
            }
            let pending = PendingSpan {
                source: span.source(),
                weight: span.values().len(),
            };
            self.queued_through = pending.source.end();
            added_units = added_units.saturating_add(pending.weight);
            self.pending.push_back(pending);
        }
        if reset_queue {
            self.pending_units = added_units;
        } else {
            self.pending_units = self.pending_units.saturating_add(added_units);
        }
        self.input_base = Some(input.source_base());
        self.input_stable = input.stable_through();
        self.input_end = input.source_end();
    }

    fn output<T: Clone>(&self, input: &Projection<T>) -> Projection<T> {
        self.output_from(input, input.source_base())
    }

    fn output_from<T: Clone>(&self, input: &Projection<T>, from: StreamOffset) -> Projection<T> {
        let end = self.published_end.min(input.source_end());
        if from < input.source_base() || from > end {
            return self.output(input);
        }
        let spans = input.spans_from(from);
        if spans
            .first()
            .is_some_and(|span| span.source().start() < from)
        {
            return self.output(input);
        }
        let mut builder = ProjectionBuilder::new(
            from,
            end,
            end,
            input.is_sealed() && end == input.source_end(),
        );
        for span in spans.iter().take_while(|span| span.source.end() <= end) {
            builder = builder.emit_many(span.source, span.values.iter().cloned());
        }
        builder
            .finish()
            .expect("Smooth output must preserve input coverage")
    }

    fn update<T: Clone>(&mut self, input: &Projection<T>) {
        let was_caught_up = !self.episode_active;
        self.input_sealed = input.is_sealed();
        if self.published_end < input.source_base() {
            self.published_end = input.source_base();
        }
        if self.published_end > input.source_end() {
            self.published_end = input.source_end();
            self.credit_units = 0.0;
            self.next_wakeup = None;
            self.last_advance = None;
            self.episode_active = false;
        }
        self.rebuild_pending(input);

        if input.is_sealed() {
            self.published_end = input.source_end();
            self.pending.clear();
            self.pending_units = 0;
            self.credit_units = 0.0;
            self.last_advance = None;
            self.next_wakeup = None;
            self.episode_active = false;
        } else if was_caught_up && !self.pending.is_empty() {
            self.release_immediate();
            self.episode_active = !self.pending.is_empty();
        } else if self.pending.is_empty() {
            self.next_wakeup = None;
            self.last_advance = None;
            self.credit_units = 0.0;
            self.episode_active = false;
        } else {
            self.episode_active = true;
        }
    }

    /// Advances smoothing state and returns only newly published source spans.
    /// The caller owns the retained published prefix and can append this delta
    /// without reconstructing the complete historic projection.
    pub(crate) fn project_incremental<T: Clone>(
        &mut self,
        input: &Projection<T>,
        from: StreamOffset,
    ) -> Projection<T> {
        self.update(input);
        self.output_from(input, from)
    }
}

impl<T: Clone> Projector<T> for Smooth {
    type Output = T;
    type Error = std::convert::Infallible;

    fn project(&mut self, input: &Projection<T>) -> Result<Projection<T>, Self::Error> {
        self.update(input);
        Ok(self.output(input))
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.next_wakeup
    }

    fn advance(&mut self, now: Instant) -> bool {
        Smooth::advance(self, now)
    }
}
