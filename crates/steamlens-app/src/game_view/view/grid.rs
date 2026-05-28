use iced::widget::{button, column, container, responsive, row, scrollable, space, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::card::achievement_card_widget;
use crate::game_view::types::{AchievementRow, BulkOp, compute_tier_map, visible_achievement_ids};
use crate::game_view::{GameViewMessage, GameViewState};
use crate::ui::grid::compute_grid;
use crate::ui::theme::{palette, theme_from_iced};
use crate::ui::widgets::skeleton::{SKEL_DEFAULT_RADIUS, skeleton_box};

pub(super) const ACH_CARD_GAP: f32 = 12.0;
pub(super) const ACH_MIN_GAP: f32 = 12.0;
pub(super) const ACH_CARD_WIDTH: f32 = 260.0;
pub(super) const ACH_CARD_TEXT_COL_SPACING: u32 = 2;
pub(super) const ACH_CARD_ICON: f32 = 64.0;
pub(super) const ACH_CARD_HEIGHT: f32 = 140.0;
pub(super) const ACH_CARD_TITLE_TEXT_SIZE: f32 = 13.0;
pub(super) const ACH_CARD_DESCRIPTION_TEXT_SIZE: f32 = 11.0;
pub(super) const SKEL_ACH_CARD_STATUS_PILL_WIDTH: f32 = 80.0;
pub(super) const SKEL_ACH_CARD_RARITY_PILL_WIDTH: f32 = 60.0;
pub(super) const SKEL_ACH_CARD_PILL_HEIGHT: f32 = 18.0;

pub(super) fn achievements_tab<'a>(
    state: &'a GameViewState,
    skeleton_phase: f32,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'a, GameViewMessage> {
    let mut col = column![].spacing(0).height(Length::Fill);
    col = col.push(achievement_list(state, skeleton_phase, app_theme));
    col.into()
}

