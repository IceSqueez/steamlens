use std::collections::HashMap;

use iced::widget::image::Handle as ImageHandle;
use iced::widget::{button, column, container, image as image_widget, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::types::{CachedAchievement, GameCacheEntry};
use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::theme::{C_ACCENT, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY};
use crate::ui::theme::{AppTheme, palette};
use crate::ui::widgets::bar::{BarSegment, segmented_bar};
use crate::ui::widgets::pill::pill;

use super::types::{GameEntry, ProfileViewMessage, StoredCapsule, TopEntry};

pub(crate) const C_RARITY_COMMON: Color = Color::from_rgb(0.314, 0.980, 0.482);
pub(crate) const C_RARITY_UNCOMMON: Color = Color::from_rgb(0.545, 0.914, 0.992);
pub(crate) const C_RARITY_RARE: Color = Color::from_rgb(0.741, 0.576, 0.976);
pub(crate) const C_RARITY_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
pub(crate) const C_RARITY_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

const RARITY_CARD_WIDTH: f32 = 95.0;
const RARITY_CARD_GAP: f32 = 16.0;
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

fn overall_tier_color(unlocked_pct: f32) -> Color {
    if unlocked_pct >= 100.0 {
        C_RARITY_LEGENDARY
    } else if unlocked_pct >= 75.0 {
        C_RARITY_MYTHICAL
    } else if unlocked_pct >= 50.0 {
        C_RARITY_RARE
    } else if unlocked_pct >= 25.0 {
        C_RARITY_UNCOMMON
    } else if unlocked_pct > 0.0 {
        C_RARITY_COMMON
    } else {
        C_TEXT_MUTED
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

        if !entry.tier_breakdown.is_empty() {
            for (tier, count) in &entry.tier_breakdown {
                match tier {
                    RarityTier::Legendary => legendary_count += count,
                    RarityTier::Mythical => mythical_count += count,
                    RarityTier::Rare => rare_count += count,
                    RarityTier::Uncommon => uncommon_count += count,
                    RarityTier::Common => common_count += count,
                }
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
    unlocked_pct > 0.0 && unlocked_pct >= threshold as f32
}

pub fn top5_closest_to_complete(
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
        .take(5)
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
    pub summary: ProfileSummary,
    pub top5: Vec<TopEntry>,
    pub games_count: usize,
    pub skeleton_phase: f32,
    pub hovered_bar_slice: Option<RarityTier>,
    pub capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub capsule_size: CapsuleSize,
    pub steam_level: Option<u32>,
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
    );
    let right_col = build_right_column(
        params.top5,
        params.capsule_handles,
        params.capsule_size,
        params.skeleton_phase,
    );

    const PROFILE_ROW_HEIGHT: f32 = 320.0;

    let two_col_row = row![
        container(left_col)
            .width(Length::FillPortion(5))
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
            .width(Length::FillPortion(2))
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
    steam_level: Option<u32>,
) -> Element<'a, ProfileViewMessage> {
    let header_row = build_profile_header(
        user_profile,
        avatar_handle,
        summary,
        games_count,
        skeleton_phase,
        steam_level,
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
    steam_level: Option<u32>,
) -> Element<'a, ProfileViewMessage> {
    let persona = user_profile
        .map(|p| p.persona_name.as_str())
        .unwrap_or("Steam User");

    let avatar = build_avatar(avatar_handle, skeleton_phase);

    let nickname = text(persona.to_string()).size(15).color(C_TEXT_PRIMARY);

    let level_str = match steam_level {
        Some(n) => format!("level ({n})"),
        None => "level (X)".to_owned(),
    };
    let profile_level = pill(text(level_str).size(11).color(C_ACCENT), C_ACCENT).radius(4.0);

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
const AVATAR_RADIUS: f32 = 8.0;

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

    crate::ui::widgets::skeleton::skeleton_box(
        AVATAR_SIZE,
        AVATAR_SIZE,
        AVATAR_RADIUS,
        skeleton_phase,
    )
}

fn build_rarity_cards(summary: &ProfileSummary) -> Element<'static, ProfileViewMessage> {
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
) -> Element<'a, ProfileViewMessage> {
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
    let label_text = text(label).size(11).color(C_TEXT_MUTED);

    let info_col = column![number, label_text, pct_text]
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

fn build_cards_separator(summary: &ProfileSummary) -> Element<'static, ProfileViewMessage> {
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

fn build_breakdown_label() -> Element<'static, ProfileViewMessage> {
    text("ACHIEVEMENTS BREAKDOWN")
        .size(10)
        .color(C_TEXT_MUTED)
        .into()
}

fn build_rarity_bar<'a>(
    summary: &ProfileSummary,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, ProfileViewMessage> {
    let tier_counts: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];

    let total_unlocked: u32 = tier_counts.iter().map(|(_, c)| c).sum();
    let total = summary.achievement_total;
    let tick_thresholds: [u8; 5] = [0, 25, 50, 75, 100];
    let unlocked_pct = if total > 0 {
        total_unlocked as f32 / total as f32 * 100.0
    } else {
        0.0
    };

    let mut segments: Vec<BarSegment> = Vec::new();
    let mut tier_at: Vec<Option<RarityTier>> = Vec::new();
    let mut tooltips: Vec<String> = Vec::new();
    let total_for_pct = total.max(1);

    for (tier, count) in tier_counts.iter() {
        if *count == 0 {
            continue;
        }
        let pct = *count as f64 / total_for_pct as f64 * 100.0;
        segments.push(BarSegment {
            weight: *count,
            color: rarity_color(*tier),
        });
        tier_at.push(Some(*tier));
        tooltips.push(format!(
            "{} {} \u{00B7} {:.1}%",
            format_thousands(*count),
            rarity_label(*tier),
            pct
        ));
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
        let pct = unrated_earned as f64 / total_for_pct as f64 * 100.0;
        segments.push(BarSegment {
            weight: unrated_earned,
            color: C_ACCENT,
        });
        tier_at.push(None);
        tooltips.push(format!(
            "{} Unrated \u{00B7} {:.1}%",
            format_thousands(unrated_earned),
            pct
        ));
    }

    let locked = total.saturating_sub(total_unlocked);
    if locked > 0 {
        let pct = locked as f64 / total_for_pct as f64 * 100.0;
        segments.push(BarSegment {
            weight: locked,
            color: palette(AppTheme::Dark).hover,
        });
        tier_at.push(None);
        tooltips.push(format!(
            "{} Locked \u{00B7} {:.1}%",
            format_thousands(locked),
            pct
        ));
    }

    let hovered_idx = hovered_bar_slice.and_then(|t| tier_at.iter().position(|x| *x == Some(t)));

    let tier_lookup = tier_at.clone();
    let tip_lookup = tooltips.clone();

    let bar: Element<'a, ProfileViewMessage> = segmented_bar(segments, Length::Fill, BAR_HEIGHT)
        .theme(AppTheme::Dark)
        .radius(BAR_RADIUS)
        .hovered(hovered_idx)
        .on_hover(
            move |idx| match idx.and_then(|i| tier_lookup.get(i).copied().flatten()) {
                Some(tier) => ProfileViewMessage::BarSliceHoverEnter(tier),
                None => ProfileViewMessage::BarSliceHoverExit,
            },
        )
        .tooltip(move |idx| tip_lookup.get(idx).cloned().unwrap_or_default())
        .into();

    let ticks_layer = build_tick_marks(tick_thresholds, unlocked_pct);
    column![bar, ticks_layer].spacing(4).into()
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
    tick_thresholds: [u8; 5],
    unlocked_pct: f32,
) -> Element<'static, ProfileViewMessage> {
    let mut ticks_row: iced::widget::Row<'static, ProfileViewMessage> = row![].spacing(0);
    let lit_color = overall_tier_color(unlocked_pct);

    for (i, threshold) in tick_thresholds.iter().enumerate() {
        let lit = tick_lit_at(unlocked_pct, *threshold);
        let tick_color = if lit { lit_color } else { C_TEXT_MUTED };

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
            tick_pct - tick_thresholds[i - 1] as f32 - 0.5
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

fn build_right_column<'a>(
    top5: Vec<TopEntry>,
    capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    capsule_size: CapsuleSize,
    skeleton_phase: f32,
) -> Element<'a, ProfileViewMessage> {
    let header = text("CLOSEST TO 100%").size(11).color(C_TEXT_MUTED);

    if top5.is_empty() {
        return column![
            header,
            text("Nothing to recommend yet").size(12).color(C_TEXT_DIM),
        ]
        .spacing(8)
        .into();
    }

    let mut rows_col = column![header].spacing(6);

    for entry in top5 {
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
    entry: TopEntry,
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
            crate::ui::widgets::skeleton::skeleton_box(
                CAPSULE_W,
                CAPSULE_H,
                crate::ui::widgets::skeleton::SKEL_DEFAULT_RADIUS,
                skeleton_phase,
            )
        };

    let game_name_label = text(entry.game_name.clone())
        .size(12)
        .color(C_TEXT_PRIMARY)
        .wrapping(text::Wrapping::None);

    let remaining = entry.total.saturating_sub(entry.earned);
    let left_label = text(format!("{} of {} left", remaining, entry.total))
        .size(11)
        .color(C_TEXT_MUTED);

    let info_col = column![
        container(game_name_label).width(Length::Fill).clip(true),
        left_label,
    ]
    .spacing(1)
    .width(Length::Fill);

    let pct_label = text(format!("{:.0}%", entry.completion_pct))
        .size(13)
        .color(C_ACCENT);

    let row_content = row![capsule_el, info_col, pct_label,]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(6).right(6).top(5).bottom(5));

    let app_id = entry.app_id;
    let row_container = container(row_content).style(|_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(C_HOVER)),
        border: iced::Border {
            radius: 6.0.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    });

    button(row_container)
        .on_press(ProfileViewMessage::RequestOpenGame(app_id))
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
                genre: None,
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
        assert_eq!(top5[0].earned, 75);
        assert_eq!(top5[0].total, 100);
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
