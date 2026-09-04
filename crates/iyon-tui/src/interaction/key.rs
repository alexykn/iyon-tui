use std::ops::{BitOr, BitOrAssign};

/// Backend-neutral key identity.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Tab,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Function(u8),
    Null,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    Menu,
    KeypadBegin,
    Media(MediaKey),
    Modifier(ModifierKey),
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKey {
    Play,
    Pause,
    PlayPause,
    Reverse,
    Stop,
    FastForward,
    Rewind,
    TrackNext,
    TrackPrevious,
    Record,
    LowerVolume,
    RaiseVolume,
    MuteVolume,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierKey {
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftSuper,
    LeftHyper,
    LeftMeta,
    RightShift,
    RightControl,
    RightAlt,
    RightSuper,
    RightHyper,
    RightMeta,
    IsoLevel3Shift,
    IsoLevel5Shift,
}

/// Framework-owned keyboard modifier bitset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    bits: u8,
}

impl Modifiers {
    pub const NONE: Self = Self { bits: 0 };
    pub const SHIFT: Self = Self { bits: 1 << 0 };
    pub const CONTROL: Self = Self { bits: 1 << 1 };
    pub const ALT: Self = Self { bits: 1 << 2 };
    pub const SUPER: Self = Self { bits: 1 << 3 };
    pub const HYPER: Self = Self { bits: 1 << 4 };
    pub const META: Self = Self { bits: 1 << 5 };

    #[must_use]
    pub const fn contains(self, modifiers: Self) -> bool {
        self.bits & modifiers.bits == modifiers.bits
    }

    #[must_use]
    pub const fn union(self, modifiers: Self) -> Self {
        Self {
            bits: self.bits | modifiers.bits,
        }
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

/// A normalized keyboard actuation used by framework command routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyStroke {
    key: Key,
    modifiers: Modifiers,
}

impl KeyStroke {
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn with_modifiers(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    #[must_use]
    pub const fn key(self) -> Key {
        self.key
    }

    #[must_use]
    pub const fn modifiers(self) -> Modifiers {
        self.modifiers
    }
}
