use std::collections::HashMap;
use std::time::Instant;

use iced::widget::{button, column, container, image as image_widget, row, text};
use iced::widget::image::Handle as ImageHandle;
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::types::{CachedAchievement, GameCacheEntry};
use crate::game_view::types::RarityTier;
use crate::profile_view::types::LoaderPhase;
use crate::theme::{
    C_ACCENT, C_APP, C_BORDER, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY,
};

use super::types::{GameEntry, TopEntry};

pub(crate) const C_RARITY_COMMON: Color = Color::from_rgb(0.314, 0.980, 0.482);
pub(crate) const C_RARITY_UNCOMMON: Color = Color::from_rgb(0.545, 0.914, 0.992);
pub(crate) const C_RARITY_RARE: Color = Color::from_rgb(0.741, 0.576, 0.976);
pub(crate) const C_RARITY_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
pub(crate) const C_RARITY_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

fn rarity_color(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_RARITY_COMMON,
        RarityTier::Uncommon => C_RARITY_UNCOMMON,
        RarityTier::Rare => C_RARITY_RARE,
        RarityTier::Mythical => C_RARITY_MYTHICAL,
        RarityTier::Legendary => C_RARITY_LEGENDARY,
    }
}

fn rarity_label(tier: RarityTier) -> &'static str {
    match tier {
        RarityTier::Common => "COMMON",
        RarityTier::Uncommon => "UNCOMMON",
        RarityTier::Rare => "RARE",
        RarityTier::Mythical => "MYTHICAL",
        RarityTier::Legendary => "LEGENDARY",
    }
}

pub struct ProfileSummary {
    pub earned_total: u32,
    pub achievement_total: u32,
    pub legendary_count: u32,
    pub mythical_count: u32,
    pub rare_count: u32,
    pub uncommon_count: u32,
    pub common_count: u32,
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

pub fn format_thousands(n: u32) -> String {
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

pub fn top5_closest_to_complete(
    games: &[GameEntry],
    cached_entries: &HashMap<u32, GameCacheEntry>,
) -> Vec<TopEntry> {
    let mut candidates: Vec<(u32, String, f64, u64, Option<RarityTier>)> = games
        .iter()
        .filter_map(|g| {
            let prog = g.progress.as_ref()?;
            if prog.earned == 0 || prog.total == 0 || prog.earned >= prog.total {
                return None;
            }
            let ratio = prog.earned as f64 / prog.total as f64;
            let cache = cached_entries.get(&g.summary.app_id);
            let last_played = cache.map(|e| e.steam_last_played).unwrap_or(0);

            let rarity_tier = cache.and_then(|e| {
                if e.achievements.is_empty() {
                    return None;
                }
                let tier_map = compute_tier_map_from_cached(&e.achievements);
                e.achievements
                    .iter()
                    .filter(|a| a.earned)
                    .filter_map(|a| tier_map.get(&a.api_name).copied())
                    .max_by_key(|t| match t {
                        RarityTier::Common => 0u8,
                        RarityTier::Uncommon => 1,
                        RarityTier::Rare => 2,
                        RarityTier::Mythical => 3,
                        RarityTier::Legendary => 4,
                    })
            });

            Some((
                g.summary.app_id,
                g.summary.name.clone(),
                ratio,
                last_played,
                rarity_tier,
            ))
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.3.cmp(&a.3))
    });

    candidates
        .into_iter()
        .take(5)
        .map(|(app_id, game_name, ratio, _, rarity_tier)| TopEntry {
            app_id,
            game_name,
            completion_pct: ratio * 100.0,
            rarity_tier,
        })
        .collect()
}

/// Renders the full profile widget: 2-column main area (stats left, closest-to-complete right)
/// plus an optional bottom loader strip.
pub fn profile_widget<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    summary: &ProfileSummary,
    top5: Vec<TopEntry>,
    loader_phase: LoaderPhase,
    loader_hiding_since: Option<Instant>,
    games_count: usize,
) -> Element<'a, crate::Message> {
    let left_col = build_left_column(user_profile, summary, games_count);
    let right_col = build_right_column(top5);

    let two_col_row = row![
        container(left_col)
            .width(Length::FillPortion(3))
            .height(Length::Fill)
            .padding(18)
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_SURFACE)),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            }),
        container(right_col)
            .width(Length::FillPortion(1))
            .height(Length::Fill)
            .padding(16)
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_SURFACE)),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            }),
    ]
    .spacing(16);

    let mut outer = column![two_col_row].spacing(0);

    if let Some(strip) = build_loader_strip(loader_phase, loader_hiding_since) {
        outer = outer.push(strip);
    }

    container(outer)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12))
        .into()
}

