use std::collections::HashMap;

use iced::widget::{column, container, image as img_widget, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::types::{CachedAchievement, GameCacheEntry};
use crate::game_view::types::RarityTier;

use super::types::GameEntry;

const C_SURFACE: Color = Color::from_rgb(0.267, 0.278, 0.353);
const C_SURFACE_DEEP: Color = Color::from_rgb(0.18, 0.188, 0.235);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_TEXT: Color = Color::from_rgb(0.973, 0.973, 0.949);
const C_ACCENT: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_CYAN: Color = Color::from_rgb(0.545, 0.914, 0.992);
const C_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
const C_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);
const C_GREEN: Color = Color::from_rgb(0.314, 0.980, 0.482);
const C_RARE: Color = Color::from_rgb(0.545, 0.914, 0.992);
const C_COMMON: Color = Color::from_rgb(0.4, 0.45, 0.55);

pub struct ProfileSummary {
    pub earned_total: u32,
    pub achievement_total: u32,
    pub legendary_count: u32,
    pub mythical_count: u32,
    pub rare_count: u32,
    pub uncommon_count: u32,
    pub common_count: u32,
}

impl ProfileSummary {
    pub fn level(&self) -> u32 {
        compute_level(self.earned_total, self.legendary_count, self.mythical_count)
    }
}

/// Computes a synthetic user level from achievement counts.
///
/// Formula: (earned + 5 * legendary + 2 * mythical) / 100.
/// Legendary and Mythical achievements contribute extra weight to reflect
/// their comparative rarity within a game's achievement set.
pub fn compute_level(earned: u32, legendary: u32, mythical: u32) -> u32 {
    ((earned as u64 + 5 * legendary as u64 + 2 * mythical as u64) / 100) as u32
}

pub fn compute_profile_summary(cached_entries: &HashMap<u32, GameCacheEntry>) -> ProfileSummary {
    let mut earned_total: u32 = 0;
    let mut achievement_total: u32 = 0;
    let mut legendary_count: u32 = 0;
    let mut mythical_count: u32 = 0;
    let mut rare_count: u32 = 0;
    let mut uncommon_count: u32 = 0;
    let mut common_count: u32 = 0;

    for entry in cached_entries.values() {
        achievement_total += entry.progress.total;
        earned_total += entry.progress.earned;

        if entry.achievements.is_empty() {
            continue;
        }

        let tier_map = compute_tier_map_from_cached(&entry.achievements);
        for ach in &entry.achievements {
            if !ach.earned {
                continue;
            }
            match tier_map.get(&ach.api_name).copied() {
                Some(RarityTier::Legendary) => legendary_count += 1,
                Some(RarityTier::Mythical) => mythical_count += 1,
                Some(RarityTier::Rare) => rare_count += 1,
                Some(RarityTier::Uncommon) => uncommon_count += 1,
                Some(RarityTier::Common) => common_count += 1,
                None => {}
            }
        }
    }

    ProfileSummary {
        earned_total,
        achievement_total,
        legendary_count,
        mythical_count,
        rare_count,
        uncommon_count,
        common_count,
    }
}

const LEGENDARY_TOP_N: usize = 3;

fn compute_tier_map_from_cached(achievements: &[CachedAchievement]) -> HashMap<String, RarityTier> {
    let mut rated: Vec<(String, f64)> = achievements
        .iter()
        .filter_map(|a| a.global_percent.map(|p| (a.api_name.clone(), p)))
        .collect();

    if rated.is_empty() {
        return HashMap::new();
    }

    rated.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let total = rated.len();
    let mut legendary_n = total.min(LEGENDARY_TOP_N);
    if legendary_n > 0 && legendary_n < total {
        let threshold = rated[legendary_n - 1].1;
        while legendary_n < total && (rated[legendary_n].1 - threshold).abs() < 0.001 {
            legendary_n += 1;
        }
    }
    let remaining = total - legendary_n;

    let mythical_n = (remaining as f64 * 0.10).round() as usize;
    let rare_n = (remaining as f64 * 0.15).round() as usize;
    let uncommon_n = (remaining as f64 * 0.25).round() as usize;
    let common_n = remaining
        .saturating_sub(mythical_n)
        .saturating_sub(rare_n)
        .saturating_sub(uncommon_n);

    let mut map = HashMap::with_capacity(total);
    let mut idx = 0;

    for (id, _) in &rated[idx..idx + legendary_n] {
        map.insert(id.clone(), RarityTier::Legendary);
    }
    idx += legendary_n;
    for (id, _) in &rated[idx..idx + mythical_n] {
        map.insert(id.clone(), RarityTier::Mythical);
    }
    idx += mythical_n;
    for (id, _) in &rated[idx..idx + rare_n] {
        map.insert(id.clone(), RarityTier::Rare);
    }
    idx += rare_n;
    for (id, _) in &rated[idx..idx + uncommon_n] {
        map.insert(id.clone(), RarityTier::Uncommon);
    }
    idx += uncommon_n;
    for (id, _) in &rated[idx..idx + common_n] {
        map.insert(id.clone(), RarityTier::Common);
    }

    map
}

