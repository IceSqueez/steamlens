use std::collections::HashMap;

use iced::widget::image::Handle as ImageHandle;
use iced::widget::{column, container, image as image_widget, row, text};
use iced::{Alignment, Element, Length};

use crate::cache::types::{CachedAchievement, GameCacheEntry};
use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::ui::theme::{AppTheme, palette, theme_from_iced};
use crate::ui::widgets::skeleton::{SKELETON_DEFAULT_RADIUS, skeleton_box};
use crate::ui::widgets::widget::{
    WidgetSummary, breakdown_row, cards_separator, closest_row, rarity_bar, rarity_cards,
    widget_panel,
};

use super::types::{GameEntry, ProfileViewMessage, StoredCapsule, TopEntry};

const AVATAR_SIZE: f32 = 100.0;
const AVATAR_RADIUS: f32 = 8.0;

pub fn compute_profile_summary(cached_entries: &HashMap<u32, GameCacheEntry>) -> WidgetSummary {
    let mut summary = WidgetSummary::default();

    for entry in cached_entries.values() {
        summary.achievement_total += entry.progress.total;
        summary.earned_total += entry.progress.earned;

        if !entry.tier_breakdown.is_empty() {
            for (tier, count) in &entry.tier_breakdown {
                summary.add_tier(*tier, *count);
            }
            continue;
        }

        if entry.achievements.is_empty() {
            continue;
        }
        let tier_map = compute_tier_map_from_cached(&entry.achievements);
        if tier_map.is_empty() {
            continue;
        }
        for ach in &entry.achievements {
            if !ach.is_achieved {
                continue;
            }
            if let Some(tier) = tier_map.get(&ach.api_name).copied() {
                summary.add_tier(tier, 1);
            }
        }
    }
    summary
}

fn compute_tier_map_from_cached(achievements: &[CachedAchievement]) -> HashMap<String, RarityTier> {
    let rated: Vec<(String, f32)> = achievements
        .iter()
        .filter_map(|a| a.global_percent.map(|p| (a.api_name.clone(), p as f32)))
        .collect();
    crate::game_view::types::assign_tiers_from_percentages(rated)
}

pub fn top6_closest_to_complete(
    games: &[GameEntry],
    cached_entries: &HashMap<u32, GameCacheEntry>,
) -> Vec<TopEntry> {
    let mut candidates: Vec<(u32, String, f64, u32, u32, u64)> = games
        .iter()
        .filter_map(|g| {
            let prog = g.progress.as_ref()?;
            if prog.earned == 0 || prog.total == 0 || prog.earned >= prog.total {
                return None;
            }
            let ratio = prog.earned as f64 / prog.total as f64;
            let last_played = cached_entries
                .get(&g.app_id)
                .map(|e| e.steam_last_played)
                .unwrap_or(0);

            Some((
                g.app_id,
                g.name.clone().unwrap_or_default(),
                ratio,
                prog.earned,
                prog.total,
                last_played,
            ))
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.5.cmp(&a.5))
    });

    candidates
        .into_iter()
        .take(6)
        .map(|(app_id, game_name, ratio, earned, total, _)| TopEntry {
            app_id,
            game_name,
            completion_pct: ratio * 100.0,
            earned,
            total,
        })
        .collect()
}

pub struct ProfileWidgetParams<'a> {
    pub user_profile: Option<&'a steamlens_core::UserProfile>,
    pub avatar_handle: Option<&'a iced::widget::image::Handle>,
    pub summary: WidgetSummary,
    pub top6: &'a [TopEntry],
    pub games_count: usize,
    pub skeleton_phase: f32,
    pub hovered_bar_slice: Option<RarityTier>,
    pub capsules: &'a crate::app_context::CapsuleStore,
    pub capsule_size: CapsuleSize,
    pub steam_level: Option<u32>,
    pub theme: AppTheme,
}

pub fn profile_widget<'a>(params: ProfileWidgetParams<'a>) -> Element<'a, ProfileViewMessage> {
    let left_col = build_left_column(
        params.user_profile,
        params.avatar_handle,
        &params.summary,
        params.games_count,
        params.skeleton_phase,
        params.hovered_bar_slice,
        params.steam_level,
        params.theme,
    );
    let right_col = build_right_column(
        params.top6,
        &params.capsules.handles,
        params.capsule_size,
        params.skeleton_phase,
    );
    widget_panel(left_col, right_col)
}

