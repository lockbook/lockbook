//! The characters a note's text can take, each filled by a family the
//! platform ships: the first of a slot's candidates installed, or none, which
//! hides the slot. The platform's own slot always shows: notes draw it from
//! the bundled fonts where the system lacks it.

use super::loader::{self, Kind, Load};
use super::system;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Sans,
    Serif,
    Humanist,
    Geometric,
    Mono,
}

impl Slot {
    pub const ALL: [Slot; 5] =
        [Slot::Sans, Slot::Serif, Slot::Humanist, Slot::Geometric, Slot::Mono];

    pub fn name(self) -> &'static str {
        match self {
            Slot::Sans => "Sans",
            Slot::Humanist => "Humanist",
            Slot::Serif => "Serif",
            Slot::Geometric => "Geometric",
            Slot::Mono => "Mono",
        }
    }

    /// The slot the platform's own text family fills, which notes start in.
    pub fn platform() -> Slot {
        if cfg!(target_vendor = "apple") { Slot::Sans } else { Slot::Humanist }
    }

    #[cfg(target_vendor = "apple")]
    fn candidates(self) -> &'static [&'static str] {
        match self {
            Slot::Sans => &[".SF NS", ".SF UI"],
            Slot::Humanist => &["Seravek"],
            Slot::Serif => &["Iowan Old Style"],
            Slot::Geometric => &["Avenir Next"],
            Slot::Mono => &[".SF NS Mono", ".SF UI Mono"],
        }
    }

    #[cfg(target_os = "windows")]
    fn candidates(self) -> &'static [&'static str] {
        match self {
            Slot::Sans => &["Arial"],
            Slot::Humanist => &["Segoe UI"],
            Slot::Serif => &["Palatino Linotype"],
            Slot::Geometric => &["Century Gothic"],
            Slot::Mono => &["Consolas"],
        }
    }

    #[cfg(not(any(target_vendor = "apple", target_os = "windows")))]
    fn candidates(self) -> &'static [&'static str] {
        match self {
            Slot::Sans => &["Nimbus Sans"],
            Slot::Humanist => &["Noto Sans"],
            Slot::Serif => &["P052"],
            Slot::Geometric => &["URW Gothic"],
            Slot::Mono => &["DejaVu Sans Mono"],
        }
    }

    /// The installed family filling this slot, the first of its candidates
    /// the system has, once read; none while they're being read, off the UI
    /// thread.
    pub fn family(self) -> Option<&'static str> {
        for name in self.candidates() {
            match loader::get(name, Kind::Text) {
                Load::Ready(family) => return Some(family.name),
                Load::Loading => return None,
                Load::Missing => {}
            }
        }
        None
    }

    /// The slots this platform fills.
    pub fn shown() -> impl Iterator<Item = Slot> {
        Slot::ALL
            .into_iter()
            .filter(|s| *s == Slot::platform() || s.family().is_some())
    }

    /// The family that draws this slot's name: its own, else, for the
    /// platform's slot where the system lacks it, the bundled text family.
    pub fn drawn_family(self) -> &'static str {
        let platform = (self == Slot::platform())
            .then(system::platform_sans)
            .flatten();
        self.family().or(platform).unwrap_or("Noto Sans")
    }
}
