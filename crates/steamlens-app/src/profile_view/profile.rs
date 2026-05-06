use std::collections::HashMap;

use iced::widget::image::Handle as ImageHandle;
use iced::widget::{
    button, column, container, image as image_widget, mouse_area, row, text, tooltip,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::types::{CachedAchievement, GameCacheEntry};
use crate::game_view::types::RarityTier;
use crate::theme::{
    C_ACCENT, C_BORDER, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY,
};

use super::types::{GameEntry, ProfileViewMessage, TopEntry};

pub(crate) const C_RARITY_COMMON: Color = Color::from_rgb(0.314, 0.980, 0.482);
pub(crate) const C_RARITY_UNCOMMON: Color = Color::from_rgb(0.545, 0.914, 0.992);
pub(crate) const C_RARITY_RARE: Color = Color::from_rgb(0.741, 0.576, 0.976);
pub(crate) const C_RARITY_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
pub(crate) const C_RARITY_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

const RARITY_CARD_WIDTH: f32 = 95.0;
const RARITY_CARD_GAP: f32 = 8.0;
const BAR_HEIGHT: f32 = 16.0;
const BAR_RADIUS: f32 = 6.0;

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

fn tick_threshold_color(threshold_pct: u8) -> Color {
    match threshold_pct {
        0 => C_TEXT_MUTED,
        25 => C_RARITY_COMMON,
        50 => C_RARITY_UNCOMMON,
        75 => C_RARITY_MYTHICAL,
        100 => C_RARITY_LEGENDARY,
        _ => C_TEXT_DIM,
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
        if tier_map.is_empty() {
            continue;
        }

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

pub fn format_remaining(remaining: u64) -> String {
    format!("{} achievements remaining", format_thousands_u64(remaining))
}

fn format_thousands_u64(n: u64) -> String {
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

pub fn tick_lit_at(unlocked_pct: f32, threshold: u8) -> bool {
    unlocked_pct >= threshold as f32
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
            let cache = cached_entries.get(&g.app_id);
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
                g.app_id,
                g.name.clone().unwrap_or_default(),
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

pub fn profile_widget<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    summary: &ProfileSummary,
    top5: Vec<TopEntry>,
    games_count: usize,
    skeleton_phase: f32,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, crate::Message> {
    let left_col = build_left_column(
        user_profile,
        avatar_handle,
        summary,
        games_count,
        skeleton_phase,
        hovered_bar_slice,
    );
    let right_col = build_right_column(top5);

    const PROFILE_ROW_HEIGHT: f32 = 320.0;

    let two_col_row = row![
        container(left_col)
            .width(Length::FillPortion(3))
            .height(Length::Fixed(PROFILE_ROW_HEIGHT))
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
            .height(Length::Fixed(PROFILE_ROW_HEIGHT))
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

    container(two_col_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12))
        .into()
}

fn build_left_column<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    summary: &ProfileSummary,
    games_count: usize,
    skeleton_phase: f32,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, crate::Message> {
    let header_row = build_profile_header(
        user_profile,
        avatar_handle,
        summary,
        games_count,
        skeleton_phase,
    );
    let rarity_cards = build_rarity_cards(summary);
    let separator_row = build_cards_separator(summary);
    let breakdown_label = build_breakdown_label();
    let rarity_bar = build_rarity_bar(summary, hovered_bar_slice);

    column![
        header_row,
        breakdown_label,
        rarity_bar,
        rarity_cards,
        separator_row,
    ]
    .spacing(10)
    .into()
}

fn build_profile_header<'a>(
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    summary: &ProfileSummary,
    games_count: usize,
    skeleton_phase: f32,
) -> Element<'a, crate::Message> {
    let persona = user_profile
        .map(|p| p.persona_name.as_str())
        .unwrap_or("Steam User");

    let avatar = build_avatar(avatar_handle, skeleton_phase);

    let nickname = text(persona.to_string()).size(15).color(C_TEXT_PRIMARY);

    let profile_level = container(text("lvl \u{2014}").size(11).color(C_ACCENT))
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

    let nickname_row = row![nickname, profile_level]
        .spacing(6)
        .align_y(Alignment::Center);

    let tracked_games = text(format!("{games_count} games tracked"))
        .size(12)
        .color(C_TEXT_MUTED);

    let info = column![nickname_row, tracked_games].spacing(2);

    let earned = summary.earned_total;
    let total = summary.achievement_total;
    let pct = if total > 0 {
        earned as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let earned_text = text(format_thousands(earned))
        .size(16)
        .color(C_TEXT_PRIMARY);
    let total_text = text(format!("/ {}", format_thousands(total)))
        .size(16)
        .color(C_TEXT_DIM);
    let pct_text = text(format!("{pct:.1}% unlocked")).size(12).color(C_ACCENT);

    let counter_row = row![earned_text, total_text]
        .spacing(6)
        .align_y(Alignment::Center);

    let earnings = column![counter_row, pct_text]
        .spacing(4)
        .align_x(Alignment::End);

    let info_block = container(info)
        .height(Length::Fill)
        .align_y(Alignment::Start);

    let earnings_block = container(earnings)
        .height(Length::Fill)
        .align_y(Alignment::End);

    row![
        avatar,
        info_block,
        iced::widget::Space::new().width(Length::Fill),
        earnings_block,
    ]
    .spacing(14)
    .width(Length::Fill)
    .height(Length::Fixed(AVATAR_SIZE))
    .into()
}

const AVATAR_SIZE: f32 = 100.0;

fn build_avatar<'a>(
    avatar_handle: Option<&'a ImageHandle>,
    skeleton_phase: f32,
) -> Element<'a, crate::Message> {
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
                radius: 8.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into();
    }

    crate::skeleton::skeleton_box(AVATAR_SIZE, AVATAR_SIZE, skeleton_phase)
}

fn build_rarity_cards(summary: &ProfileSummary) -> Element<'static, crate::Message> {
    let tiers: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];

    let total = summary.achievement_total;

    let mut cards = row![].spacing(RARITY_CARD_GAP);

    for (tier, count) in tiers {
        let pct = if total > 0 {
            count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        cards = cards.push(build_count_card(
            rarity_color(tier),
            rarity_label(tier),
            count,
            pct,
        ));
    }

    cards.into()
}