#[allow(clippy::too_many_arguments)]
fn build_left_column<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    summary: &WidgetSummary,
    games_count: usize,
    skeleton_phase: f32,
    hovered_bar_slice: Option<RarityTier>,
    steam_level: Option<u32>,
    theme: AppTheme,
) -> Element<'a, ProfileViewMessage> {
    let avatar = build_avatar(avatar_handle, skeleton_phase);
    let info = build_profile_info(user_profile, games_count, steam_level);

    let info_column = column![
        info,
        iced::widget::Space::new().height(Length::Fill),
        breakdown_row::<ProfileViewMessage>(summary),
    ]
    .spacing(4)
    .width(Length::Fill)
    .height(Length::Fixed(AVATAR_SIZE));

    let header_section = row![avatar, info_column]
        .spacing(14)
        .align_y(Alignment::Start);

    let bar: Element<'a, ProfileViewMessage> = rarity_bar::<ProfileViewMessage>(*summary, theme)
        .hovered(hovered_bar_slice)
        .on_hover(|tier| match tier {
            Some(t) => ProfileViewMessage::BarSliceHoverEntered(t),
            None => ProfileViewMessage::BarSliceHoverExited,
        })
        .into();

    column![
        header_section,
        bar,
        rarity_cards::<ProfileViewMessage>(summary, theme),
        iced::widget::Space::new().height(Length::Fill),
        cards_separator::<ProfileViewMessage>(summary),
    ]
    .spacing(10)
    .height(Length::Fill)
    .into()
}

fn build_profile_info<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    games_count: usize,
    steam_level: Option<u32>,
) -> Element<'a, ProfileViewMessage> {
    let persona = user_profile
        .map(|p| p.nickname.as_str())
        .unwrap_or("Steam User");

    let nickname =
        text(persona.to_string())
            .size(15)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_primary),
            });

    let level_str = match steam_level {
        Some(n) => format!("level ({n})"),
        None => "level (X)".to_owned(),
    };
    let level_label = text(level_str)
        .size(11)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).accent),
        });
    let profile_level = iced::widget::container(level_label)
        .padding(
            iced::Padding::default()
                .left(8u32)
                .right(8u32)
                .top(3u32)
                .bottom(3u32),
        )
        .style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color {
                    a: 0.15,
                    ..p.accent
                })),
                border: iced::Border {
                    color: iced::Color {
                        a: 0.40,
                        ..p.accent
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..iced::widget::container::Style::default()
            }
        });

    let nickname_row = row![nickname, profile_level]
        .spacing(6)
        .align_y(Alignment::Center);

    let tracked_games =
        text(format!("{games_count} games tracked"))
            .size(12)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            });

    column![nickname_row, tracked_games].spacing(2).into()
}

fn build_avatar<'a>(
    avatar_handle: Option<&'a ImageHandle>,
    skeleton_phase: f32,
) -> Element<'a, ProfileViewMessage> {
    if let Some(handle) = avatar_handle {
        return container(
            image_widget(handle.clone())
                .width(Length::Fixed(AVATAR_SIZE))
                .height(Length::Fixed(AVATAR_SIZE)),
        )
        .width(Length::Fixed(AVATAR_SIZE))
        .height(Length::Fixed(AVATAR_SIZE))
        .style(|_: &iced::Theme| container::Style {
            border: iced::Border {
                radius: AVATAR_RADIUS.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into();
    }
    skeleton_box(AVATAR_SIZE, AVATAR_SIZE, AVATAR_RADIUS, skeleton_phase)
}

fn build_right_column<'a>(
    top6: &'a [TopEntry],
    capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    capsule_size: CapsuleSize,
    skeleton_phase: f32,
) -> Element<'a, ProfileViewMessage> {
    let header =
        text("CLOSEST TO 100%")
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            });

    if top6.is_empty() {
        return column![
            header,
            text("Nothing to recommend yet")
                .size(12)
                .style(|t: &iced::Theme| iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).text_dim),
                }),
        ]
        .spacing(8)
        .into();
    }

    let mut rows_col = column![header].spacing(6);

    for entry in top6 {
        rows_col = rows_col.push(build_closest_row(
            entry,
            capsule_handles,
            capsule_size,
            skeleton_phase,
        ));
    }
    rows_col.into()
}

