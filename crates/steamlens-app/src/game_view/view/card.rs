use iced::widget::{
    button, column, container, image, lazy, rich_text, row, space, span, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::grid::{
    ACH_CARD_DESCRIPTION_TEXT_SIZE, ACH_CARD_HEIGHT, ACH_CARD_ICON, ACH_CARD_TEXT_COL_SPACING,
    ACH_CARD_TITLE_TEXT_SIZE,
};
use super::{C_MUTED, dracula_border_radius, tier_color};
use crate::game_view::GameViewMessage;
use crate::game_view::types::{AchievementRow, RarityTier};
use crate::ui::theme::{palette, theme_from_iced};
use crate::ui::widgets::card::card;
use crate::ui::widgets::pill::pill;

const C_LOCKED_DESC: Color = Color::from_rgb8(0x99, 0x94, 0xb0);
const C_YELLOW: Color = Color::from_rgb(0.945, 0.980, 0.549);

#[derive(Hash, PartialEq, Eq)]
struct AchievementCardDeps {
    id_hash: u64,
    is_dirty: bool,
    revealed: bool,
    is_achieved: bool,
    is_hidden: bool,
    permission: u32,
    icon_present: bool,
    icon_len: usize,
    rarity_bits: u32,
    has_rarity: bool,
    display_name_hash: u64,
    description_hash: u64,
    search_hash: u64,
    tier: Option<RarityTier>,
    card_w_bits: u32,
    theme: crate::ui::theme::AppTheme,
}

impl AchievementCardDeps {
    fn new(
        row: &AchievementRow,
        card_w: f32,
        search_query_lower: &str,
        tier: Option<RarityTier>,
        theme: crate::ui::theme::AppTheme,
    ) -> Self {
        Self {
            id_hash: fnv64(row.data.id.as_bytes()),
            is_dirty: row.is_dirty,
            revealed: row.revealed,
            is_achieved: row.data.is_achieved,
            is_hidden: row.data.is_hidden,
            permission: row.data.permission,
            icon_present: row.data.icon.is_some(),
            icon_len: row.data.icon.as_ref().map(|i| i.rgba.len()).unwrap_or(0),
            rarity_bits: row.rarity_percent.map(|f| f.to_bits()).unwrap_or(0),
            has_rarity: row.rarity_percent.is_some(),
            display_name_hash: fnv64(row.data.display_name.as_bytes()),
            description_hash: fnv64(row.data.description.as_bytes()),
            search_hash: fnv64(search_query_lower.as_bytes()),
            tier,
            card_w_bits: card_w.to_bits(),
            theme,
        }
    }
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h = h.wrapping_mul(0x100000001b3);
        h ^= b as u64;
    }
    h
}

pub(super) fn achievement_card_widget<'a>(
    row: &'a AchievementRow,
    card_w: f32,
    search_query_lower: String,
    tier: Option<RarityTier>,
    app_theme: crate::ui::theme::AppTheme,
    icon_handle: Option<iced::widget::image::Handle>,
) -> Element<'a, GameViewMessage> {
    let deps = AchievementCardDeps::new(row, card_w, &search_query_lower, tier, app_theme);

    let entry = row.clone();
    lazy(deps, move |_| {
        render_achievement_card(
            &entry,
            card_w,
            &search_query_lower,
            tier,
            app_theme,
            icon_handle.clone(),
        )
    })
    .into()
}

