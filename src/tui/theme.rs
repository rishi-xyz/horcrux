//! One color palette, used everywhere, so a screenshot is self-explanatory:
//! green/yellow/red always mean the same thing (audit allow/warn/block; log
//! entry kind) on every screen.

use horcrux::audit::EntryKind;
use ratatui::style::Color;

pub const ALLOW: Color = Color::Green;
pub const WARN: Color = Color::Yellow;
pub const BLOCK: Color = Color::Red;
pub const ACCENT: Color = Color::Cyan;
pub const MUTED: Color = Color::DarkGray;

pub fn entry_kind_color(kind: EntryKind) -> Color {
    match kind {
        EntryKind::DecryptOk => ALLOW,
        EntryKind::DecryptFail => BLOCK,
        EntryKind::Blocked => BLOCK,
        EntryKind::Signed => ACCENT,
    }
}
