#![allow(dead_code)]

use iced::{Color, Theme};

pub const C_APP: Color = Color::from_rgb8(0x1a, 0x18, 0x25);
pub const C_SURFACE: Color = Color::from_rgb8(0x22, 0x1f, 0x30);
pub const C_HOVER: Color = Color::from_rgb8(0x2a, 0x26, 0x38);
pub const C_BORDER: Color = Color::from_rgb8(0x2d, 0x29, 0x40);

pub const C_TEXT_PRIMARY: Color = Color::from_rgb8(0xf0, 0xee, 0xf8);
pub const C_TEXT_SECONDARY: Color = Color::from_rgb8(0xe4, 0xe2, 0xf0);
pub const C_TEXT_MUTED: Color = Color::from_rgb8(0x8a, 0x86, 0xa3);
pub const C_TEXT_DIM: Color = Color::from_rgb8(0x6b, 0x68, 0x84);

pub const C_ACCENT: Color = Color::from_rgb8(0xc9, 0xa6, 0xf0);
pub const C_ACCENT_DARK: Color = Color::from_rgb8(0x7b, 0x5d, 0xb5);

pub const C_DANGER: Color = Color::from_rgb8(0xdc, 0x64, 0x64);

pub fn theme() -> Theme {
    crate::ui::theme::AppTheme::Dark.into()
}
