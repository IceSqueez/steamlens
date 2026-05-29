use std::borrow::Cow;

use super::state::GameViewState;
use super::view::{
    achievement_search_id, build_back_leading, build_game_reload_button, tier_color,
};
use super::{GameViewMessage, types};

pub fn header_content<'a>(
    state: &'a GameViewState,
    theme: crate::ui::theme::AppTheme,
) -> crate::screen::AppHeaderContent<'a> {
    use crate::screen::{SegmentItem, SegmentedControlConfig};
    use types::AchievementSort;

    let sort_items: Vec<SegmentItem<'_>> = AchievementSort::ALL
        .iter()
        .copied()
        .map(|s| SegmentItem {
            label: Cow::Borrowed(s.short_label()),
            tooltip: Some(s.tooltip()),
            selected: state.achievement_sort == s,
            on_press: crate::Message::GameSortChanged(s),
        })
        .collect();

    let unlocked_toggle = SegmentedControlConfig {
        label: None,
        items: vec![SegmentItem {
            label: Cow::Borrowed("Unlocked at top"),
            tooltip: Some("Group unlocked achievements at the top"),
            selected: state.unlocked_at_top,
            on_press: crate::Message::GameView(GameViewMessage::UnlockedAtTopToggled),
        }],
    };

    crate::screen::AppHeaderContent {
        search: Some(crate::screen::SearchConfig {
            placeholder: "Search achievements\u{2026}",
            value: state.search_query.as_str(),
            id: achievement_search_id(),
        }),
        segments: vec![
            unlocked_toggle,
            SegmentedControlConfig {
                label: Some("SORT"),
                items: sort_items,
            },
        ],
        screen_actions: vec![build_game_reload_button()],
        leading: Some(build_back_leading()),
        status_filter: Some(build_achievement_status_strip(state)),
        category_filter: Some(build_rarity_tier_strip(state, theme)),
        theme,
    }
}

fn build_achievement_status_strip(state: &GameViewState) -> crate::screen::FilterStrip<'_> {
    use types::AchievementFilter;
    let buttons = [
        (AchievementFilter::All, "All"),
        (AchievementFilter::Unlocked, "Unlocked"),
        (AchievementFilter::Locked, "Locked"),
    ]
    .into_iter()
    .map(|(f, label)| crate::screen::FilterButton {
        label: Cow::Borrowed(label),
        selected: state.filter == f,
        on_press: crate::Message::GameView(GameViewMessage::FilterChanged(f)),
    })
    .collect();

    crate::screen::FilterStrip { buttons }
}

fn build_rarity_tier_strip(
    state: &GameViewState,
    theme: crate::ui::theme::AppTheme,
) -> iced::Element<'_, crate::Message> {
    use crate::ui::widgets::pill::pill;
    use iced::widget::{Space, row, text};
    use iced::{Alignment, Color, Length};
    use types::RarityTier;

    const TIER_PILL_RADIUS: f32 = 14.0;
    const TIER_PILL_PAD_H: u32 = 9;
    const TIER_PILL_PAD_V: u32 = 4;

    let tier_map = &state.derived.tier_map;
    let hidden_count = state
        .achievements
        .iter()
        .filter(|r| r.data.is_hidden)
        .count();

    let any_selected = !state.rarity_tier_set.is_empty() || state.include_hidden;

    let mut chips: Vec<iced::Element<'_, crate::Message>> = Vec::new();
    for (tier, label) in [
        (RarityTier::Common, "Common"),
        (RarityTier::Uncommon, "Uncommon"),
        (RarityTier::Rare, "Rare"),
        (RarityTier::Mythical, "Mythical"),
        (RarityTier::Legendary, "Legendary"),
    ] {
        let count = tier_map.values().filter(|&&v| v == tier).count();
        let color = tier_color(tier, theme);
        let inner = row![
            text(label).size(11).color(color),
            text(format!("{count}"))
                .size(11)
                .color(Color { a: 0.65, ..color }),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        let is_selected = state.rarity_tier_set.contains(&tier);
        let mut p = pill(inner, color)
            .radius(TIER_PILL_RADIUS)
            .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
            .reserve_dot_space(true)
            .selected(is_selected)
            .on_press(crate::Message::GameView(
                GameViewMessage::RarityTierToggled(tier),
            ));
        if !any_selected || is_selected {
            p = p.with_dot(color);
        }
        chips.push(p.into());
    }

    let hidden_color = crate::ui::theme::palette(crate::ui::theme::AppTheme::Dark).text_muted;
    let hidden_inner = row![
        text("Hidden").size(11).color(hidden_color),
        text(format!("{hidden_count}")).size(11).color(Color {
            a: 0.65,
            ..hidden_color
        }),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let mut hidden_pill = pill(hidden_inner, hidden_color)
        .radius(TIER_PILL_RADIUS)
        .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
        .reserve_dot_space(true)
        .selected(state.include_hidden)
        .on_press(crate::Message::GameView(GameViewMessage::HiddenPillToggled));
    if !any_selected || state.include_hidden {
        hidden_pill = hidden_pill.with_dot(hidden_color);
    }
    chips.push(hidden_pill.into());

    if !state.rarity_tier_set.is_empty() || state.include_hidden {
        chips.push(Space::new().width(Length::Fill).into());
        let clear_label = text("Clear").size(11).color(hidden_color);
        chips.push(
            pill(clear_label, hidden_color)
                .radius(TIER_PILL_RADIUS)
                .padding(TIER_PILL_PAD_H, TIER_PILL_PAD_V)
                .selected(false)
                .on_press(crate::Message::GameView(
                    GameViewMessage::RarityFilterCleared,
                ))
                .into(),
        );
    }

    row(chips)
        .spacing(6)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
}
