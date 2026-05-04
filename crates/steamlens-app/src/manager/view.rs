use iced::widget::{
    button, column, container, image, mouse_area, opaque, pick_list, responsive, row, scrollable,
    slider, space, stack, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Padding};

use super::types::{
    AchievementFilter, AchievementRow, ActiveTab, BannerKind, BulkOp, ResetScope, StatRow,
    visible_achievement_ids,
};
use super::{ManagerMessage, ManagerPhase, ManagerState};
use crate::Message;

const C_BG: Color = Color::from_rgb(0.157, 0.165, 0.212);
const C_CURRENT_LINE: Color = Color::from_rgb(0.267, 0.278, 0.353);
const C_FG: Color = Color::from_rgb(0.973, 0.973, 0.949);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_GREEN: Color = Color::from_rgb(0.314, 0.980, 0.482);
const C_ORANGE: Color = Color::from_rgb(1.0, 0.722, 0.424);
const C_PURPLE: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_RED: Color = Color::from_rgb(1.0, 0.333, 0.333);
const C_YELLOW: Color = Color::from_rgb(0.945, 0.980, 0.549);
#[allow(dead_code)]
const C_CYAN: Color = Color::from_rgb(0.545, 0.914, 0.992);
fn msg(m: ManagerMessage) -> Message {
    Message::Manager(m)
}

fn dracula_border_radius(r: f32) -> iced::Border {
    iced::Border {
        radius: r.into(),
        ..iced::Border::default()
    }
}

pub fn render(state: &ManagerState) -> Element<'_, Message> {
    match state.phase {
        ManagerPhase::Connecting | ManagerPhase::WaitingStats | ManagerPhase::LoadingData => {
            loading_view(state)
        }
        ManagerPhase::Saving | ManagerPhase::Resetting => {
            let base = loaded_view(state);
            let label = if state.phase == ManagerPhase::Saving {
                "Saving changes..."
            } else {
                "Resetting..."
            };
            stack![base, opaque(saving_overlay(state.spinner_angle, label))].into()
        }
        ManagerPhase::Ready => {
            let base = loaded_view(state);
            if state.show_reset_modal {
                stack![base, opaque(reset_modal(state))].into()
            } else {
                base
            }
        }
        ManagerPhase::Error => error_view(state),
    }
}

fn loading_view(state: &ManagerState) -> Element<'_, Message> {
    let phase_label = match state.phase {
        ManagerPhase::Connecting => "Connecting to Steam...",
        ManagerPhase::WaitingStats => "Requesting stats from Steam...",
        ManagerPhase::LoadingData => "Loading achievements...",
        _ => "Loading...",
    };

    let content = column![
        text(&state.game_name).size(20).color(C_FG),
        text(format!("App ID: {}", state.app_id))
            .size(13)
            .color(C_MUTED),
        text(spinner_frame(state.spinner_angle))
            .size(24)
            .color(C_PURPLE),
        text(phase_label).size(14).color(C_MUTED),
        button(text("Cancel").size(13))
            .on_press(Message::GoBack)
            .padding(Padding::from([8, 16])),
    ]
    .spacing(16)
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_view(state: &ManagerState) -> Element<'_, Message> {
    let content = column![
        text("Failed to load").size(20).color(C_RED),
        text(&state.error_message).size(13).color(C_MUTED),
        button(text("Back").size(13))
            .on_press(Message::GoBack)
            .padding(Padding::from([8, 16])),
    ]
    .spacing(16)
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn loaded_view(state: &ManagerState) -> Element<'_, Message> {
    let hdr = header_bar(state);
    let tabs = tab_bar_widget(state);
    let body = match state.active_tab {
        ActiveTab::Achievements => achievements_tab(state),
        ActiveTab::Stats => stats_tab(state),
    };
    let footer = footer_bar(state);

    let mut col = column![hdr, tabs];
    if let Some(b) = &state.banner {
        col = col.push(banner_widget(b));
    }
    col = col.push(body).push(footer);
    col.spacing(0).into()
}