fn format_thousands(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn format_short(n: u32) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f32 / 1000.0)
    } else {
        n.to_string()
    }
}

pub fn top5_closest_to_complete<'a>(
    games: &'a [GameEntry],
    cached_entries: &HashMap<u32, GameCacheEntry>,
) -> Vec<&'a GameEntry> {
    let mut candidates: Vec<(&GameEntry, f64, u64)> = games
        .iter()
        .filter_map(|g| {
            let prog = g.progress.as_ref()?;
            if prog.earned == 0 || prog.total == 0 || prog.earned >= prog.total {
                return None;
            }
            let ratio = prog.earned as f64 / prog.total as f64;
            let last_played = cached_entries
                .get(&g.summary.app_id)
                .map(|e| e.steam_last_played)
                .unwrap_or(0);
            Some((g, ratio, last_played))
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.2.cmp(&a.2))
    });

    candidates.into_iter().take(5).map(|(g, _, _)| g).collect()
}

pub fn profile_widget<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    summary: &ProfileSummary,
    top5: Vec<&'a GameEntry>,
    capsule_handles: &'a HashMap<
        (u32, crate::capsule_cache::CapsuleSize),
        super::types::StoredCapsule,
    >,
) -> Element<'a, crate::Message> {
    let section1 = build_header_row(user_profile, summary);
    let section2 = build_tier_row(summary);
    let section3 = build_closest_row(top5, capsule_handles);

    let inner = column![section1, section2, section3]
        .spacing(12)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12));

    container(inner)
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE_DEEP)),
            border: iced::Border {
                color: Color { a: 0.3, ..C_ACCENT },
                width: 0.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn build_header_row<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    summary: &ProfileSummary,
) -> Element<'a, crate::Message> {
    let avatar: Element<'_, crate::Message> = build_avatar(user_profile);

    let persona = user_profile
        .map(|p| p.persona_name.as_str())
        .unwrap_or("Steam User");

    let level = summary.level();
    let level_chip = container(text(format!("Lvl {level}")).size(11).color(C_TEXT))
        .padding(Padding::default().left(6).right(6).top(2).bottom(2))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color {
                a: 0.25,
                ..C_ACCENT
            })),
            border: iced::Border {
                color: Color { a: 0.5, ..C_ACCENT },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        });

    let name_row = row![text(persona).size(18).color(C_TEXT), level_chip,]
        .spacing(8)
        .align_y(Alignment::Center);

    let progress_pct = if summary.achievement_total > 0 {
        summary.earned_total as f32 / summary.achievement_total as f32
    } else {
        0.0
    };
    let pct_label = format!("{:.1}%", progress_pct * 100.0);
    let fraction_label = format!(
        "{} / {}",
        format_thousands(summary.earned_total),
        format_thousands(summary.achievement_total)
    );

    let progress_bar = progress_bar_widget(progress_pct);

    let right_col = column![
        name_row,
        text(fraction_label).size(12).color(C_MUTED),
        progress_bar,
        text(pct_label).size(11).color(C_MUTED),
    ]
    .spacing(4)
    .width(Length::Fill);

    row![avatar, right_col]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
}

