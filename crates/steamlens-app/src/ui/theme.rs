use iced::Color;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
}

#[expect(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub app: Color,
    pub surface: Color,
    pub control_surface: Color,
    pub border: Color,
    pub hover: Color,
    pub placeholder: Color,

    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_dim: Color,

    pub accent: Color,
    pub accent_pressed: Color,
    pub accent_soft: Color,
    pub accent_soft_border: Color,

    pub rarity_common: Color,
    pub rarity_uncommon: Color,
    pub rarity_rare: Color,
    pub rarity_mythical: Color,
    pub rarity_legendary: Color,

    pub rarity_common_soft: Color,
    pub rarity_uncommon_soft: Color,
    pub rarity_rare_soft: Color,
    pub rarity_mythical_soft: Color,
    pub rarity_legendary_soft: Color,

    pub tier_locked: Color,

    pub dot_scanning: Color,
    pub dot_connected: Color,
    pub dot_offline: Color,

    pub severity: SeverityPalette,
}

#[derive(Debug, Clone, Copy)]
pub struct SeverityPalette {
    #[cfg_attr(not(test), expect(dead_code))]
    pub info: SeveritySlot,
    pub success: SeveritySlot,
    pub warning: SeveritySlot,
    pub error: SeveritySlot,
}

#[derive(Debug, Clone, Copy)]
pub struct SeveritySlot {
    pub text: Color,
    #[cfg_attr(not(test), expect(dead_code))]
    pub background: Color,
    #[cfg_attr(not(test), expect(dead_code))]
    pub border: Color,
}

pub const DARK: ThemePalette = ThemePalette {
    app: Color::from_rgb8(0x1a, 0x18, 0x25),
    surface: Color::from_rgb8(0x22, 0x1f, 0x30),
    control_surface: Color::from_rgb8(0x2e, 0x29, 0x42),
    border: Color::from_rgb8(0x2d, 0x29, 0x40),
    hover: Color::from_rgb8(0x3a, 0x35, 0x52),
    placeholder: Color::from_rgb(0.188, 0.192, 0.247),

    text_primary: Color::from_rgb8(0xf0, 0xee, 0xf8),
    text_secondary: Color::from_rgb8(0xe4, 0xe2, 0xf0),
    text_muted: Color::from_rgb8(0x8a, 0x86, 0xa3),
    text_dim: Color::from_rgb8(0x6b, 0x68, 0x84),

    accent: Color::from_rgb8(0xc9, 0xa6, 0xf0),
    accent_pressed: Color::from_rgb8(0x7b, 0x5d, 0xb5),
    accent_soft: Color {
        r: 0.788,
        g: 0.651,
        b: 0.941,
        a: 0.15,
    },
    accent_soft_border: Color {
        r: 0.788,
        g: 0.651,
        b: 0.941,
        a: 0.40,
    },

    rarity_common: Color::from_rgb(0.314, 0.980, 0.482),
    rarity_uncommon: Color::from_rgb(0.545, 0.914, 0.992),
    rarity_rare: Color::from_rgb(0.741, 0.576, 0.976),
    rarity_mythical: Color::from_rgb(1.0, 0.4, 0.85),
    rarity_legendary: Color::from_rgb(1.0, 0.85, 0.4),

    rarity_common_soft: Color {
        r: 0.314,
        g: 0.980,
        b: 0.482,
        a: 0.18,
    },
    rarity_uncommon_soft: Color {
        r: 0.545,
        g: 0.914,
        b: 0.992,
        a: 0.18,
    },
    rarity_rare_soft: Color {
        r: 0.741,
        g: 0.576,
        b: 0.976,
        a: 0.18,
    },
    rarity_mythical_soft: Color {
        r: 1.0,
        g: 0.4,
        b: 0.85,
        a: 0.18,
    },
    rarity_legendary_soft: Color {
        r: 1.0,
        g: 0.85,
        b: 0.4,
        a: 0.18,
    },

    dot_scanning: Color::from_rgb(0.741, 0.576, 0.976),
    dot_connected: Color::from_rgb(0.314, 0.980, 0.482),
    dot_offline: Color::from_rgb(0.878, 0.722, 0.376),

    severity: SeverityPalette {
        info: SeveritySlot {
            text: Color::from_rgb8(0xf0, 0xee, 0xf8),
            background: Color {
                r: 0.12,
                g: 0.25,
                b: 0.45,
                a: 0.55,
            },
            border: Color {
                r: 0.40,
                g: 0.60,
                b: 0.90,
                a: 0.45,
            },
        },
        success: SeveritySlot {
            text: Color::from_rgb(0.314, 0.980, 0.482),
            background: Color {
                r: 0.314,
                g: 0.980,
                b: 0.482,
                a: 0.12,
            },
            border: Color {
                r: 0.314,
                g: 0.980,
                b: 0.482,
                a: 0.40,
            },
        },
        warning: SeveritySlot {
            text: Color::from_rgb(0.878, 0.722, 0.376),
            background: Color {
                r: 0.40,
                g: 0.30,
                b: 0.10,
                a: 0.55,
            },
            border: Color {
                r: 0.85,
                g: 0.65,
                b: 0.25,
                a: 0.55,
            },
        },
        error: SeveritySlot {
            text: Color::from_rgb(0.863, 0.392, 0.392),
            background: Color {
                r: 0.45,
                g: 0.12,
                b: 0.12,
                a: 0.65,
            },
            border: Color {
                r: 0.85,
                g: 0.30,
                b: 0.30,
                a: 0.55,
            },
        },
    },

    tier_locked: Color::from_rgb8(0x3a, 0x36, 0x50),
};