fn header_bar(state: &ManagerState) -> Element<'_, Message> {
    let back_btn = button(text("\u{2190} Back").size(13).color(C_PURPLE))
        .on_press(Message::GoBack)
        .padding(Padding::from([8u16, 0]))
        .style(|_theme, _status| button::Style {
            background: None,
            ..button::Style::default()
        });

    let title = text(&state.game_name).size(20).color(C_FG);

    let reload_btn = button(text("\u{27F3} Reload").size(13))
        .on_press(msg(ManagerMessage::ReloadRequested))
        .padding(Padding::from([6u16, 12]));

    let right_group = row![title, reload_btn]
        .spacing(16)
        .align_y(Alignment::Center);

    let header_row = row![back_btn, space().width(Length::Fill), right_group]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding(Padding::from([16u16, 16]));

    container(header_row)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            ..container::Style::default()
        })
        .into()
}

fn tab_bar_widget(state: &ManagerState) -> Element<'_, Message> {
    let unlocked = state
        .achievements
        .iter()
        .filter(|r| r.effective_achieved())
        .count();
    let total = state.achievements.len();

    let ach_label = if total > 0 {
        format!("Achievements  {unlocked}/{total}")
    } else {
        "Achievements".to_owned()
    };
    let stats_label = if state.stats.is_empty() {
        "Stats".to_owned()
    } else {
        format!("Stats  {}", state.stats.len())
    };

    let ach_active = state.active_tab == ActiveTab::Achievements;
    let ach_btn = tab_btn(
        ach_label,
        ach_active,
        msg(ManagerMessage::TabChanged(ActiveTab::Achievements)),
    );
    let stats_btn = tab_btn(
        stats_label,
        !ach_active,
        msg(ManagerMessage::TabChanged(ActiveTab::Stats)),
    );

    let tabs_row = row![ach_btn, stats_btn]
        .spacing(4)
        .padding(Padding::from([8u16, 16]));

    container(tabs_row)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_BG)),
            ..container::Style::default()
        })
        .into()
}

fn tab_btn(label: String, active: bool, on_press: Message) -> Element<'static, Message> {
    let color = if active { C_PURPLE } else { C_MUTED };
    button(text(label).size(14).color(color))
        .on_press(on_press)
        .padding(Padding::from([6u16, 12]))
        .style(move |_theme, _status| button::Style {
            background: if active {
                Some(iced::Background::Color(C_CURRENT_LINE))
            } else {
                None
            },
            border: dracula_border_radius(4.0),
            ..button::Style::default()
        })
        .into()
}

fn banner_widget(banner: &super::types::Banner) -> Element<'_, Message> {
    let (bg, text_color) = match banner.kind {
        BannerKind::Success => (
            Color {
                r: 0.314,
                g: 0.980,
                b: 0.482,
                a: 0.15,
            },
            C_GREEN,
        ),
        BannerKind::Warning => (
            Color {
                r: 1.0,
                g: 0.722,
                b: 0.424,
                a: 0.15,
            },
            C_ORANGE,
        ),
        BannerKind::Error => (
            Color {
                r: 1.0,
                g: 0.333,
                b: 0.333,
                a: 0.15,
            },
            C_RED,
        ),
    };

    let msg_text = text(banner.message.clone()).size(13).color(text_color);

    let inner: Element<'_, Message> = if banner.dismissible {
        let dismiss = button(text("\u{00D7}").size(13).color(text_color))
            .on_press(msg(ManagerMessage::BannerDismissed))
            .padding(Padding::from([2u16, 8]))
            .style(|_t, _s| button::Style {
                background: None,
                ..button::Style::default()
            });
        row![msg_text, space().width(Length::Fill), dismiss]
            .align_y(Alignment::Center)
            .spacing(8)
            .into()
    } else {
        msg_text.into()
    };

    container(inner)
        .width(Length::Fill)
        .padding(Padding::from([8u16, 16]))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(bg)),
            ..container::Style::default()
        })
        .into()
}

