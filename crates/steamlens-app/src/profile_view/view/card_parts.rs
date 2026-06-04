use iced::widget::{button, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::dims::*;
use crate::game_view::types::RarityTier;
use crate::profile_view::types::{GameEntry, ProfileViewMessage};
use crate::ui::theme::{AppTheme, palette, theme_from_iced};
use crate::ui::widgets::bar::{BarColor, BarSegment, segmented_bar};
use crate::ui::widgets::pill::pill;
use crate::ui::widgets::tooltip_box::tooltip_box;

fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_owned()
    } else if max_chars <= 1 {
        "\u{2026}".to_owned()
    } else {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}\u{2026}", truncated.trim_end())
    }
}

pub(super) fn build_tier_stacked_bar(
    app_id: u32,
    tier_breakdown: &[(RarityTier, u32)],
    total_earned: u32,
    total_achievements: u32,
    card_w: f32,
    hovered_tier: Option<RarityTier>,
    theme: AppTheme,
) -> Element<'static, ProfileViewMessage> {
    const BAR_H: f32 = 8.0;
    const TIER_ORDER: [RarityTier; 5] = [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Mythical,
        RarityTier::Legendary,
    ];

    let inner_w = card_w - 16.0;
    let locked_count = total_achievements.saturating_sub(total_earned);
    let total = total_achievements.max(1);

    let mut segments: Vec<BarSegment> = Vec::new();
    let mut tier_at: Vec<Option<RarityTier>> = Vec::new();
    let mut tooltips: Vec<String> = Vec::new();

    for t in TIER_ORDER.iter() {
        let count = tier_breakdown
            .iter()
            .find(|(tt, _)| tt == t)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if count == 0 {
            continue;
        }
        let pct = count as f64 / total as f64 * 100.0;
        segments.push(BarSegment {
            weight: count,
            color: rarity_color_for_tier(*t, theme).into(),
        });
        tier_at.push(Some(*t));
        tooltips.push(format!(
            "{} {} \u{00B7} {:.1}%",
            count,
            rarity_label(*t),
            pct
        ));
    }

    if locked_count > 0 {
        let pct = locked_count as f64 / total as f64 * 100.0;
        segments.push(BarSegment {
            weight: locked_count,
            color: BarColor::Locked,
        });
        tier_at.push(None);
        tooltips.push(format!("{locked_count} Locked \u{00B7} {pct:.1}%"));
    }

    let hovered_idx = hovered_tier.and_then(|t| tier_at.iter().position(|x| *x == Some(t)));

    let tier_lookup = tier_at.clone();
    let tip_lookup = tooltips.clone();

    let bar: Element<'static, ProfileViewMessage> =
        segmented_bar(segments, Length::Fixed(inner_w), BAR_H)
            .hovered(hovered_idx)
            .on_hover(move |idx| {
                let tier = idx.and_then(|i| tier_lookup.get(i).copied().flatten());
                ProfileViewMessage::CardTierHovered { app_id, tier }
            })
            .tooltip(move |idx| tip_lookup.get(idx).cloned().unwrap_or_default())
            .into();

    container(bar)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(BAR_H))
        .padding(
            Padding::default()
                .left(CARD_HORIZONTAL_PADDING)
                .right(CARD_HORIZONTAL_PADDING),
        )
        .into()
}