fn build_left_column<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    summary: &ProfileSummary,
    games_count: usize,
) -> Element<'a, crate::Message> {
    let header_row = build_profile_header(user_profile, summary, games_count);
    let rarity_bar = build_rarity_bar(summary);
    let rarity_cards = build_rarity_cards(summary);

    column![header_row, rarity_bar, rarity_cards]
        .spacing(12)
        .into()
}

fn build_profile_header<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    summary: &ProfileSummary,
    games_count: usize,
) -> Element<'a, crate::Message> {
    let persona = user_profile
        .map(|p| p.persona_name.as_str())
        .unwrap_or("Steam User");

    let avatar = build_avatar(user_profile, persona);

    let nick_label = text(format!("{{# {persona} }}"))
        .size(15)
        .color(C_TEXT_PRIMARY);

    let level_chip = container(text("Lvl \u{2014}").size(11).color(C_ACCENT))
        .padding(Padding::default().left(6).right(6).top(2).bottom(2))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color {
                a: 0.15,
                ..C_ACCENT
            })),
            border: iced::Border {
                color: Color {
                    a: 0.35,
                    ..C_ACCENT
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    let nick_row = row![nick_label, level_chip]
        .spacing(6)
        .align_y(Alignment::Center);

    let games_label = text(format!("{games_count} games tracked"))
        .size(12)
        .color(C_TEXT_MUTED);

    let left_info = column![nick_row, games_label].spacing(2);

    let earned = summary.earned_total;
    let total = summary.achievement_total;
    let pct = if total > 0 {
        earned as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let earned_text = text(format_thousands(earned))
        .size(24)
        .color(C_TEXT_PRIMARY);
    let total_text = text(format!("/ {}", format_thousands(total)))
        .size(16)
        .color(C_TEXT_DIM);
    let pct_text = text(format!("{pct:.1}% unlocked")).size(12).color(C_ACCENT);

    let counter_row = row![earned_text, total_text]
        .spacing(6)
        .align_y(Alignment::Center);

    let right_info = column![counter_row, pct_text]
        .spacing(4)
        .align_x(Alignment::End);

    row![
        avatar,
        left_info,
        iced::widget::Space::new().width(Length::Fill),
        right_info,
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

const AVATAR_SIZE: f32 = 112.0;

fn build_avatar<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    persona: &'a str,
) -> Element<'a, crate::Message> {
    if let Some(bytes) = user_profile.and_then(|p| p.avatar_png_bytes.as_ref()) {
        let handle = ImageHandle::from_bytes(bytes.clone());
        return container(
            image_widget(handle)
                .width(Length::Fixed(AVATAR_SIZE))
                .height(Length::Fixed(AVATAR_SIZE)),
        )
        .width(Length::Fixed(AVATAR_SIZE))
        .height(Length::Fixed(AVATAR_SIZE))
        .style(|_: &iced::Theme| container::Style {
            border: iced::Border {
                radius: 8.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into();
    }

    build_avatar_initials(persona)
}

fn build_avatar_initials(persona: &str) -> Element<'_, crate::Message> {
    let mut words = persona.split_whitespace();
    let first = words
        .next()
        .and_then(|w| w.chars().next())
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?');
    let second = words
        .next()
        .and_then(|w| w.chars().next())
        .unwrap_or(first)
        .to_uppercase()
        .next()
        .unwrap_or(first);

    let initials = format!("{first}{second}");

    container(
        text(initials)
            .size(36)
            .color(C_APP)
            .align_x(Alignment::Center),
    )
    .width(Length::Fixed(AVATAR_SIZE))
    .height(Length::Fixed(AVATAR_SIZE))
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(C_ACCENT)),
        border: iced::Border {
            radius: 8.0.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn build_rarity_bar(summary: &ProfileSummary) -> Element<'static, crate::Message> {
    let tiers: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];

    let total_unlocked: u32 = tiers.iter().map(|(_, c)| c).sum();

    if total_unlocked == 0 {
        return container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(8.0))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_HOVER)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into();
    }

    let mut bar_row = row![].spacing(1);

    for (tier, count) in &tiers {
        if *count == 0 {
            continue;
        }
        let color = rarity_color(*tier);
        let portion = *count;
        let segment = container(iced::widget::Space::new())
            .width(Length::FillPortion(portion.min(u16::MAX as u32) as u16))
            .height(Length::Fixed(8.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(color)),
                ..container::Style::default()
            });
        bar_row = bar_row.push(segment);
    }

    container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(8.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_HOVER)),
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn build_rarity_cards(summary: &ProfileSummary) -> Element<'static, crate::Message> {
    let tiers: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];

    let mut cards = row![].spacing(6);

    for (tier, count) in tiers {
        cards = cards.push(build_rarity_card(tier, count));
    }

    cards.into()
}