fn achievements_tab(state: &ManagerState) -> Element<'_, Message> {
    column![filter_row(state), achievement_list(state)]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

const ACH_CARD_GAP: f32 = 10.0;
const ACH_CARD_MIN: f32 = 140.0;
const ACH_CARD_MAX: f32 = 240.0;

fn filter_row(state: &ManagerState) -> Element<'_, Message> {
    let search = text_input("Search achievements...", &state.search_query)
        .on_input(|s| msg(ManagerMessage::SearchChanged(s)))
        .padding(8)
        .size(13)
        .width(Length::Fill);

    let filter_pick = pick_list(AchievementFilter::ALL, Some(state.filter), |f| {
        msg(ManagerMessage::FilterChanged(f))
    })
    .text_size(13)
    .padding(8);

    let bulk_unlock = button(text("Unlock All").size(13))
        .on_press(msg(ManagerMessage::BulkAction(BulkOp::Unlock)))
        .padding(Padding::from([6u16, 10]));
    let bulk_lock = button(text("Lock All").size(13))
        .on_press(msg(ManagerMessage::BulkAction(BulkOp::Lock)))
        .padding(Padding::from([6u16, 10]));
    let bulk_invert = button(text("Invert").size(13))
        .on_press(msg(ManagerMessage::BulkAction(BulkOp::Invert)))
        .padding(Padding::from([6u16, 10]));

    let card_w = state.achievement_card_width;
    let width_slider = slider(ACH_CARD_MIN..=ACH_CARD_MAX, card_w, |v| {
        msg(ManagerMessage::AchievementCardWidthChanged(v))
    })
    .width(Length::Fixed(80.0));
    let slider_label = text(format!("{}px", card_w as u32)).size(12).color(C_MUTED);

    let r = row![
        search,
        filter_pick,
        bulk_unlock,
        bulk_lock,
        bulk_invert,
        width_slider,
        slider_label,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding::from([8u16, 16]));

    container(r)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            ..container::Style::default()
        })
        .into()
}