pub const LIGHT: ThemePalette = ThemePalette {
    app: Color::from_rgb8(0xe6, 0xe9, 0xef),
    surface: Color::from_rgb8(0xef, 0xf1, 0xf5),
    control_surface: Color::from_rgb8(0xe0, 0xe4, 0xec),
    border: Color::from_rgb8(0xcc, 0xd0, 0xda),
    hover: Color::from_rgb8(0xc8, 0xcd, 0xd6),
    placeholder: Color::from_rgb8(0xdc, 0xe0, 0xe8),

    text_primary: Color::from_rgb8(0x1f, 0x1c, 0x2c),
    text_secondary: Color::from_rgb8(0x2a, 0x26, 0x38),
    text_muted: Color::from_rgb8(0x6b, 0x68, 0x84),
    text_dim: Color::from_rgb8(0x99, 0x94, 0xa8),

    accent: Color::from_rgb8(0x88, 0x39, 0xef),
    accent_pressed: Color::from_rgb8(0x6c, 0x1b, 0xd5),
    accent_soft: Color::from_rgb8(0xed, 0xe0, 0xfa),
    accent_soft_border: Color::from_rgb8(0xc8, 0xa8, 0xeb),

    rarity_common: Color::from_rgb8(0x4c, 0xaf, 0x50),
    rarity_uncommon: Color::from_rgb8(0x02, 0x84, 0xc7),
    rarity_rare: Color::from_rgb8(0x72, 0x87, 0xfd),
    rarity_mythical: Color::from_rgb8(0xc9, 0x3b, 0x87),
    rarity_legendary: Color::from_rgb8(0xc8, 0xa1, 0x16),

    rarity_common_soft: Color::from_rgb8(0xec, 0xf5, 0xf0),
    rarity_uncommon_soft: Color::from_rgb8(0xe8, 0xf0, 0xf6),
    rarity_rare_soft: Color::from_rgb8(0xf0, 0xea, 0xf8),
    rarity_mythical_soft: Color::from_rgb8(0xf8, 0xe9, 0xf0),
    rarity_legendary_soft: Color::from_rgb8(0xf8, 0xf0, 0xdd),

    dot_scanning: Color::from_rgb8(0x6d, 0x4c, 0xb5),
    dot_connected: Color::from_rgb8(0x04, 0x78, 0x57),
    dot_offline: Color::from_rgb8(0xb4, 0x53, 0x09),

    severity: SeverityPalette {
        info: SeveritySlot {
            text: Color::from_rgb8(0x1f, 0x1c, 0x2c),
            background: Color::from_rgb8(0xff, 0xff, 0xff),
            border: Color::from_rgb8(0xc8, 0xb2, 0xeb),
        },
        success: SeveritySlot {
            text: Color::from_rgb8(0x04, 0x78, 0x57),
            background: Color::from_rgb8(0xf0, 0xfd, 0xf4),
            border: Color::from_rgb8(0x04, 0x78, 0x57),
        },
        warning: SeveritySlot {
            text: Color::from_rgb8(0xb4, 0x53, 0x09),
            background: Color::from_rgb8(0xfe, 0xf3, 0xc7),
            border: Color::from_rgb8(0xb4, 0x53, 0x09),
        },
        error: SeveritySlot {
            text: Color::from_rgb8(0xb9, 0x1c, 0x1c),
            background: Color::from_rgb8(0xfe, 0xf2, 0xf2),
            border: Color::from_rgb8(0xb9, 0x1c, 0x1c),
        },
    },

    tier_locked: Color::from_rgb8(0xd0, 0xc9, 0xdd),
};