fn build_avatar<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
) -> Element<'a, crate::Message> {
    let size = 64.0f32;

    if let Some(profile) = user_profile
        && let Some(bytes) = profile.avatar_png_bytes.as_deref()
        && let Ok(dyn_img) = image::load_from_memory(bytes)
    {
        let rgba = dyn_img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let handle = iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw());
        return container(
            img_widget(handle)
                .width(Length::Fixed(size))
                .height(Length::Fixed(size)),
        )
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(|_: &iced::Theme| container::Style {
            border: iced::Border {
                color: Color { a: 0.6, ..C_ACCENT },
                width: 2.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .into();
    }

    let initial = user_profile
        .map(|p| p.persona_name.as_str())
        .unwrap_or("?")
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?');

    container(text(initial.to_string()).size(28).color(C_ACCENT))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color {
                r: 0.18,
                g: 0.14,
                b: 0.28,
                a: 1.0,
            })),
            border: iced::Border {
                color: Color { a: 0.6, ..C_ACCENT },
                width: 2.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn progress_bar_widget(fraction: f32) -> Element<'static, crate::Message> {
    let bar_color = if fraction >= 0.9 {
        C_CYAN
    } else if fraction >= 0.5 {
        C_MYTHICAL
    } else {
        C_ACCENT
    };

    let fill_width = fraction.clamp(0.0, 1.0) * 400.0;

    let fill = container(iced::widget::Space::new())
        .width(Length::Fixed(fill_width))
        .height(Length::Fixed(4.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(bar_color)),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let track = container(fill)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(4.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.3, 0.3, 0.35, 0.5,
            ))),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    track.into()
}

fn tier_chip_color(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_COMMON,
        RarityTier::Uncommon => C_GREEN,
        RarityTier::Rare => C_RARE,
        RarityTier::Mythical => C_MYTHICAL,
        RarityTier::Legendary => C_LEGENDARY,
    }
}

fn tier_abbrev(tier: RarityTier) -> &'static str {
    match tier {
        RarityTier::Common => "C",
        RarityTier::Uncommon => "U",
        RarityTier::Rare => "R",
        RarityTier::Mythical => "M",
        RarityTier::Legendary => "L",
    }
}