fn achievement_list(state: &ManagerState) -> Element<'_, Message> {
    let visible_ids =
        visible_achievement_ids(&state.achievements, state.filter, &state.search_query);

    let shown = visible_ids.len();
    let total = state.achievements.len();

    let by_id: std::collections::HashMap<&str, &AchievementRow> = state
        .achievements
        .iter()
        .map(|r| (r.data.id.as_str(), r))
        .collect();

    let cards: Vec<&AchievementRow> = visible_ids
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .collect();

    let count_str = if shown < total {
        format!("{shown} shown of {total}")
    } else {
        format!("{total} achievements")
    };

    let card_w = state.achievement_card_width;

    let grid = responsive(move |size| {
        let available = size.width.max(card_w + ACH_CARD_GAP);
        let cols = ((available + ACH_CARD_GAP) / (card_w + ACH_CARD_GAP))
            .floor()
            .max(1.0) as usize;

        let mut rows_col: iced::widget::Column<'_, Message> = column![]
            .spacing(ACH_CARD_GAP as u32)
            .padding(Padding::default().left(16).right(16).top(8).bottom(4));

        for chunk in cards.chunks(cols) {
            let mut r: iced::widget::Row<'_, Message> = row![]
                .spacing(ACH_CARD_GAP as u32)
                .align_y(Alignment::Start);
            for entry in chunk {
                r = r.push(achievement_card_widget(entry, card_w));
            }
            let needed = cols - chunk.len();
            for _ in 0..needed {
                r = r.push(iced::widget::Space::new().width(Length::Fixed(card_w)));
            }
            rows_col = rows_col.push(r);
        }

        let footer_note = container(text(count_str.clone()).size(12).color(C_MUTED))
            .padding(Padding::from([4u16, 0]));
        rows_col = rows_col.push(footer_note);

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

fn achievement_card_widget(row: &AchievementRow, card_w: f32) -> Element<'_, Message> {
    let effective = row.effective_achieved();
    let is_protected = row.data.permission != 0;
    let spoiler_hidden = row.data.is_hidden && !effective && !row.revealed;

    let icon_size = 80.0f32;

    let icon_el: Element<'_, Message> = if spoiler_hidden {
        container(text("\u{2754}").size(28).color(Color { a: 0.5, ..C_MUTED }))
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(Color {
                    r: C_CURRENT_LINE.r * 0.7,
                    g: C_CURRENT_LINE.g * 0.7,
                    b: C_CURRENT_LINE.b * 0.7,
                    a: 1.0,
                })),
                border: dracula_border_radius(6.0),
                ..container::Style::default()
            })
            .into()
    } else if let Some(ico) = &row.data.icon {
        let handle = image::Handle::from_rgba(ico.width, ico.height, ico.rgba.clone());
        let opacity = if effective { 1.0f32 } else { 0.45f32 };
        image(handle)
            .width(Length::Fixed(icon_size))
            .height(Length::Fixed(icon_size))
            .opacity(opacity)
            .into()
    } else {
        let icon_bg = if effective {
            C_CURRENT_LINE
        } else {
            Color {
                r: C_CURRENT_LINE.r * 0.6,
                g: C_CURRENT_LINE.g * 0.6,
                b: C_CURRENT_LINE.b * 0.6,
                a: 1.0,
            }
        };
        container(
            text(if effective { "\u{2713}" } else { "\u{25CB}" })
                .size(24)
                .color(if effective { C_GREEN } else { C_MUTED }),
        )
        .width(Length::Fixed(icon_size))
        .height(Length::Fixed(icon_size))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(icon_bg)),
            border: dracula_border_radius(6.0),
            ..container::Style::default()
        })
        .into()
    };

    let icon_area = container(icon_el)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .padding(Padding::from([8u16, 0]));

    let display_name = if spoiler_hidden {
        "Hidden Achievement".to_owned()
    } else {
        row.data.display_name.clone()
    };

    let name_color = if row.is_dirty { C_YELLOW } else { C_FG };
    let name_label = text(display_name).size(13).color(name_color);

    let description = if spoiler_hidden {
        String::new()
    } else {
        row.data.description.clone()
    };

    let desc_el: Option<Element<'_, Message>> = if !description.is_empty() {
        Some(text(description).size(11).color(C_MUTED).into())
    } else {
        None
    };

    let (badge_text, badge_color) = if is_protected {
        ("Protected", C_ORANGE)
    } else if spoiler_hidden {
        ("Hidden", C_MUTED)
    } else if row.is_dirty {
        ("Pending", C_YELLOW)
    } else if effective {
        ("Unlocked", C_GREEN)
    } else {
        ("Locked", C_MUTED)
    };

    let badge = container(text(badge_text).size(10).color(Color {
        a: 0.9,
        ..badge_color
    }))
    .padding(Padding::default().left(6).right(6).top(2).bottom(2))
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.15,
            ..badge_color
        })),
        border: iced::Border {
            color: Color {
                a: 0.4,
                ..badge_color
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    });

    let badge_row = container(badge)
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding(Padding::default().right(4).bottom(4));

    let mut text_col = column![name_label].spacing(3);
    if let Some(d) = desc_el {
        text_col = text_col.push(d);
    }

    let text_area = container(text_col)
        .width(Length::Fill)
        .padding(Padding::default().left(6).right(6).bottom(4));

    let card_body = column![icon_area, text_area, badge_row].spacing(0);

    let card_container = container(card_body)
        .width(Length::Fixed(card_w))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            border: iced::Border {
                radius: 8.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    if spoiler_hidden {
        let reveal_id = row.data.id.clone();
        let toggle_id = row.data.id.clone();

        let reveal_btn = button(text("Reveal").size(11).color(C_MUTED))
            .on_press(msg(ManagerMessage::RevealHidden(reveal_id)))
            .padding(Padding::from([4u16, 12]))
            .style(|_t, _s| button::Style {
                background: Some(iced::Background::Color(Color { a: 0.12, ..C_MUTED })),
                border: iced::Border {
                    color: Color { a: 0.3, ..C_MUTED },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: C_MUTED,
                ..button::Style::default()
            });

        let reveal_area = container(reveal_btn)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .padding(Padding::from([4u16, 0]));

        let card_with_reveal =
            container(column![card_container, reveal_area].spacing(4)).width(Length::Fixed(card_w));

        mouse_area(card_with_reveal)
            .on_press(msg(ManagerMessage::AchievementToggled(toggle_id)))
            .into()
    } else {
        let toggle_id = row.data.id.clone();
        mouse_area(card_container)
            .on_press(msg(ManagerMessage::AchievementToggled(toggle_id)))
            .into()
    }
}

fn stats_tab(state: &ManagerState) -> Element<'_, Message> {
    let consent_check = iced::widget::checkbox(state.stats_edit_consent)
        .on_toggle(|v| msg(ManagerMessage::StatsConsentToggled(v)))
        .size(14);
    let consent_label = text("I understand that editing stats may corrupt game saves").size(13);
    let consent_row = row![consent_check, consent_label]
        .spacing(8)
        .align_y(Alignment::Center);

    let consent_area = container(consent_row)
        .width(Length::Fill)
        .padding(Padding::from([12u16, 16]))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            ..container::Style::default()
        });

    if state.stats.is_empty() {
        let empty = container(
            text("No stats available for this game.")
                .size(13)
                .color(C_MUTED),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

        return column![consent_area, empty]
            .spacing(0)
            .height(Length::Fill)
            .into();
    }

    let header = container(
        row![
            text("Name")
                .size(13)
                .color(C_MUTED)
                .width(Length::FillPortion(3)),
            text("Value / Max")
                .size(13)
                .color(C_MUTED)
                .width(Length::FillPortion(2)),
            text("Type")
                .size(13)
                .color(C_MUTED)
                .width(Length::FillPortion(1)),
        ]
        .spacing(16)
        .padding(Padding::from([8u16, 16])),
    )
    .width(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(Color { a: 0.8, ..C_BG })),
        ..container::Style::default()
    });

    let rows: Vec<Element<'_, Message>> = state
        .stats
        .iter()
        .map(|s| stat_row_widget(s, state.stats_edit_consent))
        .collect();

    let list = scrollable(column(rows).spacing(0)).height(Length::Fill);

    column![consent_area, header, list]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