fn build_count_card<'a>(
    accent: Color,
    label: &'static str,
    count: u32,
    pct: f64,
) -> Element<'a, crate::Message> {
    let count_str = format_thousands(count);
    let pct_str = format!("{pct:.1}%");

    let stripe = container(iced::widget::Space::new())
        .width(Length::Fixed(3.0))
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(accent)),
            ..container::Style::default()
        });

    let number = text(count_str).size(18).color(accent);
    let pct_text = text(pct_str).size(10).color(Color { a: 0.75, ..accent });
    let label_text = text(label).size(9).color(C_TEXT_MUTED);

    let info_col = column![number, pct_text, label_text]
        .spacing(2)
        .padding(Padding::default().left(8).right(6).top(6).bottom(6));

    let card_inner = row![stripe, info_col];

    container(card_inner)
        .width(Length::Fixed(RARITY_CARD_WIDTH))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color { a: 0.08, ..accent })),
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn build_cards_separator(summary: &ProfileSummary) -> Element<'static, crate::Message> {
    let total = summary.achievement_total;
    let earned = summary.earned_total;
    let remaining = total.saturating_sub(earned);
    let pct_to_go = if total > 0 {
        remaining as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let remaining_label = text(format_remaining(remaining as u64))
        .size(11)
        .color(C_TEXT_MUTED);
    let pct_label = text(format!("{pct_to_go:.1}% to go"))
        .size(11)
        .color(C_TEXT_DIM);

    let label_row = row![
        remaining_label,
        iced::widget::Space::new().width(Length::Fill),
        pct_label,
    ]
    .align_y(Alignment::Center);

    let separator = iced::widget::rule::horizontal(1);

    column![separator, label_row].spacing(6).into()
}

fn build_breakdown_label() -> Element<'static, crate::Message> {
    text("ACHIEVEMENTS BREAKDOWN")
        .size(10)
        .color(C_TEXT_MUTED)
        .into()
}