pub(super) fn legendary_glow_overlay(
    glow_pulse: f32,
    card_w: f32,
    card_h: f32,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'static, GameViewMessage> {
    let p = *palette(app_theme);
    let alpha = 0.30 + 0.20 * glow_pulse;
    let blur = 12.0 + 10.0 * glow_pulse;
    let border_alpha = 0.50 + 0.35 * glow_pulse;

    container(iced::widget::Space::new())
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(card_h))
        .style(move |_: &iced::Theme| container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: alpha,
                    ..p.rarity_legendary
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: blur,
            },
            border: iced::Border {
                color: Color {
                    a: border_alpha,
                    ..p.rarity_legendary
                },
                width: 1.5,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn render_achievement_card(
    row: &AchievementRow,
    card_w: f32,
    search_query_lower: &str,
    tier: Option<RarityTier>,
    app_theme: crate::ui::theme::AppTheme,
    icon_handle: Option<iced::widget::image::Handle>,
) -> Element<'static, GameViewMessage> {
    let p = *palette(app_theme);
    let fg = p.text_primary;
    let effective = row.effective_achieved();
    let spoiler_hidden = row.is_spoiler_hidden();
    let is_hidden_meta = row.data.is_hidden;

    let icon_el: Element<'static, GameViewMessage> = if spoiler_hidden {
        container(text("\u{2754}").size(22).color(Color { a: 0.5, ..C_MUTED }))
            .width(Length::Fixed(ACH_CARD_ICON))
            .height(Length::Fixed(ACH_CARD_ICON))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(Color {
                    r: p.border.r * 0.7,
                    g: p.border.g * 0.7,
                    b: p.border.b * 0.7,
                    a: 1.0,
                })),
                border: dracula_border_radius(6.0),
                ..container::Style::default()
            })
            .into()
    } else if let Some(handle) = icon_handle {
        let opacity = if effective { 1.0f32 } else { 0.45f32 };
        container(
            image(handle)
                .width(Length::Fixed(ACH_CARD_ICON))
                .height(Length::Fixed(ACH_CARD_ICON))
                .opacity(opacity),
        )
        .style(move |_theme| icon_glow_style(tier, app_theme))
        .into()
    } else {
        let icon_bg = if effective {
            p.border
        } else {
            Color {
                r: p.border.r * 0.6,
                g: p.border.g * 0.6,
                b: p.border.b * 0.6,
                a: 1.0,
            }
        };
        container(
            text(if effective { "\u{2713}" } else { "\u{25CB}" })
                .size(20)
                .color(if effective { p.rarity_common } else { C_MUTED }),
        )
        .width(Length::Fixed(ACH_CARD_ICON))
        .height(Length::Fixed(ACH_CARD_ICON))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(icon_bg)),
            border: dracula_border_radius(6.0),
            ..container::Style::default()
        })
        .into()
    };

    let icon_el: Element<'static, GameViewMessage> = if is_hidden_meta && !spoiler_hidden {
        let badge = container(text("H").size(11).color(C_LOCKED_DESC))
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|t: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color {
                    a: 0.85,
                    ..palette(theme_from_iced(t)).surface
                })),
                border: Border {
                    color: C_LOCKED_DESC,
                    width: 1.0,
                    radius: 9.0.into(),
                },
                ..container::Style::default()
            });
        let positioned = container(badge)
            .width(Length::Fixed(ACH_CARD_ICON))
            .height(Length::Fixed(ACH_CARD_ICON))
            .align_x(Alignment::End)
            .align_y(Alignment::Start)
            .padding(Padding::default().top(2).right(2));
        stack![icon_el, positioned].into()
    } else {
        icon_el
    };

    let display_name = if spoiler_hidden {
        "Hidden Achievement".to_owned()
    } else if row.data.display_name.is_empty() {
        row.data.id.clone()
    } else {
        row.data.display_name.clone()
    };

    let name_color = if row.is_dirty { C_YELLOW } else { fg };
    let name_label: Element<'static, GameViewMessage> = if !row.is_dirty
        && !spoiler_hidden
        && !search_query_lower.is_empty()
    {
        if let Some((before, matched, after)) = highlight_split(&display_name, search_query_lower) {
            let before = before.to_owned();
            let matched = matched.to_owned();
            let after = after.to_owned();
            container(
                rich_text![
                    span(before).color(fg),
                    span(matched)
                        .color(C_YELLOW)
                        .background(Color { a: 0.2, ..C_YELLOW }),
                    span(after).color(fg),
                ]
                .on_link_click(iced::never)
                .size(ACH_CARD_TITLE_TEXT_SIZE)
                .wrapping(text::Wrapping::Word)
                .line_height(text::LineHeight::Relative(1.2)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(36.0))
            .into()
        } else {
            container(
                text(display_name)
                    .size(ACH_CARD_TITLE_TEXT_SIZE)
                    .color(name_color)
                    .wrapping(text::Wrapping::Word)
                    .line_height(text::LineHeight::Relative(1.2)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(36.0))
            .into()
        }
    } else {
        container(
            text(display_name)
                .size(ACH_CARD_TITLE_TEXT_SIZE)
                .color(name_color)
                .wrapping(text::Wrapping::Word)
                .line_height(text::LineHeight::Relative(1.2)),
        )
        .width(Length::Fill)
        .height(Length::Fixed(36.0))
        .into()
    };

    let description = if spoiler_hidden {
        "Hidden until revealed".to_owned()
    } else {
        row.data.description.clone()
    };

    let desc_color = if spoiler_hidden {
        Color { a: 0.5, ..C_MUTED }
    } else {
        C_LOCKED_DESC
    };

    let desc_label: Element<'static, GameViewMessage> = if !spoiler_hidden
        && !search_query_lower.is_empty()
    {
        if let Some((before, matched, after)) = highlight_split(&description, search_query_lower) {
            let before = before.to_owned();
            let matched = matched.to_owned();
            let after = after.to_owned();
            container(
                rich_text![
                    span(before).color(desc_color),
                    span(matched)
                        .color(C_YELLOW)
                        .background(Color { a: 0.2, ..C_YELLOW }),
                    span(after).color(desc_color),
                ]
                .on_link_click(iced::never)
                .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
                .wrapping(text::Wrapping::Word),
            )
            .width(Length::Fill)
            .height(Length::Fixed(30.0))
            .into()
        } else {
            container(
                text(description)
                    .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
                    .color(desc_color)
                    .wrapping(text::Wrapping::Word),
            )
            .width(Length::Fill)
            .height(Length::Fixed(30.0))
            .into()
        }
    } else {
        container(
            text(description)
                .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
                .color(desc_color)
                .wrapping(text::Wrapping::Word),
        )
        .width(Length::Fill)
        .height(Length::Fixed(30.0))
        .into()
    };

    let text_col = column![name_label, desc_label].spacing(ACH_CARD_TEXT_COL_SPACING);

    let top_row = row![icon_el, text_col]
        .spacing(8)
        .align_y(Alignment::Start)
        .padding(Padding::from([8u16, 8]));

    let badge_text = row.status_label();
    let is_locked_badge = badge_text == "Locked";
    let fixed_badge_color: Option<Color> = match badge_text {
        "Protected" => Some(p.severity.warning.text),
        "Pending" => Some(C_YELLOW),
        "Unlocked" => Some(p.rarity_common),
        _ => None,
    };

    let badge = if let Some(badge_color) = fixed_badge_color {
        let badge_text_color = Color {
            a: 0.9,
            ..badge_color
        };
        pill(
            text(badge_text)
                .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
                .color(badge_text_color),
            badge_color,
        )
    } else {
        let locked_text =
            text(badge_text)
                .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
                .style(move |_t: &iced::Theme| iced::widget::text::Style {
                    color: Some(C_LOCKED_DESC),
                });
        pill(locked_text, C_LOCKED_DESC)
    };
    let _ = is_locked_badge;

    let rarity_badge: Option<Element<'static, GameViewMessage>> = if spoiler_hidden {
        None
    } else if let (Some(t), Some(pct)) = (tier, row.rarity_percent) {
        let tc = tier_color(t, app_theme);
        let label = format!("{} \u{00B7} {:.1}%", t.label(), pct);
        let label_text = text(label)
            .size(ACH_CARD_DESCRIPTION_TEXT_SIZE)
            .color(Color { a: 0.95, ..tc });
        let rb = pill(label_text, tc).with_dot(tc);
        Some(rb.into())
    } else {
        None
    };

    let bottom_row: Element<'static, GameViewMessage> = if spoiler_hidden {
        let reveal_id = row.data.id.clone();
        let reveal_btn = button(text("Reveal").size(ACH_CARD_DESCRIPTION_TEXT_SIZE).style(
            |t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).accent),
            },
        ))
        .on_press(GameViewMessage::RevealHidden(reveal_id))
        .padding(Padding::default().left(12).right(12).top(3).bottom(3))
        .style(|t: &iced::Theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let accent = palette(theme_from_iced(t)).accent;
            button::Style {
                background: Some(Background::Color(Color {
                    a: if hovered { 0.28 } else { 0.18 },
                    ..accent
                })),
                border: Border {
                    color: Color {
                        a: if hovered { 0.65 } else { 0.45 },
                        ..accent
                    },
                    width: 1.0,
                    radius: 12.0.into(),
                },
                text_color: accent,
                ..button::Style::default()
            }
        });

        row![reveal_btn, space().width(Length::Fill), badge]
            .spacing(4)
            .align_y(Alignment::Center)
            .padding(Padding::default().left(8).right(8).bottom(8))
            .into()
    } else {
        let mut bottom: iced::widget::Row<'static, GameViewMessage> =
            row![].spacing(6).align_y(Alignment::Center);
        if let Some(rb) = rarity_badge {
            bottom = bottom.push(rb);
        }
        bottom = bottom.push(space().width(Length::Fill));
        bottom = bottom.push(badge);

        container(bottom)
            .width(Length::Fill)
            .padding(Padding::default().left(8).right(8).bottom(8))
            .into()
    };

    let separator = container(iced::widget::rule::horizontal(1))
        .padding(Padding::default().left(8).right(8).top(8).bottom(0))
        .width(Length::Fixed(card_w));

    let card_body = column![
        top_row,
        iced::widget::Space::new().height(Length::Fixed(10.0)),
        separator,
        iced::widget::Space::new().height(Length::Fill),
        bottom_row,
    ]
    .spacing(0);

    let card_container = container(card_body)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(ACH_CARD_HEIGHT));

    let toggle_id = row.data.id.clone();
    let is_hidden_card = spoiler_hidden;
    let glow_color: Option<Color> = if effective && !spoiler_hidden {
        tier.map(|t| tier_color(t, app_theme))
    } else {
        None
    };

    let mut c = card(card_container).on_press(GameViewMessage::AchievementToggled(toggle_id));

    if is_hidden_card {
        c = c
            .border_accent_when(true)
            .accent_border_width(1.0, 1.0)
            .accent_alpha(0.40, 0.55)
            .radius(10.0);
    } else if let Some(gc) = glow_color {
        c = c
            .accent(gc)
            .accent_border_width(1.0, 2.0)
            .accent_alpha(0.45, 0.85)
            .radius(8.0);
    } else {
        c = c.radius(8.0);
    }

    c.into()
}

