use std::collections::HashMap;

use iced::widget::{column, container, image as image_widget, row, text};
use iced::{Alignment, Border, Color, Element, Length, Shadow, Vector};

use crate::capsule_cache::CapsuleSize;
use crate::theme::{C_ACCENT, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY};
use crate::ui::widgets::pill::pill;
use crate::ui::widgets::skeleton::{SKEL_DEFAULT_RADIUS, skeleton_box};
use crate::ui::widgets::widget::{
    WidgetSummary, breakdown_row, cards_separator, closest_row, format_thousands, rarity_bar,
    rarity_cards, widget_panel,
};

use super::GameViewMessage;
use super::types::{AchievementRow, RarityTier, compute_tier_map};
use crate::profile_view::types::StoredCapsule;

const CAPSULE_SLOT_W: f32 = 184.0;
const CAPSULE_SLOT_H: f32 = 276.0;
const CAPSULE_RADIUS: f32 = 10.0;
const ACH_ICON_SIZE: f32 = 28.0;

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

#[derive(Debug, Clone)]
pub struct TopUnlockable {
    pub api_name: String,
    pub display_name: String,
    pub global_percent: Option<f32>,
    pub icon_rgba: Option<(u32, u32, Vec<u8>)>,
}

pub fn top5_easiest_to_unlock(achievements: &[AchievementRow]) -> Vec<TopUnlockable> {
    let mut candidates: Vec<TopUnlockable> = achievements
        .iter()
        .filter(|r| !r.data.is_achieved && !r.is_spoiler_hidden())
        .map(|r| TopUnlockable {
            api_name: r.data.id.clone(),
            display_name: if r.data.display_name.is_empty() {
                r.data.id.clone()
            } else {
                r.data.display_name.clone()
            },
            global_percent: r.rarity_percent,
            icon_rgba: r
                .data
                .icon
                .as_ref()
                .map(|i| (i.width, i.height, i.rgba.clone())),
        })
        .collect();

    candidates.sort_by(|a, b| {
        let pa = a.global_percent.unwrap_or(-1.0);
        let pb = b.global_percent.unwrap_or(-1.0);
        pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates.into_iter().take(5).collect()
}

pub struct GameWidgetParams<'a> {
    pub app_id: u32,
    pub game_name: &'a str,
    pub achievements: &'a [AchievementRow],
    pub capsule_handles: &'a HashMap<(u32, CapsuleSize), StoredCapsule>,
    pub skeleton_phase: f32,
    pub hovered_bar_slice: Option<RarityTier>,
}

pub fn game_widget<'a>(params: GameWidgetParams<'a>) -> Element<'a, GameViewMessage> {
    let summary = compute_game_summary(params.achievements);
    let top5 = top5_easiest_to_unlock(params.achievements);

    let capsule_el = build_capsule(params.app_id, params.capsule_handles, params.skeleton_phase);
    let inner_col = build_left_column(
        params.app_id,
        params.game_name,
        &summary,
        params.hovered_bar_slice,
    );
    let left_content: Element<'a, GameViewMessage> = row![capsule_el, inner_col]
        .spacing(16)
        .align_y(Alignment::Center)
        .into();
    let right_col = build_right_column(top5);
    widget_panel(left_content, right_col)
}

const HEADER_BLOCK_H: f32 = 64.0;

fn build_left_column<'a>(
    app_id: u32,
    game_name: &'a str,
    summary: &WidgetSummary,
    hovered_bar_slice: Option<RarityTier>,
) -> Element<'a, GameViewMessage> {
    let header_row = build_game_header(app_id, game_name, summary);
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
    summary: &WidgetSummary,
) -> Element<'a, GameViewMessage> {
    let name = text(game_name.to_owned()).size(15).color(C_TEXT_PRIMARY);
    let appid_pill = pill(
        text(format!("AppID {app_id}")).size(11).color(C_ACCENT),
        C_ACCENT,
    )
    .radius(4.0);

    let name_row = row![name, appid_pill].spacing(6).align_y(Alignment::Center);

    let subtitle = text(format!(
        "{} achievements",
        format_thousands(summary.achievement_total)
    ))
    .size(12)
    .color(C_TEXT_MUTED);

    let info = column![name_row, subtitle].spacing(2);

    container(info)
        .width(Length::Fill)
        .height(Length::Fixed(HEADER_BLOCK_H))
        .align_y(Alignment::Start)
        .into()
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

fn build_right_column<'a>(top5: Vec<TopUnlockable>) -> Element<'a, GameViewMessage> {
    let header = text("CLOSEST TO ACHIEVE").size(11).color(C_TEXT_MUTED);

    if top5.is_empty() {
        return column![
            header,
            text("All achievements unlocked").size(12).color(C_TEXT_DIM),
        ]
        .spacing(8)
        .into();
    }

    let mut rows_col = column![header].spacing(6);
    for entry in top5 {
        rows_col = rows_col.push(build_closest_row(entry));
    }
    rows_col.into()
}

fn build_closest_row<'a>(entry: TopUnlockable) -> Element<'a, GameViewMessage> {
    let icon_el: Element<'a, GameViewMessage> = if let Some((w, h, rgba)) = entry.icon_rgba {
        container(
            image_widget(iced::widget::image::Handle::from_rgba(w, h, rgba))
                .width(Length::Fixed(ACH_ICON_SIZE))
                .height(Length::Fixed(ACH_ICON_SIZE)),
        )
        .width(Length::Fixed(ACH_ICON_SIZE))
        .height(Length::Fixed(ACH_ICON_SIZE))
        .style(|_: &iced::Theme| container::Style {
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
    } else {
        skeleton_box(ACH_ICON_SIZE, ACH_ICON_SIZE, SKEL_DEFAULT_RADIUS, 0.0)
    };

    let secondary = match entry.global_percent {
        Some(p) => format!("{p:.1}% have it"),
        None => "rarity unknown".to_owned(),
    };
    let pct_label = match entry.global_percent {
        Some(p) => format!("{p:.0}%"),
        None => "—".to_owned(),
    };

    closest_row::<GameViewMessage>(
        icon_el,
        entry.display_name,
        secondary,
        pct_label,
        GameViewMessage::SearchChanged(entry.api_name),
    )
}

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

    #[test]
    fn top5_excludes_unlocked() {
        let rows = vec![
            make_ach("a1", true, Some(90.0)),
            make_ach("a2", false, Some(80.0)),
            make_ach("a3", false, Some(50.0)),
            make_ach("a4", false, Some(20.0)),
        ];
        let top = top5_easiest_to_unlock(&rows);
        let names: Vec<&str> = top.iter().map(|t| t.api_name.as_str()).collect();
        assert_eq!(names, vec!["a2", "a3", "a4"]);
    }

    #[test]
    fn top5_orders_by_highest_global_percent_first() {
        let rows = vec![
            make_ach("hard", false, Some(1.0)),
            make_ach("easy", false, Some(95.0)),
            make_ach("mid", false, Some(50.0)),
        ];
        let top = top5_easiest_to_unlock(&rows);
        assert_eq!(top[0].api_name, "easy");
        assert_eq!(top[1].api_name, "mid");
        assert_eq!(top[2].api_name, "hard");
    }
}
