use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::{button, column, container, image as image_widget, row, svg, text};
use iced::{Alignment, Border, Color, Element, Length, Shadow, Vector};

use crate::capsule_cache::CapsuleSize;
use crate::theme::{C_ACCENT, C_BORDER, C_HOVER, C_TEXT_MUTED, C_TEXT_PRIMARY};
use crate::ui::widgets::pill::pill;
use crate::ui::widgets::skeleton::skeleton_box;
use crate::ui::widgets::widget::{
    WidgetSummary, breakdown_row, cards_separator, rarity_bar, rarity_cards, widget_panel,
};

use super::GameViewMessage;
use super::types::{AchievementRow, RarityTier, compute_tier_map};
use crate::profile_view::types::StoredCapsule;

static SVG_CLOCK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#6b6884" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><polyline points="12 7 12 12 16 14"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

static SVG_INVALIDATE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#bd93f9" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9"/><path d="M3 4v5h5"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

const CAPSULE_SLOT_W: f32 = 184.0;
const CAPSULE_SLOT_H: f32 = 276.0;
const CAPSULE_RADIUS: f32 = 10.0;
pub fn compute_game_summary(achievements: &[AchievementRow]) -> WidgetSummary {
    let tier_map = compute_tier_map(achievements);
    let mut s = WidgetSummary {
        earned_total: achievements.iter().filter(|r| r.data.is_achieved).count() as u32,
        achievement_total: achievements.len() as u32,
        ..WidgetSummary::default()
    };

    for ach in achievements {
        if !ach.data.is_achieved {
            continue;
        }
        match tier_map.get(&ach.data.id).copied() {
            Some(RarityTier::Legendary) => s.legendary_count += 1,
            Some(RarityTier::Mythical) => s.mythical_count += 1,
            Some(RarityTier::Rare) => s.rare_count += 1,
            Some(RarityTier::Uncommon) => s.uncommon_count += 1,
            Some(RarityTier::Common) => s.common_count += 1,
            None => {}
        }
    }
    s
}

pub struct GameWidgetParams<'a> {
    pub app_id: u32,
    pub game_name: &'a str,
    pub genre: Option<&'a str>,
    pub playtime_minutes: Option<u32>,
    pub achievements: &'a [AchievementRow],
    pub stats: &'a [super::types::StatRow],
    pub stats_search_query: &'a str,
    pub capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub skeleton_phase: f32,
    pub hovered_bar_slice: Option<RarityTier>,
}

pub fn game_widget<'a>(params: GameWidgetParams<'a>) -> Element<'a, GameViewMessage> {
    let summary = compute_game_summary(params.achievements);

    let capsule_el = build_capsule(params.app_id, params.capsule_handles, params.skeleton_phase);
    let inner_col = build_left_column(
        params.app_id,
        params.game_name,
        params.genre,
        params.playtime_minutes,
        &summary,
        params.hovered_bar_slice,
    );
    let left_content: Element<'a, GameViewMessage> = row![capsule_el, inner_col]
        .spacing(16)
        .align_y(Alignment::Center)
        .into();
    let right_col = build_stats_panel(params.stats, params.stats_search_query);
    widget_panel(left_content, right_col)
}

const HEADER_BLOCK_H: f32 = 64.0;

fn build_left_column<'a>(
    app_id: u32,
    game_name: &'a str,
    genre: Option<&'a str>,
    playtime_minutes: Option<u32>,
    summary: &WidgetSummary,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, GameViewMessage> {
    let header_row = build_game_header(app_id, game_name, genre, playtime_minutes);
    let bar: Element<'a, GameViewMessage> = rarity_bar::<GameViewMessage>(*summary)
        .hovered(hovered_bar_slice)
        .on_hover(|tier| match tier {
            Some(t) => GameViewMessage::BarSliceHoverEnter(t),
            None => GameViewMessage::BarSliceHoverExit,
        })
        .into();

    column![
        header_row,
        breakdown_row::<GameViewMessage>(summary),
        bar,
        rarity_cards::<GameViewMessage>(summary),
        iced::widget::Space::new().height(Length::Fill),
        cards_separator::<GameViewMessage>(summary),
    ]
    .spacing(10)
    .height(Length::Fill)
    .into()
}

