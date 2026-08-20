//! Colours, matching the zinc chrome the teley CLI uses so the two tools look
//! like they come from the same place.

use ratatui::style::Color;

pub const BG: Color = Color::Rgb(0x09, 0x09, 0x0b);
pub const PANEL: Color = Color::Rgb(0x18, 0x18, 0x1b);
pub const BORDER: Color = Color::Rgb(0x27, 0x27, 0x2a);
pub const BORDER_ACTIVE: Color = Color::Rgb(0x52, 0x52, 0x5b);
pub const TEXT: Color = Color::Rgb(0xd4, 0xd4, 0xd8);
pub const TEXT_STRONG: Color = Color::Rgb(0xf4, 0xf4, 0xf5);
pub const DIM: Color = Color::Rgb(0x71, 0x71, 0x7a);
pub const ACCENT: Color = Color::Rgb(0x3b, 0x82, 0xf6);

pub const OK: Color = Color::Rgb(0x22, 0xc5, 0x5e);
pub const WARN: Color = Color::Rgb(0xf5, 0x9e, 0x0b);
pub const ERROR: Color = Color::Rgb(0xef, 0x44, 0x44);