fn build_rarity_bar<'a>(
    summary: &ProfileSummary,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, crate::Message> {
    const TIER_ORDER: [RarityTier; 5] = [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Mythical,
        RarityTier::Legendary,
    ];

    let tier_counts: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];

    let total_unlocked: u32 = tier_counts.iter().map(|(_, c)| c).sum();
    let total = summary.achievement_total;

    if total == 0 {
        return container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(BAR_HEIGHT))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_HOVER)),
                border: iced::Border {
                    radius: BAR_RADIUS.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into();
    }

    let unlocked_pct = total_unlocked as f32 / total as f32 * 100.0;

    let tick_thresholds: [(u8, Color); 5] = [
        (0, tick_threshold_color(0)),
        (25, tick_threshold_color(25)),
        (50, tick_threshold_color(50)),
        (75, tick_threshold_color(75)),
        (100, tick_threshold_color(100)),
    ];

    let mut bar_row: iced::widget::Row<'a, crate::Message> = row![].spacing(0);
    let mut first_segment = true;
    let mut last_tier_idx: Option<usize> = None;

    let active_tier_count = tier_counts.iter().filter(|(_, c)| *c > 0).count();

    for (seg_idx, tier) in TIER_ORDER.iter().enumerate() {
        let count = tier_counts
            .iter()
            .find(|(t, _)| t == tier)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if count > 0 {
            last_tier_idx = Some(seg_idx);
        }
    }

    for (seg_idx, tier) in TIER_ORDER.iter().enumerate() {
        let count = tier_counts
            .iter()
            .find(|(t, _)| t == tier)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        if count == 0 {
            continue;
        }

        let color = rarity_color(*tier);
        let portion = count as u16;
        let is_hovered = hovered_bar_slice == Some(*tier);
        let is_first = first_segment;
        let is_last = last_tier_idx == Some(seg_idx);
        let _ = active_tier_count;

        let effective_color = if is_hovered {
            Color {
                r: (color.r * 1.25).min(1.0),
                g: (color.g * 1.25).min(1.0),
                b: (color.b * 1.25).min(1.0),
                a: 1.0,
            }
        } else {
            color
        };

        let radius = iced::border::Radius {
            top_left: if is_first { BAR_RADIUS } else { 0.0 },
            bottom_left: if is_first { BAR_RADIUS } else { 0.0 },
            top_right: if is_last { BAR_RADIUS } else { 0.0 },
            bottom_right: if is_last { BAR_RADIUS } else { 0.0 },
        };

        let tier_copy = *tier;
        let count_for_tooltip = count;
        let total_for_tooltip = total;

        let slice = container(iced::widget::Space::new())
            .width(Length::FillPortion(portion))
            .height(Length::Fixed(BAR_HEIGHT))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(effective_color)),
                border: iced::Border {
                    radius,
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let slice_pct = if total_for_tooltip > 0 {
            count_for_tooltip as f64 / total_for_tooltip as f64 * 100.0
        } else {
            0.0
        };
        let tooltip_text = format!(
            "{} {} \u{00B7} {:.1}%",
            format_thousands(count_for_tooltip),
            rarity_label(tier_copy),
            slice_pct,
        );

        let tooltip_content = container(text(tooltip_text).size(11).color(C_TEXT_PRIMARY))
            .padding(Padding::default().left(8).right(8).top(4).bottom(4))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.10, 0.09, 0.14, 0.95,
                ))),
                border: iced::Border {
                    color: Color { a: 0.5, ..C_ACCENT },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            });

        let hoverable_slice = mouse_area(tooltip(slice, tooltip_content, tooltip::Position::Top))
            .on_enter(crate::Message::ProfileView(
                ProfileViewMessage::BarSliceHoverEnter(tier_copy),
            ))
            .on_exit(crate::Message::ProfileView(
                ProfileViewMessage::BarSliceHoverExit,
            ));

        bar_row = bar_row.push(hoverable_slice);
        first_segment = false;
    }

    let unrated_earned = summary_unrated_earned(
        tier_counts[0].1,
        tier_counts[1].1,
        tier_counts[2].1,
        tier_counts[3].1,
        tier_counts[4].1,
        total_unlocked,
    );
    if unrated_earned > 0 {
        let unrated_segment = container(iced::widget::Space::new())
            .width(Length::FillPortion(
                unrated_earned.min(u16::MAX as u32) as u16
            ))
            .height(Length::Fixed(BAR_HEIGHT))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_ACCENT)),
                ..container::Style::default()
            });
        bar_row = bar_row.push(unrated_segment);
    }

    let locked = total.saturating_sub(total_unlocked);
    if locked > 0 {
        let locked_segment = container(iced::widget::Space::new())
            .width(Length::FillPortion(locked.min(u16::MAX as u32) as u16))
            .height(Length::Fixed(BAR_HEIGHT))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_HOVER)),
                border: iced::Border {
                    radius: iced::border::Radius {
                        top_right: BAR_RADIUS,
                        bottom_right: BAR_RADIUS,
                        ..iced::border::Radius::default()
                    },
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
        bar_row = bar_row.push(locked_segment);
    }

    let bar_track = container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(BAR_HEIGHT))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_HOVER)),
            border: iced::Border {
                radius: BAR_RADIUS.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let ticks_layer = build_tick_marks(tick_thresholds, unlocked_pct);
    column![bar_track, ticks_layer].spacing(4).into()
}

