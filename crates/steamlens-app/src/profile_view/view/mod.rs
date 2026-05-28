mod card;
mod card_parts;
mod dims;

use std::collections::HashMap;

use iced::widget::Id as WidgetId;
use iced::widget::{container, text};
use iced::{Alignment, Element, Length};

use crate::cache::GameCacheEntry;
use crate::profile_view::ProfileViewState;
use crate::profile_view::types::{GameEntry, ProfileViewMessage, ProfileViewPhase};
use crate::profile_view::widget::{ProfileWidgetParams, profile_widget};
use crate::ui::grid::{GridLayout, responsive_card_grid};
use crate::ui::theme::{palette, theme_from_iced};

use card::build_card;
use dims::{CARD_GAP, MIN_GAP, card_width};

pub struct ProfileViewProps<'a> {
    pub user_profile: Option<&'a steamlens_core::UserProfile>,
    pub avatar_handle: Option<&'a iced::widget::image::Handle>,
    pub cached_entries: &'a HashMap<u32, GameCacheEntry>,
    pub skeleton_phase: f32,
    pub pinned: &'a [u32],
    pub steam_level: Option<u32>,
    pub steam_running: Option<bool>,
}

pub fn render<'a>(
    state: &'a ProfileViewState,
    props: ProfileViewProps<'a>,
) -> crate::screen::ScreenContent<'a, ProfileViewMessage> {
    let profile_section = build_profile_section(
        state,
        props.user_profile,
        props.avatar_handle,
        props.skeleton_phase,
        props.steam_level,
    );

    let body: Element<'_, ProfileViewMessage> = match &state.phase {
        ProfileViewPhase::Scanning => center_text("Scanning library\u{2026}"),
        ProfileViewPhase::Loaded => {
            if state.derived.visible_indices.is_empty() {
                center_text("No games found.")
            } else {
                let visible: Vec<&GameEntry> = state
                    .derived
                    .visible_indices
                    .iter()
                    .map(|&i| &state.games[i])
                    .collect();
                build_grid(
                    state,
                    visible,
                    props.cached_entries,
                    props.skeleton_phase,
                    props.pinned,
                )
            }
        }
    };

    crate::screen::ScreenContent {
        top: Some(profile_section),
        status_bar: profile_status_bar(state, props.steam_running),
        body,
        footer: None,
    }
}

fn profile_status_bar(
    state: &ProfileViewState,
    steam_running: Option<bool>,
) -> Option<Element<'_, ProfileViewMessage>> {
    use crate::ui::widgets::status_bar::status_bar;

    let total = state.games.len();

    if steam_running == Some(false) {
        if total == 0 {
            return None;
        }
        let failed = state.failed_app_ids.len();
        let hydrated = state.games.iter().filter(|g| g.is_hydrated()).count();
        return Some(
            status_bar::<ProfileViewMessage>()
                .offline(hydrated.max(total - failed), "games")
                .failed(failed)
                .into(),
        );
    }

    if total == 0 {
        return None;
    }

    let scanned_progress = state.games.iter().filter(|g| g.progress.is_some()).count();
    let loaded_capsules = state
        .games
        .iter()
        .filter(|g| !matches!(g.capsule, super::types::CapsuleAsset::Pending))
        .count();

    if scanned_progress < total {
        Some(
            status_bar::<ProfileViewMessage>()
                .scanning("Scanning library", scanned_progress, total)
                .into(),
        )
    } else if loaded_capsules < total {
        Some(
            status_bar::<ProfileViewMessage>()
                .scanning("Downloading capsules", loaded_capsules, total)
                .into(),
        )
    } else {
        Some(
            status_bar::<ProfileViewMessage>()
                .connected(total, "games", state.last_scan_completed_at)
                .into(),
        )
    }
}

fn build_profile_section<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    skeleton_phase: f32,
    steam_level: Option<u32>,
) -> Element<'a, ProfileViewMessage> {
    profile_widget(ProfileWidgetParams {
        user_profile,
        avatar_handle,
        summary: state.derived.summary,
        top6: state.derived.top6.clone(),
        games_count: state.games.len(),
        skeleton_phase,
        hovered_bar_slice: state.hovered_bar_slice,
        capsule_handles: &state.capsule_handles,
        capsule_size: state.capsule_size,
        steam_level,
    })
}

pub fn library_search_id() -> WidgetId {
    WidgetId::new("library-search")
}

fn build_grid<'a>(
    state: &'a ProfileViewState,
    visible: Vec<&'a GameEntry>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
    pinned: &'a [u32],
) -> Element<'a, ProfileViewMessage> {
    let capsule_size = state.capsule_size;
    let card_w = card_width(capsule_size);
    let hovered_card = state.hovered_card;
    let hovered_card_tier = state.hovered_card_tier;
    let pinned_set: std::collections::HashSet<u32> = pinned.iter().copied().collect();

    responsive_card_grid(
        visible,
        GridLayout {
            card_w,
            min_gap: MIN_GAP,
            row_spacing: CARD_GAP,
            padding_top: 8.0,
            padding_bottom: 8.0,
        },
        move |entry: &&'a GameEntry| {
            let app_id = entry.app_id;
            let cached = cached_entries.get(&app_id);
            let tier_breakdown = cached.map(|e| e.tier_breakdown.as_slice()).unwrap_or(&[]);
            let genre = cached.and_then(|e| e.genre.as_deref());
            let is_pinned = pinned_set.contains(&app_id);
            let is_hovered = hovered_card == Some(app_id);
            let hovered_tier = hovered_card_tier
                .filter(|(id, _)| *id == app_id)
                .map(|(_, t)| t);
            build_card(
                entry,
                capsule_size,
                card_w,
                skeleton_phase,
                tier_breakdown,
                genre,
                is_pinned,
                is_hovered,
                hovered_tier,
            )
        },
    )
}

fn center_text(msg: &str) -> Element<'_, ProfileViewMessage> {
    let msg = msg.to_owned();
    container(
        text(msg)
            .size(14)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}