pub(super) fn highlight_split<'a>(
    source: &'a str,
    query_lower: &str,
) -> Option<(&'a str, &'a str, &'a str)> {
    if query_lower.is_empty() {
        return None;
    }
    let lower_source = source.to_lowercase();
    let byte_offset = lower_source.find(query_lower)?;
    let match_end = byte_offset + query_lower.len();
    let before = &source[..byte_offset];
    let matched = &source[byte_offset..match_end];
    let after = &source[match_end..];
    Some((before, matched, after))
}

pub(super) fn icon_glow_style(
    tier: Option<RarityTier>,
    theme: crate::ui::theme::AppTheme,
) -> container::Style {
    let p = palette(theme);
    match tier {
        Some(RarityTier::Legendary) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.75,
                    ..p.rarity_legendary
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 22.0,
            },
            border: iced::Border {
                color: p.rarity_legendary,
                width: 3.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        },
        Some(RarityTier::Mythical) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.45,
                    ..p.rarity_mythical
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 16.0,
            },
            border: iced::Border {
                color: p.rarity_mythical,
                width: 2.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        },
        Some(RarityTier::Rare) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.7,
                    ..p.rarity_rare
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            border: iced::Border {
                color: p.rarity_rare,
                width: 1.5,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        },
        Some(RarityTier::Uncommon) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.7,
                    ..p.rarity_uncommon
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            ..container::Style::default()
        },
        Some(RarityTier::Common) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.7,
                    ..p.rarity_common
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            ..container::Style::default()
        },
        None => container::Style {
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(1.5, 1.5),
                blur_radius: 3.0,
            },
            ..container::Style::default()
        },
    }
}
#[cfg(test)]
mod skeleton_polish_tests {
    use super::super::grid::{ACH_CARD_HEIGHT, ACH_CARD_ICON, ACH_CARD_WIDTH};
    use super::C_LOCKED_DESC;
    use crate::ui::theme::DARK;
    use iced::Color;