fn stat_row_widget(row: &StatRow, editing_enabled: bool) -> Element<'_, Message> {
    let is_protected = row.data.permission != 0;
    let can_edit = editing_enabled && !is_protected;

    let type_badge = match row.data.value {
        super::types::StatValue::Int(_) => "Int",
        super::types::StatValue::Float(_) => "Float",
    };

    let value_str = row.data.value.to_edit_string();
    let value_display = match row.data.max_value {
        Some(max) => format!("{value_str} / {max}"),
        None => value_str,
    };

    let value_col: Element<'_, Message> = if can_edit {
        text_input("", &row.edit_text)
            .on_input(|s| msg(ManagerMessage::StatEdited(row.data.id.clone(), s)))
            .on_submit(msg(ManagerMessage::StatEditCommitted(row.data.id.clone())))
            .padding(6)
            .size(13)
            .width(Length::FillPortion(2))
            .into()
    } else {
        container(
            text(value_display)
                .size(13)
                .color(if is_protected { C_ORANGE } else { C_FG }),
        )
        .width(Length::FillPortion(2))
        .padding(Padding::from([6u16, 8]))
        .into()
    };

    let dirty_dot: Element<'_, Message> = if row.is_dirty {
        text("*").size(12).color(C_YELLOW).into()
    } else {
        space().width(10).into()
    };

    let name_col: Element<'_, Message> = row![
        text(row.data.display_name.clone())
            .size(13)
            .color(C_FG)
            .width(Length::Fill),
        dirty_dot,
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .width(Length::FillPortion(3))
    .into();

    let type_col: Element<'_, Message> = text(type_badge)
        .size(12)
        .color(C_MUTED)
        .width(Length::FillPortion(1))
        .into();

    let main_row = row![name_col, value_col, type_col]
        .spacing(16)
        .align_y(Alignment::Center)
        .padding(Padding::from([8u16, 16]));

    let mut col_parts = column![main_row].spacing(0);

    if let Some(err) = &row.edit_error {
        col_parts = col_parts.push(
            container(text(err.clone()).size(12).color(C_RED))
                .padding(Padding::default().left(16).bottom(4)),
        );
    }

    container(col_parts)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            border: iced::Border {
                color: Color { a: 0.15, ..C_MUTED },
                width: 0.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn footer_bar(state: &ManagerState) -> Element<'_, Message> {
    let dirty = state.dirty_count();
    let has_errors = state.has_stat_errors();
    let is_busy = matches!(state.phase, ManagerPhase::Saving | ManagerPhase::Resetting);

    let reset_btn = button(text("\u{26A0} Reset...").size(13).color(C_RED))
        .on_press(msg(ManagerMessage::ResetClicked))
        .padding(Padding::from([8u16, 16]))
        .style(|_t, _s| button::Style {
            background: Some(iced::Background::Color(Color {
                r: 1.0,
                g: 0.333,
                b: 0.333,
                a: 0.1,
            })),
            border: iced::Border {
                color: C_RED,
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: C_RED,
            ..button::Style::default()
        });

    let cancel_label = if dirty > 0 {
        format!(
            "Cancel  {dirty} change{}",
            if dirty == 1 { "" } else { "s" }
        )
    } else {
        "Cancel".to_owned()
    };

    let cancel_style = |_t: &_, _s: _| button::Style {
        background: Some(iced::Background::Color(C_CURRENT_LINE)),
        border: dracula_border_radius(4.0),
        ..button::Style::default()
    };

    let cancel_btn = if dirty > 0 && !is_busy {
        button(text(cancel_label).size(13))
            .on_press(msg(ManagerMessage::DiscardChanges))
            .padding(Padding::from([8u16, 16]))
            .style(cancel_style)
    } else {
        button(text(cancel_label).size(13))
            .padding(Padding::from([8u16, 16]))
            .style(cancel_style)
    };

    let apply_label = if dirty > 0 {
        format!("Apply  {dirty} change{}", if dirty == 1 { "" } else { "s" })
    } else {
        "Apply Changes".to_owned()
    };

    let apply_enabled = dirty > 0 && !has_errors && !is_busy;
    let apply_btn = if apply_enabled {
        button(text(apply_label).size(13))
            .on_press(msg(ManagerMessage::ApplyChanges))
            .padding(Padding::from([8u16, 16]))
            .style(|_t, _s| button::Style {
                background: Some(iced::Background::Color(C_PURPLE)),
                border: dracula_border_radius(4.0),
                text_color: Color::BLACK,
                ..button::Style::default()
            })
    } else {
        button(text(apply_label).size(13))
            .padding(Padding::from([8u16, 16]))
            .style(|_t, _s| button::Style {
                background: Some(iced::Background::Color(Color { a: 0.3, ..C_PURPLE })),
                border: dracula_border_radius(4.0),
                text_color: Color { a: 0.4, ..C_FG },
                ..button::Style::default()
            })
    };

    let spinner_el: Element<'_, Message> = if is_busy {
        text(spinner_frame(state.spinner_angle))
            .size(16)
            .color(C_PURPLE)
            .into()
    } else {
        space().width(20).into()
    };

    let footer_row = row![
        reset_btn,
        space().width(Length::Fill),
        spinner_el,
        cancel_btn,
        apply_btn
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding::from([12u16, 16]));

    container(footer_row)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            ..container::Style::default()
        })
        .into()
}