fn build_game_header<'a>(
    app_id: u32,
    game_name: &'a str,
    genre: Option<&'a str>,
    playtime_minutes: Option<u32>,
) -> Element<'a, GameViewMessage> {
    let name = text(game_name.to_owned()).size(15).color(C_TEXT_PRIMARY);
    let appid_pill = pill(
        text(format!("AppID {app_id}")).size(11).color(C_ACCENT),
        C_ACCENT,
    )
    .radius(4.0);

    let invalidate_btn = build_invalidate_button(app_id);

    let name_row = row![
        name,
        appid_pill,
        iced::widget::Space::new().width(Length::Fill),
        invalidate_btn,
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let genre_text = text(genre.unwrap_or("Unknown genre").to_owned())
        .size(12)
        .color(C_TEXT_MUTED);

    let clock_icon = svg(SVG_CLOCK.clone())
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0));

    let playtime_label = format_playtime(playtime_minutes);
    let playtime_row = row![
        clock_icon,
        text(playtime_label).size(12).color(C_TEXT_MUTED),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let info = column![name_row, genre_text, playtime_row].spacing(2);

    container(info)
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_BLOCK_H))
        .align_y(Alignment::Start)
        .into()
}

fn build_invalidate_button<'a>(app_id: u32) -> Element<'a, GameViewMessage> {
    use crate::ui::widgets::tooltip_box::tooltip_box;

    let icon = svg(SVG_INVALIDATE.clone())
        .width(Length::Fixed(14.0))
        .height(Length::Fixed(14.0));
    let btn = button(
        container(icon)
            .width(Length::Fixed(28.0))
            .height(Length::Fixed(28.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press(GameViewMessage::InvalidateCacheClicked(app_id))
    .padding(0)
    .style(|_theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(iced::Background::Color(if hovered {
                C_HOVER
            } else {
                Color::TRANSPARENT
            })),
            border: Border {
                color: if hovered { C_ACCENT } else { C_BORDER },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..button::Style::default()
        }
    });

    tooltip_box(
        btn,
        "Clear cached data for this game",
        iced::widget::tooltip::Position::Left,
    )
}

fn format_playtime(minutes: Option<u32>) -> String {
    let Some(m) = minutes else {
        return "Never played".to_owned();
    };
    if m == 0 {
        return "Never played".to_owned();
    }
    let hours = m / 60;
    let mins = m % 60;
    if hours == 0 {
        format!("{mins}m played")
    } else if mins == 0 {
        format!("{hours}h played")
    } else {
        format!("{hours}h {mins}m played")
    }
}

fn build_capsule<'a>(
    app_id: u32,
    capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    skeleton_phase: f32,
) -> Element<'a, GameViewMessage> {
    if let Some(stored) = capsule_handles.get(&(app_id, CapsuleSize::Portrait)) {
        return container(
            image_widget(stored.handle.clone())
                .width(Length::Fixed(CAPSULE_SLOT_W))
                .height(Length::Fixed(CAPSULE_SLOT_H)),
        )
        .width(Length::Fixed(CAPSULE_SLOT_W))
        .height(Length::Fixed(CAPSULE_SLOT_H))
        .style(|_: &iced::Theme| container::Style {
            border: Border {
                radius: CAPSULE_RADIUS.into(),
                ..Border::default()
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.55),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
            ..container::Style::default()
        })
        .into();
    }
    skeleton_box(
        CAPSULE_SLOT_W,
        CAPSULE_SLOT_H,
        CAPSULE_RADIUS,
        skeleton_phase,
    )
}

pub use super::stats_panel::build_stats_panel;

#[cfg(test)]
mod tests {
    use super::*;
    use steamlens_core::AchievementData;

    fn make_ach(id: &str, is_achieved: bool, pct: Option<f32>) -> AchievementRow {
        let data = AchievementData {
            id: id.to_owned(),
            display_name: id.to_owned(),
            description: String::new(),
            is_hidden: false,
            is_achieved,
            unlock_time: None,
            permission: 0,
            icon: None,
        };
        let mut row = AchievementRow::from(data);
        row.appeared = true;
        row.rarity_percent = pct;
        row
    }

    #[test]
    fn compute_summary_counts_only_achieved() {
        let rows = vec![
            make_ach("a1", true, Some(1.0)),
            make_ach("a2", true, Some(5.0)),
            make_ach("a3", false, Some(10.0)),
            make_ach("a4", true, Some(50.0)),
        ];
        let s = compute_game_summary(&rows);
        assert_eq!(s.earned_total, 3);
        assert_eq!(s.achievement_total, 4);
    }
}