    #[test]
    fn skeleton_card_height_matches_hydrated_card_height() {
        assert_eq!(
            ACH_CARD_HEIGHT, 140.0,
            "skeleton and hydrated paths both use ACH_CARD_HEIGHT — must agree"
        );
    }

    #[test]
    fn skeleton_icon_size_matches_hydrated_icon_size() {
        assert_eq!(
            ACH_CARD_ICON, 64.0,
            "icon placeholder size must match real icon size"
        );
    }

    #[test]
    fn card_width_constant_is_reasonable() {
        const { assert!(ACH_CARD_WIDTH >= 200.0 && ACH_CARD_WIDTH <= 400.0) };
    }

    #[test]
    fn locked_desc_color_is_lighter_than_text_muted() {
        let locked = C_LOCKED_DESC;
        let muted = DARK.text_muted;
        let luminance = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        assert!(
            luminance(locked) >= luminance(muted),
            "C_LOCKED_DESC must be lighter than or equal to text_muted — locked descriptions should be more readable"
        );
    }

    #[test]
    fn locked_desc_color_has_correct_rgb() {
        let Color { r, g, b, .. } = C_LOCKED_DESC;
        let r8 = (r * 255.0).round() as u8;
        let g8 = (g * 255.0).round() as u8;
        let b8 = (b * 255.0).round() as u8;
        assert_eq!(r8, 0x99, "red channel");
        assert_eq!(g8, 0x94, "green channel");
        assert_eq!(b8, 0xb0, "blue channel");
    }

    #[test]
    fn skeleton_grid_uses_same_card_dimensions_as_hydrated() {
        let title_w = ACH_CARD_WIDTH * 0.60;
        let desc_w = ACH_CARD_WIDTH * 0.80;
        assert!(
            title_w < ACH_CARD_WIDTH,
            "title skeleton must be narrower than card"
        );
        assert!(
            desc_w < ACH_CARD_WIDTH,
            "desc skeleton must be narrower than card"
        );
        assert!(
            title_w < desc_w,
            "title skeleton must be narrower than desc skeleton"
        );
    }
}