fn summary_unrated_earned(
    common: u32,
    uncommon: u32,
    rare: u32,
    mythical: u32,
    legendary: u32,
    total_unlocked: u32,
) -> u32 {
    let rated_earned = common + uncommon + rare + mythical + legendary;
    total_unlocked.saturating_sub(rated_earned)
}

fn build_tick_marks(
    tick_thresholds: [(u8, Color); 5],
    unlocked_pct: f32,
) -> Element<'static, crate::Message> {
    let mut ticks_row: iced::widget::Row<'static, crate::Message> = row![].spacing(0);

    for (i, (threshold, color)) in tick_thresholds.iter().enumerate() {
        let lit = tick_lit_at(unlocked_pct, *threshold);
        let tick_color = if lit { *color } else { C_TEXT_MUTED };

        let dot = container(iced::widget::Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(tick_color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let label = text(format!("{threshold}%")).size(14).color(tick_color);

        let tick_unit = row![dot, label].spacing(3).align_y(Alignment::Center);

        let tick_pct = *threshold as f32;
        let fill_before = if i == 0 {
            tick_pct - 0.5
        } else {
            tick_pct - tick_thresholds[i - 1].0 as f32 - 0.5
        };
        let fill_before = fill_before.max(0.0) as u16;

        if fill_before > 0 {
            ticks_row = ticks_row.push(
                iced::widget::Space::new()
                    .width(Length::FillPortion(fill_before))
                    .height(Length::Fixed(20.0)),
            );
        }

        ticks_row = ticks_row.push(tick_unit);
    }

    ticks_row
        .width(Length::Fill)
        .height(Length::Fixed(20.0))
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

    let info_col = column![container(game_name_label).width(Length::Fill).clip(true),]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::types::{CachedAchievement, CachedProgress, GameCacheEntry};
    use crate::profile_view::types::{CapsuleAsset, GameEntry};
    use crate::progress_scan::ProgressData;
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

    fn make_entry_with_progress(app_id: u32, earned: u32, total: u32) -> GameEntry {
        GameEntry {
            app_id,
            change_number: 0,
            last_played: None,
            name: Some(format!("Game {app_id}")),
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
            genre: None,
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
            genre: None,
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
                app_id: 1,
                change_number: 0,
                last_played: None,
                name: None,
                capsule: CapsuleAsset::Unavailable,
                progress: None,
            },
            make_entry_with_progress(2, 10, 50),
        ];
        let visible: Vec<&GameEntry> = games.iter().filter(|g| g.progress.is_some()).collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].app_id, 2);
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

    #[test]
    fn tick_lit_at_zero_pct_always_dim() {
        assert!(!tick_lit_at(0.0, 25), "0% should not light 25% mark");
        assert!(!tick_lit_at(0.0, 50), "0% should not light 50% mark");
        assert!(!tick_lit_at(0.0, 75), "0% should not light 75% mark");
        assert!(!tick_lit_at(0.0, 100), "0% should not light 100% mark");
    }

    #[test]
    fn tick_lit_at_crosses_threshold() {
        assert!(tick_lit_at(25.0, 25), "exactly 25% lights 25% mark");
        assert!(tick_lit_at(50.0, 50), "exactly 50% lights 50% mark");
        assert!(tick_lit_at(75.0, 75), "exactly 75% lights 75% mark");
        assert!(tick_lit_at(100.0, 100), "exactly 100% lights 100% mark");
    }

    #[test]
    fn tick_lit_below_threshold_stays_dim() {
        assert!(!tick_lit_at(24.9, 25), "24.9% should not light 25% mark");
        assert!(!tick_lit_at(49.9, 50), "49.9% should not light 50% mark");
        assert!(!tick_lit_at(74.9, 75), "74.9% should not light 75% mark");
        assert!(!tick_lit_at(99.9, 100), "99.9% should not light 100% mark");
    }

    #[test]
    fn tick_lit_at_above_threshold_stays_lit() {
        assert!(tick_lit_at(30.0, 25), "30% keeps 25% lit");
        assert!(tick_lit_at(80.0, 75), "80% keeps 75% lit");
    }

    #[test]
    fn format_remaining_uses_commas() {
        assert_eq!(format_remaining(21982), "21,982 achievements remaining");
        assert_eq!(format_remaining(0), "0 achievements remaining");
        assert_eq!(
            format_remaining(1000000),
            "1,000,000 achievements remaining"
        );
    }
}