pub(super) fn bulk_action_buttons<'a>() -> Element<'a, GameViewMessage> {
    let mk_outlined =
        |lbl: &'static str, use_accent: bool, m: GameViewMessage| -> Element<'_, GameViewMessage> {
            button(
                text(lbl)
                    .size(12)
                    .style(move |t: &iced::Theme| iced::widget::text::Style {
                        color: Some(if use_accent {
                            palette(theme_from_iced(t)).accent
                        } else {
                            palette(theme_from_iced(t)).text_muted
                        }),
                    }),
            )
            .on_press(m)
            .padding(Padding::default().left(12).right(12).top(6).bottom(6))
            .style(move |t: &iced::Theme, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                let p = palette(theme_from_iced(t));
                button::Style {
                    background: if hovered {
                        Some(iced::Background::Color(p.hover))
                    } else {
                        None
                    },
                    border: iced::Border {
                        color: p.border,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    text_color: if use_accent { p.accent } else { p.text_muted },
                    ..button::Style::default()
                }
            })
            .into()
        };

    let unlock_btn = mk_outlined(
        "Unlock all",
        true,
        GameViewMessage::BulkAction(BulkOp::Unlock),
    );
    let lock_btn = mk_outlined("Lock all", false, GameViewMessage::BulkAction(BulkOp::Lock));
    let invert_btn = mk_outlined("Invert", false, GameViewMessage::BulkAction(BulkOp::Invert));

    row![unlock_btn, lock_btn, invert_btn]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

pub(super) fn achievement_list(
    state: &GameViewState,
    skeleton_phase: f32,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'_, GameViewMessage> {
    if state.achievements.is_empty() {
        return build_skeleton_ach_grid(state, skeleton_phase);
    }

    let visible_ids = visible_achievement_ids(
        &state.achievements,
        state.filter,
        &state.search_query,
        state.achievement_sort,
        &state.rarity_tier_set,
        state.include_hidden,
    );

    let by_id: std::collections::HashMap<&str, &AchievementRow> = state
        .achievements
        .iter()
        .map(|r| (r.data.id.as_str(), r))
        .collect();

    let cards: Vec<&AchievementRow> = visible_ids
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect();

    let query_owned = state.search_query.clone();
    let glow_pulse = (state.rare_glow_phase.sin() + 1.0) * 0.5;
    let tier_map = compute_tier_map(&state.achievements);
    let cache_only = state.cache_only;

    let grid = responsive(move |size| {
        let (cols, gap) = compute_grid(size.width, ACH_CARD_WIDTH, ACH_MIN_GAP);

        let mut rows_col: iced::widget::Column<'_, GameViewMessage> = column![]
            .spacing(ACH_CARD_GAP as u32)
            .padding(Padding::default().top(8).bottom(4));

        for chunk in cards.chunks(cols) {
            let mut r: iced::widget::Row<'_, GameViewMessage> =
                row![space().width(Length::Fixed(gap))].align_y(Alignment::Start);
            for entry in chunk {
                let tier = tier_map.get(&entry.data.id).copied();
                let is_ready = entry.is_spoiler_hidden()
                    || cache_only
                    || (entry.data.icon.is_some() && entry.rarity_percent.is_some());
                let card: Element<'_, GameViewMessage> = if is_ready {
                    achievement_card_widget(
                        entry,
                        ACH_CARD_WIDTH,
                        query_owned.clone(),
                        glow_pulse,
                        tier,
                        skeleton_phase,
                        app_theme,
                    )
                } else {
                    build_skeleton_ach_card(ACH_CARD_WIDTH, skeleton_phase)
                };
                r = r.push(card);
                r = r.push(space().width(Length::Fixed(gap)));
            }
            let needed = cols - chunk.len();
            for _ in 0..needed {
                r = r.push(space().width(Length::Fixed(ACH_CARD_WIDTH)));
                r = r.push(space().width(Length::Fixed(gap)));
            }
            rows_col = rows_col.push(r);
        }

        scrollable(rows_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    });

    container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub(super) fn build_skeleton_ach_grid(
    state: &GameViewState,
    skeleton_phase: f32,
) -> Element<'_, GameViewMessage> {
    let count = state
        .achievements
        .len()
        .max(state.expected_total as usize)
        .max(6);

    let grid = responsive(move |size| {
        let (cols, gap) = compute_grid(size.width, ACH_CARD_WIDTH, ACH_MIN_GAP);

        let mut rows_col: iced::widget::Column<'_, GameViewMessage> = column![]
            .spacing(ACH_CARD_GAP as u32)
            .padding(Padding::default().top(8).bottom(4));

        let total_cards = count;
        let mut rendered = 0;

        while rendered < total_cards {
            let chunk_size = (total_cards - rendered).min(cols);
            let mut r: iced::widget::Row<'_, GameViewMessage> =
                row![space().width(Length::Fixed(gap))].align_y(Alignment::Start);
            for _ in 0..chunk_size {
                r = r.push(build_skeleton_ach_card(ACH_CARD_WIDTH, skeleton_phase));
                r = r.push(space().width(Length::Fixed(gap)));
            }
            let needed = cols - chunk_size;
            for _ in 0..needed {
                r = r.push(space().width(Length::Fixed(ACH_CARD_WIDTH)));
                r = r.push(space().width(Length::Fixed(gap)));
            }
            rows_col = rows_col.push(r);
            rendered += chunk_size;
        }

        scrollable(rows_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    });

    container(grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub(super) fn build_skeleton_ach_card(
    card_w: f32,
    phase: f32,
) -> Element<'static, GameViewMessage> {
    let icon = skeleton_box(ACH_CARD_ICON, ACH_CARD_ICON, SKEL_DEFAULT_RADIUS, phase);

    let title_w = card_w * 0.60;
    let desc_w = card_w * 0.80;

    let text_col = column![
        skeleton_box(
            title_w,
            ACH_CARD_TITLE_TEXT_SIZE,
            SKEL_DEFAULT_RADIUS,
            phase
        ),
        skeleton_box(
            desc_w,
            ACH_CARD_DESCRIPTION_TEXT_SIZE,
            SKEL_DEFAULT_RADIUS,
            phase
        ),
    ]
    .spacing(ACH_CARD_TEXT_COL_SPACING);

    let top_row = row![icon, text_col]
        .spacing(8)
        .align_y(Alignment::Start)
        .padding(Padding::from([8u16, 8]));

    let pill1 = skeleton_box(
        SKEL_ACH_CARD_STATUS_PILL_WIDTH,
        SKEL_ACH_CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
    );
    let pill2 = skeleton_box(
        SKEL_ACH_CARD_RARITY_PILL_WIDTH,
        SKEL_ACH_CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
    );

    let bottom_row = container(
        row![pill1, space().width(Length::Fill), pill2]
            .align_y(Alignment::Center)
            .padding(Padding::default().right(8).bottom(8)),
    )
    .width(Length::Fill);

    let separator = container(iced::widget::rule::horizontal(1))
        .padding(Padding::default().left(8).right(8).top(8).bottom(0))
        .width(Length::Fixed(card_w));

    let card_body = column![
        top_row,
        iced::widget::Space::new().height(Length::Fixed(10.0)),
        separator,
        iced::widget::Space::new().height(Length::Fill),
        bottom_row,
    ]
    .spacing(0);

    let card_container = container(card_body)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(ACH_CARD_HEIGHT))
        .style(|t: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(palette(theme_from_iced(t)).border)),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    card_container.into()
}