pub(super) fn build_hover_overlay(
    app_id: u32,
    is_pinned: bool,
    card_w: f32,
    capsule_h: f32,
) -> Element<'static, ProfileViewMessage> {
    let pin_label = if is_pinned {
        "\u{2299} Unpin"
    } else {
        "\u{2299} Pin"
    };
    let pin_btn =
        button(
            text(pin_label)
                .size(11)
                .style(move |t: &iced::Theme| iced::widget::text::Style {
                    color: Some(if is_pinned {
                        palette(theme_from_iced(t)).accent
                    } else {
                        palette(theme_from_iced(t)).text_primary
                    }),
                }),
        )
        .on_press(ProfileViewMessage::GamePinToggleRequested(app_id))
        .padding(Padding::default().left(10).right(10).top(4).bottom(4))
        .style(move |t: &iced::Theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let p = palette(theme_from_iced(t));
            button::Style {
                background: Some(iced::Background::Color(if hovered {
                    Color { a: 0.90, ..p.hover }
                } else {
                    Color {
                        a: 0.75,
                        ..p.surface
                    }
                })),
                border: iced::Border {
                    color: if is_pinned {
                        Color { a: 0.6, ..p.accent }
                    } else {
                        p.border
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: if is_pinned { p.accent } else { p.text_primary },
                ..button::Style::default()
            }
        });

    container(pin_btn)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h))
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .padding(Padding::default().top(4).right(12))
        .into()
}
pub(super) fn build_name_row(
    entry: &GameEntry,
    card_w: f32,
) -> Element<'static, ProfileViewMessage> {
    const NAME_CHAR_W_PX: f32 = 7.0;
    const COUNTER_CHAR_W_PX: f32 = 6.5;
    const ROW_SPACING_PX: f32 = 4.0;

    let name_str: String = entry.name.as_deref().unwrap_or("").to_owned();
    let name_text = text(name_str.clone())
        .size(12)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_primary),
        })
        .wrapping(text::Wrapping::None);

    let counter_str: Option<String> = match entry.progress.as_ref() {
        Some(p) if p.total > 0 => Some(format!("{} / {}", p.earned, p.total)),
        _ => None,
    };

    let counter_w_estimate: f32 = counter_str
        .as_ref()
        .map(|s| s.chars().count() as f32 * COUNTER_CHAR_W_PX)
        .unwrap_or(0.0);

    let counter: Element<'static, ProfileViewMessage> = match counter_str {
        Some(s) => text(s)
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            })
            .into(),
        None => iced::widget::Space::new().width(Length::Shrink).into(),
    };

    let name_clipped = container(name_text).width(Length::Fill).clip(true);

    let available_for_name =
        card_w - 2.0 * CARD_HORIZONTAL_PADDING - counter_w_estimate - ROW_SPACING_PX;
    let estimated_name_w = name_str.chars().count() as f32 * NAME_CHAR_W_PX;
    let truncated = estimated_name_w > available_for_name;

    let name_node: Element<'static, ProfileViewMessage> = if truncated && !name_str.is_empty() {
        tooltip_box(name_clipped, name_str, iced::widget::tooltip::Position::Top)
    } else {
        name_clipped.into()
    };

    let inner = row![name_node, counter]
        .align_y(Alignment::Center)
        .spacing(ROW_SPACING_PX);

    container(inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(CARD_NAME_ROW_HEIGHT))
        .align_y(Alignment::Center)
        .padding(
            Padding::default()
                .left(CARD_HORIZONTAL_PADDING)
                .right(CARD_HORIZONTAL_PADDING)
                .top(CARD_NAME_ROW_PAD_TOP)
                .bottom(0),
        )
        .into()
}
pub(super) fn build_tags_row(
    entry: &GameEntry,
    card_w: f32,
    genre: Option<&str>,
    theme: AppTheme,
) -> Element<'static, ProfileViewMessage> {
    const PCT_TEXT_WIDTH: f32 = 24.0;
    const PCT_PILL_TOTAL_WIDTH: f32 = 51.0;
    const TAGS_ROW_GAP: f32 = 6.0;
    const PILL_PADDING_H: f32 = 16.0;
    const GENRE_CHAR_WIDTH: f32 = 5.5;

    let progress = entry.progress.as_ref();

    let completion_tag: Option<Element<'static, ProfileViewMessage>> = progress.and_then(|p| {
        if p.total == 0 {
            return None;
        }
        let pct = p.earned as f32 / p.total as f32 * 100.0;
        let tier_color_opt = completion_tier_color(pct, theme);
        let is_legendary = pct >= 100.0;
        let p_pal = palette(theme);
        let tint = tier_color_opt.unwrap_or(p_pal.rarity_common);
        let legendary_color = p_pal.rarity_legendary;

        let pct_text = text(format!("{pct:.0}%"))
            .size(11)
            .width(Length::Fixed(PCT_TEXT_WIDTH))
            .align_x(Alignment::Center)
            .style(move |t: &iced::Theme| iced::widget::text::Style {
                color: Some(
                    tier_color_opt.unwrap_or_else(|| palette(theme_from_iced(t)).text_muted),
                ),
            });
        let mut pill_el = pill(pct_text, tint).with_dot(tint);
        if is_legendary {
            pill_el = pill_el.glow(Color {
                a: 0.5,
                ..legendary_color
            });
        }

        Some(pill_el.into())
    });

    let genre_budget = card_w
        - 2.0 * CARD_HORIZONTAL_PADDING
        - PCT_PILL_TOTAL_WIDTH
        - TAGS_ROW_GAP
        - PILL_PADDING_H;
    let genre_max_chars = (genre_budget / GENRE_CHAR_WIDTH).max(3.0) as usize;

    let genre_tag: Option<Element<'static, ProfileViewMessage>> = genre.map(|g| {
        let tint = crate::ui::genre_color::genre_color(g);
        let display = truncate_with_ellipsis(g, genre_max_chars);
        pill(text(display).size(11).color(tint), tint).into()
    });

    let mut left_tags: iced::widget::Row<'static, ProfileViewMessage> =
        row![].spacing(6).align_y(Alignment::Center);

    if let Some(gtag) = genre_tag {
        left_tags = left_tags.push(gtag);
    }

    let mut tags: iced::widget::Row<'static, ProfileViewMessage> =
        row![left_tags, iced::widget::Space::new().width(Length::Fill)]
            .spacing(0)
            .align_y(Alignment::Center);

    if let Some(ctag) = completion_tag {
        tags = tags.push(ctag);
    }

    container(tags)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(CARD_TAGS_ROW_HEIGHT))
        .padding(
            Padding::default()
                .left(CARD_HORIZONTAL_PADDING)
                .right(CARD_HORIZONTAL_PADDING)
                .top(CARD_TAGS_ROW_PAD_TOP)
                .bottom(CARD_TAGS_ROW_PAD_BOTTOM),
        )
        .align_y(Alignment::End)
        .into()
}