fn reset_modal(state: &ManagerState) -> Element<'_, Message> {
    let scope_stats = radio_option(
        "Stats only",
        "Resets all stat counters to their default values.",
        state.reset_scope == ResetScope::StatsOnly,
        msg(ManagerMessage::ResetScopeSelected(ResetScope::StatsOnly)),
    );

    let scope_all = radio_option(
        "Stats + Achievements",
        "Resets stats AND locks all achievements. This cannot be undone.",
        state.reset_scope == ResetScope::StatsAndAchievements,
        msg(ManagerMessage::ResetScopeSelected(
            ResetScope::StatsAndAchievements,
        )),
    );

    let warning_box = container(
        column![
            text("\u{26A0} About cloud saves").size(13).color(C_ORANGE),
            text(
                "SteamLens resets Steam-side achievement and stat data. \
                 Games that use Steam Cloud may re-upload their own save \
                 data on next launch, restoring some or all values. \
                 Verify in-game progress after resetting."
            )
            .size(12)
            .color(C_MUTED),
        ]
        .spacing(4),
    )
    .padding(Padding::from([8u16, 12]))
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(Color {
            r: 1.0,
            g: 0.722,
            b: 0.424,
            a: 0.08,
        })),
        border: iced::Border {
            color: C_ORANGE,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    });

    let confirm_label = match state.reset_scope {
        ResetScope::StatsOnly => "Reset Stats \u{26A0}",
        ResetScope::StatsAndAchievements => "Reset Stats + Achievements \u{26A0}",
        ResetScope::Pending => "Reset \u{26A0}",
    };
    let confirm_btn = button(text(confirm_label).size(13).color(Color::WHITE))
        .on_press(msg(ManagerMessage::ResetConfirmed))
        .padding(Padding::from([8u16, 16]))
        .style(|_t, _s| button::Style {
            background: Some(iced::Background::Color(C_RED)),
            border: dracula_border_radius(4.0),
            ..button::Style::default()
        });

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(msg(ManagerMessage::ResetCancelled))
        .padding(Padding::from([8u16, 16]))
        .style(|_t, _s| button::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            border: dracula_border_radius(4.0),
            ..button::Style::default()
        });

    let button_row = row![cancel_btn, space().width(Length::Fill), confirm_btn]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().top(16));

    let modal_inner = column![
        text("\u{26A0}  Reset Options").size(16).color(C_FG),
        text("What would you like to reset?")
            .size(13)
            .color(C_MUTED),
        scope_stats,
        scope_all,
        warning_box,
        button_row,
    ]
    .spacing(12)
    .padding(Padding::from(24u16));

    let modal_box = container(modal_inner)
        .width(Length::Fixed(480.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(Color {
                r: 0.18,
                g: 0.19,
                b: 0.24,
                a: 1.0,
            })),
            border: iced::Border {
                color: C_CURRENT_LINE,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    let backdrop = container(space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.6,
            })),
            ..container::Style::default()
        });

    let centered = container(modal_box)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    stack![backdrop, centered].into()
}

fn radio_option<'a>(
    label: &'a str,
    sublabel: &'a str,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let indicator = if selected {
        text("(\u{2022})").size(14).color(C_PURPLE)
    } else {
        text("( )").size(14).color(C_MUTED)
    };

    let text_col = column![
        text(label).size(13).color(C_FG),
        text(sublabel).size(12).color(C_MUTED),
    ]
    .spacing(2);

    mouse_area(
        container(
            row![indicator, text_col]
                .spacing(8)
                .align_y(Alignment::Start),
        )
        .padding(Padding::from([8u16, 0])),
    )
    .on_press(on_press)
    .into()
}

fn saving_overlay<'a>(angle: f32, label: &'a str) -> Element<'a, Message> {
    let spinner = text(spinner_frame(angle)).size(24).color(C_PURPLE);

    let content = column![spinner, text(label).size(14).color(C_FG)]
        .spacing(8)
        .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.5,
            })),
            ..container::Style::default()
        })
        .into()
}

fn spinner_frame(angle: f32) -> &'static str {
    let frames = ["\u{25F4}", "\u{25F7}", "\u{25F6}", "\u{25F5}"];
    let idx = ((angle / 90.0) as usize) % frames.len();
    frames[idx]
}