fn build_rarity_card<'a>(tier: RarityTier, count: u32) -> Element<'a, crate::Message> {
    let color = rarity_color(tier);
    let label = rarity_label(tier);
    let count_str = format_thousands(count);

    let stripe = container(iced::widget::Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            ..container::Style::default()
        });

    let number = text(count_str).size(18).color(color);
    let tier_label = text(label).size(10).color(C_TEXT_MUTED);

    let info_col = column![number, tier_label]
        .spacing(4)
        .padding(Padding::default().left(8).right(6).top(6).bottom(6));

    let card_inner = row![stripe, info_col];

    container(card_inner)
        .width(Length::FillPortion(1))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color { a: 0.08, ..color })),
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn build_right_column(top5: Vec<TopEntry>) -> Element<'static, crate::Message> {
    let header = text("CLOSEST TO COMPLETE").size(11).color(C_TEXT_MUTED);

    if top5.is_empty() {
        return column![
            header,
            text("Nothing to recommend yet").size(12).color(C_TEXT_DIM),
        ]
        .spacing(8)
        .into();
    }

    let mut rows_col = column![header].spacing(8);

    for entry in top5 {
        rows_col = rows_col.push(build_closest_row(entry));
    }

    rows_col.into()
}

fn build_closest_row(entry: TopEntry) -> Element<'static, crate::Message> {
    let color = entry.rarity_tier.map(rarity_color).unwrap_or(C_BORDER);

    let initial = entry
        .game_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .next()
        .unwrap_or('?');

    let letter_avatar = container(text(initial.to_string()).size(14).color(color))
        .width(Length::Fixed(32.0))
        .height(Length::Fixed(32.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_HOVER)),
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let game_name_label = text(entry.game_name.clone())
        .size(13)
        .color(C_TEXT_PRIMARY)
        .wrapping(text::Wrapping::None);
    let pct_label = text(format!("{:.0}%", entry.completion_pct))
        .size(12)
        .color(C_ACCENT);

    let info_col = column![
        container(game_name_label)
            .width(Length::Fill)
            .clip(true),
    ]
    .spacing(2)
    .width(Length::Fill);

    let stripe = container(iced::widget::Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            ..container::Style::default()
        });

    let row_content = row![letter_avatar, info_col, pct_label,]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(6).right(6).top(4).bottom(4));

    let inner = row![stripe, row_content];

    let app_id = entry.app_id;
    let row_container = container(inner).style(move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(C_HOVER)),
        border: iced::Border {
            radius: 6.0.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    });

    button(row_container)
        .on_press(crate::Message::OpenGameView(app_id))
        .padding(0)
        .style(|_: &iced::Theme, _status| button::Style {
            background: None,
            ..button::Style::default()
        })
        .into()
}