fn build_closest_row<'a>(
    entry: &'a TopEntry,
    capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    capsule_size: CapsuleSize,
    skeleton_phase: f32,
) -> Element<'a, ProfileViewMessage> {
    const CAPSULE_W: f32 = 60.0;
    const CAPSULE_H: f32 = 22.0;

    let capsule_el: Element<'a, ProfileViewMessage> =
        if let Some(stored) = capsule_handles.get(&(entry.app_id, capsule_size)) {
            container(
                image_widget(stored.handle.clone())
                    .width(Length::Fixed(CAPSULE_W))
                    .height(Length::Fixed(CAPSULE_H)),
            )
            .width(Length::Fixed(CAPSULE_W))
            .height(Length::Fixed(CAPSULE_H))
            .style(|_: &iced::Theme| container::Style {
                border: iced::Border {
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
        } else {
            skeleton_box(
                CAPSULE_W,
                CAPSULE_H,
                SKELETON_DEFAULT_RADIUS,
                skeleton_phase,
            )
        };

    let remaining = entry.total.saturating_sub(entry.earned);
    closest_row::<ProfileViewMessage>(
        capsule_el,
        entry.game_name.clone(),
        format!("{} of {} left", remaining, entry.total),
        format!("{:.0}%", entry.completion_pct),
        ProfileViewMessage::GameOpenRequested(entry.app_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, GameCacheEntry};
    use crate::profile_view::types::CapsuleAsset;
    use crate::progress_scan::ProgressData;

    fn make_entry_with_progress(app_id: u32, earned: u32, total: u32) -> GameEntry {
        GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some(format!("Game {app_id}")),
            capsule: CapsuleAsset::Unavailable,
            progress: Some(ProgressData { earned, total }),
            genre: None,
        }
    }

    fn make_cache_entry(app_id: u32, earned: u32, total: u32, last_played: u64) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: crate::cache::CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            steam_last_played: last_played,
            cached_at: 0,
            achievements: vec![],
            stats: vec![],
            progress: CachedProgress { earned, total },
            tier_breakdown: Vec::new(),
            genre: None,
            playtime_minutes: None,
        }
    }

    fn make_cached_ach(
        name: &str,
        global_percent: Option<f64>,
        is_achieved: bool,
    ) -> CachedAchievement {
        CachedAchievement {
            api_name: name.to_owned(),
            display_name: name.to_owned(),
            description: String::new(),
            is_hidden: false,
            icon_path: None,
            icon_locked_path: None,
            is_achieved,
            earned_at: None,
            global_percent,
        }
    }

    #[test]
    fn top6_order_by_ratio_descending() {
        let games = vec![
            make_entry_with_progress(1, 10, 100),
            make_entry_with_progress(2, 80, 100),
            make_entry_with_progress(3, 50, 100),
            make_entry_with_progress(4, 95, 100),
            make_entry_with_progress(5, 30, 100),
            make_entry_with_progress(6, 70, 100),
            make_entry_with_progress(7, 5, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = (1u32..=7)
            .map(|id| (id, make_cache_entry(id, 0, 0, 0)))
            .collect();

        let top6 = top6_closest_to_complete(&games, &cached);
        assert_eq!(top6.len(), 6);
        assert_eq!(top6[0].app_id, 4);
        assert_eq!(top6[1].app_id, 2);
        assert_eq!(top6[2].app_id, 6);
        assert_eq!(top6[3].app_id, 3);
        assert_eq!(top6[4].app_id, 5);
        assert_eq!(top6[5].app_id, 1);
    }

    #[test]
    fn top6_excludes_complete_games() {
        let games = vec![
            make_entry_with_progress(1, 100, 100),
            make_entry_with_progress(2, 50, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = (1u32..=2)
            .map(|id| (id, make_cache_entry(id, 0, 0, 0)))
            .collect();
        let top6 = top6_closest_to_complete(&games, &cached);
        assert_eq!(top6.len(), 1);
        assert_eq!(top6[0].app_id, 2);
    }

    #[test]
    fn compute_summary_uses_tier_breakdown_when_present() {
        let mut entry = make_cache_entry(1, 10, 20, 0);
        entry.tier_breakdown = vec![(RarityTier::Legendary, 1), (RarityTier::Common, 5)];
        let map: HashMap<u32, GameCacheEntry> = std::iter::once((1u32, entry)).collect();
        let summary = compute_profile_summary(&map);
        assert_eq!(summary.legendary_count, 1);
        assert_eq!(summary.common_count, 5);
    }

    #[test]
    fn compute_summary_falls_back_to_cached_achievements() {
        let achs = vec![
            make_cached_ach("a", Some(1.0), true),
            make_cached_ach("b", Some(2.0), true),
            make_cached_ach("c", Some(3.0), true),
            make_cached_ach("d", Some(50.0), false),
        ];
        let mut entry = make_cache_entry(1, 3, 4, 0);
        entry.achievements = achs;
        let map: HashMap<u32, GameCacheEntry> = std::iter::once((1u32, entry)).collect();
        let summary = compute_profile_summary(&map);
        assert_eq!(summary.earned_total, 3);
        assert_eq!(summary.achievement_total, 4);
        assert!(summary.rated_unlocked() >= 1);
    }
}
