use iced::Color;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum AppTheme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub app: Color,
    pub surface: Color,
    pub control_surface: Color,
    pub border: Color,
    pub hover: Color,
    pub placeholder: Color,

    pub text_primary: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub text_locked_desc: Color,

    pub accent: Color,
    pub accent_pending: Color,

    pub rarity_common: Color,
    pub rarity_uncommon: Color,
    pub rarity_rare: Color,
    pub rarity_mythical: Color,
    pub rarity_legendary: Color,

    pub tier_locked: Color,

    pub dot_connected: Color,
    pub dot_offline: Color,

    pub severity: SeverityPalette,
}

#[derive(Debug, Clone, Copy)]
pub struct SeverityPalette {
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

pub const DARK: ThemePalette = ThemePalette {
    app: Color::from_rgb8(0x1a, 0x18, 0x25),
    surface: Color::from_rgb8(0x22, 0x1f, 0x30),
    control_surface: Color::from_rgb8(0x2e, 0x29, 0x42),
    border: Color::from_rgb8(0x2d, 0x29, 0x40),
    hover: Color::from_rgb8(0x3a, 0x35, 0x52),
    placeholder: Color::from_rgb(0.188, 0.192, 0.247),

    text_primary: Color::from_rgb8(0xf0, 0xee, 0xf8),
    text_muted: Color::from_rgb8(0x8a, 0x86, 0xa3),
    text_dim: Color::from_rgb8(0x6b, 0x68, 0x84),
    text_locked_desc: Color::from_rgb8(0x99, 0x94, 0xb0),

    accent: Color::from_rgb8(0xc9, 0xa6, 0xf0),
    accent_pending: Color::from_rgb(0.945, 0.980, 0.549),

    rarity_common: Color::from_rgb(0.314, 0.980, 0.482),
    rarity_uncommon: Color::from_rgb(0.545, 0.914, 0.992),
    rarity_rare: Color::from_rgb(0.741, 0.576, 0.976),
    rarity_mythical: Color::from_rgb(1.0, 0.4, 0.85),
    rarity_legendary: Color::from_rgb(1.0, 0.85, 0.4),

    dot_connected: Color::from_rgb(0.314, 0.980, 0.482),
    dot_offline: Color::from_rgb(0.878, 0.722, 0.376),

    severity: SeverityPalette {
        success: Color::from_rgb(0.314, 0.980, 0.482),
        warning: Color::from_rgb(0.878, 0.722, 0.376),
        error: Color::from_rgb(0.863, 0.392, 0.392),
    },

    tier_locked: Color::from_rgb8(0x54, 0x4f, 0x6e),
};

pub const LIGHT: ThemePalette = ThemePalette {
    app: Color::from_rgb8(0xe6, 0xe9, 0xef),
    surface: Color::from_rgb8(0xef, 0xf1, 0xf5),
    control_surface: Color::from_rgb8(0xe0, 0xe4, 0xec),
    border: Color::from_rgb8(0xcc, 0xd0, 0xda),
    hover: Color::from_rgb8(0xc8, 0xcd, 0xd6),
    placeholder: Color::from_rgb8(0xdc, 0xe0, 0xe8),

    text_primary: Color::from_rgb8(0x1f, 0x1c, 0x2c),
    text_muted: Color::from_rgb8(0x6b, 0x68, 0x84),
    text_dim: Color::from_rgb8(0x99, 0x94, 0xa8),
    text_locked_desc: Color::from_rgb8(0x55, 0x52, 0x6e),

    accent: Color::from_rgb8(0x88, 0x39, 0xef),
    accent_pending: Color::from_rgb8(0x9a, 0x6d, 0x00),

    rarity_common: Color::from_rgb8(0x4c, 0xaf, 0x50),
    rarity_uncommon: Color::from_rgb8(0x02, 0x84, 0xc7),
    rarity_rare: Color::from_rgb8(0x72, 0x87, 0xfd),
    rarity_mythical: Color::from_rgb8(0xc9, 0x3b, 0x87),
    rarity_legendary: Color::from_rgb8(0xc8, 0xa1, 0x16),

    dot_connected: Color::from_rgb8(0x04, 0x78, 0x57),
    dot_offline: Color::from_rgb8(0xb4, 0x53, 0x09),

    severity: SeverityPalette {
        success: Color::from_rgb8(0x04, 0x78, 0x57),
        warning: Color::from_rgb8(0xb4, 0x53, 0x09),
        error: Color::from_rgb8(0xb9, 0x1c, 0x1c),
    },

    tier_locked: Color::from_rgb8(0xb8, 0xb3, 0xc4),
};

pub const THEME_NAME_DARK: &str = "SteamLens Dark";
pub const THEME_NAME_LIGHT: &str = "SteamLens Light";

pub fn palette(theme: AppTheme) -> &'static ThemePalette {
    match theme {
        AppTheme::Dark => &DARK,
        AppTheme::Light => &LIGHT,
    }
}

pub fn theme_from_iced(iced_theme: &iced::Theme) -> AppTheme {
    use iced::theme::Base;
    match iced_theme.name() {
        name if name == THEME_NAME_LIGHT => AppTheme::Light,
        _ => AppTheme::Dark,
    }
}

fn iced_palette(theme: AppTheme) -> iced::theme::Palette {
    let p = palette(theme);
    iced::theme::Palette {
        background: p.app,
        text: p.text_primary,
        primary: p.accent,
        success: p.severity.success,
        warning: p.severity.warning,
        danger: p.severity.error,
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
        let dark = palette(AppTheme::Dark);
        let light = palette(AppTheme::Light);
        assert_ne!(dark.app, light.app);
        assert_ne!(dark.surface, light.surface);
        assert_ne!(dark.text_primary, light.text_primary);
        assert_ne!(dark.accent, light.accent);
    }

    #[test]
    fn iced_palette_derived_from_themepalette() {
        let p = palette(AppTheme::Dark);
        let ip = iced_palette(AppTheme::Dark);
        assert_eq!(ip.background, p.app);
        assert_eq!(ip.text, p.text_primary);
        assert_eq!(ip.primary, p.accent);
        assert_eq!(ip.success, p.severity.success);
        assert_eq!(ip.warning, p.severity.warning);
        assert_eq!(ip.danger, p.severity.error);
    }

    #[test]
    fn severity_slots_are_distinct_per_variant() {
        for theme in [AppTheme::Dark, AppTheme::Light] {
            let s = &palette(theme).severity;
            assert_ne!(s.success, s.error);
            assert_ne!(s.warning, s.error);
        }
    }

    #[test]
    fn into_iced_theme_works_for_each_variant() {
        let _: iced::Theme = AppTheme::Dark.into();
        let _: iced::Theme = AppTheme::Light.into();
    }
}