fn build_loader_strip<'a>(
    phase: LoaderPhase,
    loader_hiding_since: Option<Instant>,
) -> Option<Element<'a, crate::Message>> {
    if phase == LoaderPhase::Gamma {
        let elapsed = loader_hiding_since
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(0);
        if elapsed >= 300 {
            return None;
        }
    }

    let (loaded, total, status_text) = match phase {
        LoaderPhase::Alpha => (0usize, 1usize, Some("Scanning library\u{2026}")),
        LoaderPhase::Beta { loaded, total } => (loaded, total, Some("Loading\u{2026}")),
        LoaderPhase::Gamma => (1, 1, None),
    };

    let frac = if total > 0 {
        loaded as f32 / total as f32
    } else {
        0.0
    };

    let fill_w = (frac.clamp(0.0, 1.0) * 140.0).max(0.0);

    let bar_fill = container(iced::widget::Space::new())
        .width(Length::Fixed(fill_w))
        .height(Length::Fixed(4.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_ACCENT)),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let bar_track = container(bar_fill)
        .width(Length::Fixed(140.0))
        .height(Length::Fixed(4.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_HOVER)),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let count_label = text(format!("{loaded} / {total} games loaded"))
        .size(12)
        .color(C_TEXT_MUTED);

    let mut strip_row = row![bar_track, count_label]
        .spacing(10)
        .align_y(Alignment::Center);

    if let Some(status) = status_text {
        strip_row = strip_row.push(iced::widget::Space::new().width(Length::Fill));
        strip_row = strip_row.push(text(status).size(11).color(C_TEXT_DIM));
    }

    let strip = container(strip_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(10).bottom(10))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                radius: 8.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    Some(container(strip).padding(Padding::default().top(14)).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, GameCacheEntry};
    use crate::profile_view::types::{CapsuleAsset, GameEntry};
    use crate::progress_scan::ProgressData;
    use steamlens_core::GameSummary;

    fn format_short(n: u32) -> String {
        if n >= 1000 {
            format!("{:.1}k", n as f32 / 1000.0)
        } else {
            n.to_string()
        }
    }

    fn compute_level(earned: u32, legendary: u32, mythical: u32) -> u32 {
        ((earned as u64 + 5 * legendary as u64 + 2 * mythical as u64) / 100) as u32
    }

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
            tier_breakdown: Vec::new(),
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
            tier_breakdown: Vec::new(),
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
        assert_eq!(top5[0].app_id, 4, "95% first");
        assert_eq!(top5[1].app_id, 2, "80% second");
        assert_eq!(top5[2].app_id, 6, "70% third");
        assert_eq!(top5[3].app_id, 3, "50% fourth");
        assert_eq!(top5[4].app_id, 5, "30% fifth");
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
        assert_eq!(top5[0].app_id, 2);
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
        assert_eq!(top5[0].app_id, 2);
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
        assert_eq!(top5[0].app_id, 2, "more recent first on tiebreak");
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

    #[test]
    fn top5_returns_top_entry_with_pct() {
        let games = vec![make_entry_with_progress(1, 75, 100)];
        let cached: HashMap<u32, GameCacheEntry> = vec![(1u32, make_cache_entry(1, 75, 100, 0))]
            .into_iter()
            .collect();
        let top5 = top5_closest_to_complete(&games, &cached);
        assert_eq!(top5.len(), 1);
        assert!((top5[0].completion_pct - 75.0).abs() < 0.01);
        assert_eq!(top5[0].game_name, "Game 1");
    }
}