fn build_tier_chip<'a>(tier: RarityTier, count: u32) -> Element<'a, crate::Message> {
    let color = tier_chip_color(tier);
    let label = format!("{} {}", format_short(count), tier_abbrev(tier));

    container(
        row![
            container(iced::widget::Space::new())
                .width(Length::Fixed(8.0))
                .height(Length::Fixed(8.0))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: iced::Border {
                        radius: 1.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
            text(label).size(11).color(color),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::default().left(6).right(6).top(2).bottom(2))
    .style(move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(Color { a: 0.12, ..color })),
        border: iced::Border {
            color: Color { a: 0.3, ..color },
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn build_tier_row<'a>(summary: &ProfileSummary) -> Element<'a, crate::Message> {
    let chips = row![
        build_tier_chip(RarityTier::Legendary, summary.legendary_count),
        build_tier_chip(RarityTier::Mythical, summary.mythical_count),
        build_tier_chip(RarityTier::Rare, summary.rare_count),
        build_tier_chip(RarityTier::Uncommon, summary.uncommon_count),
        build_tier_chip(RarityTier::Common, summary.common_count),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    container(chips).width(Length::Fill).into()
}

fn build_closest_row<'a>(
    top5: Vec<&'a GameEntry>,
    capsule_handles: &'a HashMap<
        (u32, crate::capsule_cache::CapsuleSize),
        super::types::StoredCapsule,
    >,
) -> Element<'a, crate::Message> {
    use iced::widget::button;

    let header = text("Closest to complete").size(13).color(C_MUTED);

    if top5.is_empty() {
        return column![
            header,
            text("Nothing to recommend yet")
                .size(12)
                .color(Color { a: 0.5, ..C_MUTED }),
        ]
        .spacing(6)
        .into();
    }

    const MINI_W: f32 = 92.0;
    const MINI_H: f32 = 43.0;

    let mut cards_row = row![].spacing(8);

    for entry in top5 {
        let app_id = entry.summary.app_id;
        let pct = entry.progress.as_ref().map_or(0.0, |p| {
            if p.total > 0 {
                p.earned as f32 / p.total as f32 * 100.0
            } else {
                0.0
            }
        });
        let pct_label = format!("{pct:.0}%");

        let image_area: Element<'_, crate::Message> = {
            let key = (app_id, crate::capsule_cache::CapsuleSize::Small);
            if let Some(stored) = capsule_handles.get(&key) {
                container(
                    img_widget(stored.handle.clone())
                        .width(Length::Fixed(MINI_W))
                        .height(Length::Fixed(MINI_H)),
                )
                .width(Length::Fixed(MINI_W))
                .height(Length::Fixed(MINI_H))
                .style(|_: &iced::Theme| container::Style {
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                })
                .into()
            } else {
                let initial = entry
                    .summary
                    .name
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .next()
                    .unwrap_or('?');
                container(text(initial.to_string()).size(16).color(C_MUTED))
                    .width(Length::Fixed(MINI_W))
                    .height(Length::Fixed(MINI_H))
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(C_SURFACE)),
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..iced::Border::default()
                        },
                        ..container::Style::default()
                    })
                    .into()
            }
        };

        let card_col = column![
            image_area,
            container(text(pct_label).size(11).color(C_MUTED))
                .width(Length::Fixed(MINI_W))
                .align_x(Alignment::Center),
        ]
        .spacing(2)
        .align_x(Alignment::Center);

        let card_btn = button(card_col)
            .on_press(crate::Message::OpenGameView(app_id))
            .padding(0)
            .style(move |_: &iced::Theme, status| {
                let hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: None,
                    border: iced::Border {
                        color: if hovered {
                            Color { a: 0.8, ..C_ACCENT }
                        } else {
                            Color::TRANSPARENT
                        },
                        width: if hovered { 1.5 } else { 0.0 },
                        radius: 4.0.into(),
                    },
                    shadow: if hovered {
                        iced::Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                            offset: iced::Vector::new(0.0, -2.0),
                            blur_radius: 6.0,
                        }
                    } else {
                        iced::Shadow::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        cards_row = cards_row.push(card_btn);
    }

    column![header, cards_row].spacing(6).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, GameCacheEntry};
    use crate::profile_view::types::{CapsuleAsset, GameEntry};
    use crate::progress_scan::ProgressData;
    use steamlens_core::GameSummary;

    fn make_summary_gs(app_id: u32) -> GameSummary {
        GameSummary {
            app_id,
            name: format!("Game {app_id}"),
            last_played: None,
            achievement_count: 10,
            last_updated: 0,
            manifest_path: std::path::PathBuf::new(),
        }
    }

    fn make_entry_with_progress(app_id: u32, earned: u32, total: u32) -> GameEntry {
        GameEntry {
            summary: make_summary_gs(app_id),
            capsule: CapsuleAsset::Unavailable,
            progress: Some(ProgressData { earned, total }),
        }
    }

    fn make_cache_entry(app_id: u32, earned: u32, total: u32, last_played: u64) -> GameCacheEntry {
        GameCacheEntry {
            schema_version: crate::cache::CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            steam_last_updated: 0,
            steam_last_played: last_played,
            cached_at: 0,
            achievements: vec![],
            stats: vec![],
            progress: CachedProgress { earned, total },
        }
    }

    fn make_cache_entry_with_achievements(
        app_id: u32,
        achievements: Vec<CachedAchievement>,
    ) -> GameCacheEntry {
        let earned = achievements.iter().filter(|a| a.earned).count() as u32;
        let total = achievements.len() as u32;
        GameCacheEntry {
            schema_version: crate::cache::CURRENT_SCHEMA_VERSION,
            app_id,
            name: format!("Game {app_id}"),
            steam_last_updated: 0,
            steam_last_played: 0,
            cached_at: 0,
            achievements,
            stats: vec![],
            progress: CachedProgress { earned, total },
        }
    }

    fn make_cached_ach(name: &str, global_percent: Option<f64>, earned: bool) -> CachedAchievement {
        CachedAchievement {
            api_name: name.to_owned(),
            display_name: name.to_owned(),
            description: String::new(),
            hidden: false,
            icon_path: None,
            icon_locked_path: None,
            earned,
            earned_at: None,
            global_percent,
        }
    }

    #[test]
    fn compute_level_zero() {
        assert_eq!(compute_level(0, 0, 0), 0);
    }

    #[test]
    fn compute_level_earned_only() {
        assert_eq!(compute_level(100, 0, 0), 1);
    }

    #[test]
    fn compute_level_legendary_weight() {
        assert_eq!(compute_level(0, 20, 0), 1);
    }

    #[test]
    fn compute_level_mythical_weight() {
        assert_eq!(compute_level(0, 0, 50), 1);
    }

    #[test]
    fn top5_order_by_ratio_descending() {
        let games = vec![
            make_entry_with_progress(1, 10, 100),
            make_entry_with_progress(2, 80, 100),
            make_entry_with_progress(3, 50, 100),
            make_entry_with_progress(4, 95, 100),
            make_entry_with_progress(5, 30, 100),
            make_entry_with_progress(6, 70, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = (1u32..=6)
            .map(|id| (id, make_cache_entry(id, 0, 0, 0)))
            .collect();

        let top5 = top5_closest_to_complete(&games, &cached);
        assert_eq!(top5.len(), 5);
        assert_eq!(top5[0].summary.app_id, 4, "95% first");
        assert_eq!(top5[1].summary.app_id, 2, "80% second");
        assert_eq!(top5[2].summary.app_id, 6, "70% third");
        assert_eq!(top5[3].summary.app_id, 3, "50% fourth");
        assert_eq!(top5[4].summary.app_id, 5, "30% fifth");
    }

    #[test]
    fn top5_excludes_complete_games() {
        let games = vec![
            make_entry_with_progress(1, 100, 100),
            make_entry_with_progress(2, 50, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = vec![
            (1u32, make_cache_entry(1, 100, 100, 0)),
            (2u32, make_cache_entry(2, 50, 100, 0)),
        ]
        .into_iter()
        .collect();

        let top5 = top5_closest_to_complete(&games, &cached);
        assert_eq!(top5.len(), 1);
        assert_eq!(top5[0].summary.app_id, 2);
    }

    #[test]
    fn top5_excludes_zero_earned() {
        let games = vec![
            make_entry_with_progress(1, 0, 100),
            make_entry_with_progress(2, 50, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = vec![
            (1u32, make_cache_entry(1, 0, 100, 0)),
            (2u32, make_cache_entry(2, 50, 100, 0)),
        ]
        .into_iter()
        .collect();

        let top5 = top5_closest_to_complete(&games, &cached);
        assert_eq!(top5.len(), 1);
        assert_eq!(top5[0].summary.app_id, 2);
    }

    #[test]
    fn top5_empty_when_all_complete() {
        let games = vec![
            make_entry_with_progress(1, 100, 100),
            make_entry_with_progress(2, 50, 50),
        ];
        let cached: HashMap<u32, GameCacheEntry> = vec![
            (1u32, make_cache_entry(1, 100, 100, 0)),
            (2u32, make_cache_entry(2, 50, 50, 0)),
        ]
        .into_iter()
        .collect();

        let top5 = top5_closest_to_complete(&games, &cached);
        assert!(top5.is_empty());
    }

    #[test]
    fn top5_tiebreak_by_last_played_descending() {
        let games = vec![
            make_entry_with_progress(1, 50, 100),
            make_entry_with_progress(2, 50, 100),
        ];
        let cached: HashMap<u32, GameCacheEntry> = vec![
            (1u32, make_cache_entry(1, 50, 100, 1000)),
            (2u32, make_cache_entry(2, 50, 100, 2000)),
        ]
        .into_iter()
        .collect();

        let top5 = top5_closest_to_complete(&games, &cached);
        assert_eq!(top5[0].summary.app_id, 2, "more recent first on tiebreak");
    }

    #[test]
    fn card_visibility_filter_none_progress_excluded() {
        let games: Vec<GameEntry> = vec![
            GameEntry {
                summary: make_summary_gs(1),
                capsule: CapsuleAsset::Unavailable,
                progress: None,
            },
            make_entry_with_progress(2, 10, 50),
        ];
        let visible: Vec<&GameEntry> = games.iter().filter(|g| g.progress.is_some()).collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].summary.app_id, 2);
    }

    #[test]
    fn tier_aggregation_two_games() {
        let ach1 = make_cached_ach("a", Some(1.0), true);
        let ach2 = make_cached_ach("b", Some(50.0), true);
        let ach3 = make_cached_ach("c", Some(99.0), false);

        let entry1 = make_cache_entry_with_achievements(1, vec![ach1, ach2, ach3]);

        let ach4 = make_cached_ach("d", Some(2.0), true);
        let entry2 = make_cache_entry_with_achievements(2, vec![ach4]);

        let mut cached: HashMap<u32, GameCacheEntry> = HashMap::new();
        cached.insert(1, entry1);
        cached.insert(2, entry2);

        let summary = compute_profile_summary(&cached);
        assert_eq!(
            summary.earned_total, 3,
            "2 earned from game1 + 1 from game2"
        );
        assert_eq!(summary.achievement_total, 4);
        let tier_earned = summary.legendary_count
            + summary.mythical_count
            + summary.rare_count
            + summary.uncommon_count
            + summary.common_count;
        assert_eq!(
            tier_earned, 3,
            "tier counts cover all 3 earned achievements"
        );
    }

    #[test]
    fn format_thousands_basic() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1000), "1,000");
        assert_eq!(format_thousands(1234567), "1,234,567");
    }

    #[test]
    fn format_short_basic() {
        assert_eq!(format_short(999), "999");
        assert_eq!(format_short(1000), "1.0k");
        assert_eq!(format_short(1500), "1.5k");
    }
}