pub const THEME_NAME_DARK: &str = "SteamLens Dark";
pub const THEME_NAME_LIGHT: &str = "SteamLens Light";

pub fn palette(theme: AppTheme) -> &'static ThemePalette {
    match theme {
        AppTheme::Dark => &DARK,
        AppTheme::Light => &LIGHT,
    }
}

pub fn theme_from_iced(t: &iced::Theme) -> AppTheme {
    use iced::theme::Base;
    match t.name() {
        n if n == THEME_NAME_LIGHT => AppTheme::Light,
        _ => AppTheme::Dark,
    }
}

fn iced_palette(theme: AppTheme) -> iced::theme::Palette {
    let p = palette(theme);
    iced::theme::Palette {
        background: p.app,
        text: p.text_primary,
        primary: p.accent,
        success: p.severity.success.text,
        warning: p.severity.warning.text,
        danger: p.severity.error.text,
    }
}

impl From<AppTheme> for iced::Theme {
    fn from(theme: AppTheme) -> Self {
        let name = match theme {
            AppTheme::Dark => THEME_NAME_DARK,
            AppTheme::Light => THEME_NAME_LIGHT,
        };
        iced::Theme::custom(name.to_owned(), iced_palette(theme))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_dark() {
        assert_eq!(AppTheme::default(), AppTheme::Dark);
    }

    #[test]
    fn palette_dispatches_to_distinct_data() {
        let d = palette(AppTheme::Dark);
        let l = palette(AppTheme::Light);
        assert_ne!(d.app, l.app);
        assert_ne!(d.surface, l.surface);
        assert_ne!(d.text_primary, l.text_primary);
        assert_ne!(d.accent, l.accent);
    }

    #[test]
    fn iced_palette_derived_from_themepalette() {
        let p = palette(AppTheme::Dark);
        let ip = iced_palette(AppTheme::Dark);
        assert_eq!(ip.background, p.app);
        assert_eq!(ip.text, p.text_primary);
        assert_eq!(ip.primary, p.accent);
        assert_eq!(ip.success, p.severity.success.text);
        assert_eq!(ip.warning, p.severity.warning.text);
        assert_eq!(ip.danger, p.severity.error.text);
    }

    #[test]
    fn severity_slots_are_distinct_per_variant() {
        for theme in [AppTheme::Dark, AppTheme::Light] {
            let s = &palette(theme).severity;
            assert_ne!(s.success.text, s.error.text);
            assert_ne!(s.warning.text, s.info.text);
            assert_ne!(s.success.background, s.error.background);
            assert_ne!(s.success.border, s.error.border);
        }
    }

    #[test]
    fn rarity_full_and_soft_pair_per_tier() {
        for theme in [AppTheme::Dark, AppTheme::Light] {
            let p = palette(theme);
            let pairs = [
                (p.rarity_common, p.rarity_common_soft),
                (p.rarity_uncommon, p.rarity_uncommon_soft),
                (p.rarity_rare, p.rarity_rare_soft),
                (p.rarity_mythical, p.rarity_mythical_soft),
                (p.rarity_legendary, p.rarity_legendary_soft),
            ];
            for (full, soft) in pairs {
                assert_ne!(full, soft);
            }
        }
    }

    #[test]
    fn into_iced_theme_works_for_each_variant() {
        let _: iced::Theme = AppTheme::Dark.into();
        let _: iced::Theme = AppTheme::Light.into();
    }
}
