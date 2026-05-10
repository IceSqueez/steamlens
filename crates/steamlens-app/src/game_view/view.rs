use iced::widget::Id as WidgetId;
use iced::widget::{
    button, column, container, image, mouse_area, opaque, responsive, rich_text, row, scrollable,
    space, span, stack, text, text_input, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::screen::{ScreenContent, compose_screen};
use crate::ui::widgets::skeleton::skeleton_box;

pub fn achievement_search_id() -> WidgetId {
    WidgetId::new("achievement-search")
}

use super::types::{
    AchievementFilter, AchievementRow, AchievementSort, ActiveTab, BannerKind, BulkOp, RarityTier,
    ResetScope, StatRow, compute_tier_map, visible_achievement_ids,
};
use super::{GameViewMessage, GameViewPhase, GameViewState};
use crate::theme::{
    C_ACCENT, C_BORDER, C_DANGER, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY,
    C_TEXT_SECONDARY,
};
use crate::ui::theme::{AppTheme, palette};
use crate::ui::widgets::card::card;
use crate::ui::widgets::pill::pill;
use crate::ui::widgets::tooltip_box::tooltip_box;

const C_LOCKED_DESC: Color = Color::from_rgb8(0x99, 0x94, 0xb0);

const C_BG: Color = Color::from_rgb(0.157, 0.165, 0.212);
const C_CURRENT_LINE: Color = Color::from_rgb(0.267, 0.278, 0.353);
const C_FG: Color = Color::from_rgb(0.973, 0.973, 0.949);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_GREEN: Color = Color::from_rgb(0.314, 0.980, 0.482);
const C_ORANGE: Color = Color::from_rgb(1.0, 0.722, 0.424);
const C_PURPLE: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_RED: Color = Color::from_rgb(1.0, 0.333, 0.333);
const C_YELLOW: Color = Color::from_rgb(0.945, 0.980, 0.549);
const C_CYAN: Color = Color::from_rgb(0.545, 0.914, 0.992);
const C_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
const C_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

fn dracula_border_radius(r: f32) -> iced::Border {
    iced::Border {
        radius: r.into(),
        ..iced::Border::default()
    }
}

pub struct GameViewProps {
    pub skeleton_phase: f32,
}

pub fn render(state: &GameViewState, props: GameViewProps) -> Element<'_, GameViewMessage> {
    let skeleton_phase = props.skeleton_phase;
    match state.phase {
        GameViewPhase::Connecting | GameViewPhase::WaitingStats | GameViewPhase::LoadingData => {
            loading_view(state, skeleton_phase)
        }
        GameViewPhase::Saving | GameViewPhase::Resetting => {
            let base = loaded_view(state, skeleton_phase);
            let label = if state.phase == GameViewPhase::Saving {
                "Saving changes..."
            } else {
                "Resetting..."
            };
            stack![base, opaque(saving_overlay(state.spinner_angle, label))].into()
        }
        GameViewPhase::Ready => {
            let base = loaded_view(state, skeleton_phase);
            if state.show_reset_modal {
                stack![base, opaque(reset_modal(state))].into()
            } else {
                base
            }
        }
        GameViewPhase::Error => error_view(state),
    }
}

fn loading_view(state: &GameViewState, skeleton_phase: f32) -> Element<'_, GameViewMessage> {
    let phase_label = match state.phase {
        GameViewPhase::Connecting => "Connecting to Steam...",
        GameViewPhase::WaitingStats => "Requesting stats from Steam...",
        GameViewPhase::LoadingData => "Loading achievements...",
        _ => "Loading...",
    };

    let status_row = row![
        text(spinner_frame(state.spinner_angle))
            .size(16)
            .color(C_PURPLE),
        text(phase_label).size(13).color(C_MUTED),
        space().width(Length::Fill),
        button(text("Cancel").size(12))
            .on_press(GameViewMessage::RequestGoBack)
            .padding(Padding::default().left(12).right(12).top(4).bottom(4))
            .style(|_theme, _status| button::Style {
                background: None,
                border: iced::Border {
                    color: C_BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: C_TEXT_MUTED,
                ..button::Style::default()
            }),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding::default().left(16).right(16).top(8).bottom(8));

    let status_bar = container(status_row)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            ..container::Style::default()
        });

    let skeleton_grid = build_skeleton_ach_grid(state, skeleton_phase);

    column![status_bar, skeleton_grid]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

fn error_view(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let content = column![
        text("Failed to load").size(20).color(C_RED),
        text(&state.error_message).size(13).color(C_MUTED),
        button(text("Back").size(13))
            .on_press(GameViewMessage::RequestGoBack)
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

fn loaded_view(state: &GameViewState, skeleton_phase: f32) -> Element<'_, GameViewMessage> {
    let tabs = tab_bar_widget(state);
    let top_block: Element<'_, GameViewMessage> = if let Some(b) = &state.banner {
        column![tabs, banner_widget(b)].spacing(0).into()
    } else {
        tabs
    };

    let body = match state.active_tab {
        ActiveTab::Achievements => achievements_tab(state, skeleton_phase),
        ActiveTab::Stats => stats_tab(state),
    };

    compose_screen(ScreenContent {
        header: header_bar(state),
        top: Some(top_block),
        body,
        footer: Some(footer_bar(state)),
    })
}

fn header_bar(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let back_btn = button(text("\u{2039} Back").size(13).color(C_ACCENT))
        .on_press(GameViewMessage::RequestGoBack)
        .padding(Padding::from([0u16, 0]))
        .style(|_theme, _status| button::Style {
            background: None,
            border: iced::Border::default(),
            ..button::Style::default()
        });

    let divider = container(space())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(16.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_BORDER)),
            ..container::Style::default()
        });

    let earned = state
        .achievements
        .iter()
        .filter(|r| r.effective_achieved())
        .count();
    let total = state.achievements.len();

    let title_text = text(&state.game_name).size(16).color(C_TEXT_PRIMARY);

    let unlocked_subtitle = text(format!("{earned} / {total} unlocked"))
        .size(12)
        .color(C_TEXT_MUTED);

    let reload_btn = button(text("\u{21BB} Reload").size(12).color(C_TEXT_MUTED))
        .on_press(GameViewMessage::ReloadRequested)
        .padding(Padding::default().left(12).right(12).top(6).bottom(6))
        .style(|_theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if hovered {
                    Some(iced::Background::Color(C_HOVER))
                } else {
                    None
                },
                border: iced::Border {
                    color: C_BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: if hovered {
                    C_TEXT_PRIMARY
                } else {
                    C_TEXT_MUTED
                },
                ..button::Style::default()
            }
        });

    row![
        back_btn,
        divider,
        title_text,
        unlocked_subtitle,
        space().width(Length::Fill),
        reload_btn,
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}

fn tab_bar_widget(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let ach_count = state.achievements.len();
    let stats_count = state.stats.len();
    let ach_active = state.active_tab == ActiveTab::Achievements;

    let ach_tab = tab_button(
        "Achievements",
        ach_count,
        ach_active,
        GameViewMessage::TabChanged(ActiveTab::Achievements),
    );
    let stats_tab = tab_button(
        "Stats",
        stats_count,
        !ach_active,
        GameViewMessage::TabChanged(ActiveTab::Stats),
    );

    let tabs_row = row![ach_tab, stats_tab]
        .spacing(0)
        .padding(Padding::default().left(16).right(16).top(0).bottom(0));

    let underline = container(space())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_BORDER)),
            ..container::Style::default()
        });

    container(column![tabs_row, underline].spacing(0))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            ..container::Style::default()
        })
        .into()
}

fn tab_button(
    label: &'static str,
    count: usize,
    active: bool,
    on_press: GameViewMessage,
) -> Element<'static, GameViewMessage> {
    let label_text = text(label)
        .size(13)
        .color(if active { C_ACCENT } else { C_TEXT_MUTED });
    let count_text = text(format!("{count}")).size(11).color(C_TEXT_DIM);

    let inner = row![label_text, count_text]
        .spacing(4)
        .align_y(Alignment::Center);

    let active_indicator = container(space())
        .width(Length::Fill)
        .height(Length::Fixed(2.0))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(if active {
                C_ACCENT
            } else {
                Color::TRANSPARENT
            })),
            ..container::Style::default()
        });

    let tab_col = column![
        container(inner).padding(Padding::default().left(12).right(12).top(8).bottom(8)),
        active_indicator,
    ]
    .spacing(0);

    button(tab_col)
        .on_press(on_press)
        .padding(0)
        .style(move |_theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if hovered && !active {
                    Some(iced::Background::Color(C_HOVER))
                } else {
                    None
                },
                border: iced::Border::default(),
                ..button::Style::default()
            }
        })
        .into()
}

fn banner_widget(banner: &super::types::Banner) -> Element<'_, GameViewMessage> {
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

    let inner: Element<'_, GameViewMessage> = if banner.dismissible {
        let dismiss = button(text("\u{00D7}").size(13).color(text_color))
            .on_press(GameViewMessage::BannerDismissed)
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

fn achievements_tab<'a>(
    state: &'a GameViewState,
    skeleton_phase: f32,
) -> Element<'a, GameViewMessage> {
    let visible_ids = visible_achievement_ids(
        &state.achievements,
        state.filter,
        &state.search_query,
        state.achievement_sort,
        &state.rarity_tier_set,
        state.include_hidden,
    );
    let filtered_count = visible_ids.len();

    let mut col = column![filter_row(state), tier_rail(state)]
        .spacing(0)
        .height(Length::Fill);
    if let Some(indicator) = build_reveal_indicator(state) {
        col = col.push(indicator);
    }
    col = col.push(achievement_list(state, skeleton_phase));
    col = col.push(action_footer(state, filtered_count));
    col.into()
}

fn build_reveal_indicator(state: &GameViewState) -> Option<Element<'_, GameViewMessage>> {
    if !state.has_pending_reveals() && !state.has_fading_cards() {
        return None;
    }
    let total = state.achievements.len();
    let appeared = state.achievements.iter().filter(|r| r.appeared).count();

    let indicator_row = row![
        text(spinner_frame(state.spinner_angle))
            .size(13)
            .color(C_MUTED),
        text(format!("Loading {appeared} / {total} achievements…"))
            .size(12)
            .color(C_MUTED),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding(Padding::from([4u16, 16]));

    Some(container(indicator_row).width(Length::Fill).into())
}

const ACH_CARD_GAP: f32 = 12.0;
const ACH_MIN_GAP: f32 = 12.0;
const ACH_CARD_WIDTH: f32 = 260.0;

fn compute_ach_grid(viewport: f32, card_w: f32, min_gap: f32) -> (usize, f32) {
    let cols_max = ((viewport + min_gap) / (card_w + min_gap)).floor().max(1.0) as usize;

    let mut cols = cols_max;
    loop {
        let total_card_width = cols as f32 * card_w;
        let remainder = (viewport - total_card_width).max(0.0);
        let gap = remainder / (cols as f32 + 1.0);
        if gap >= min_gap || cols == 1 {
            return (cols, gap.max(0.0));
        }
        cols -= 1;
    }
}
const ACH_CARD_ICON: f32 = 64.0;
const ACH_CARD_HEIGHT: f32 = 140.0;

fn filter_row(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let search_input = text_input("Search achievements\u{2026}", &state.search_query)
        .id(achievement_search_id())
        .on_input(GameViewMessage::SearchChanged)
        .padding(Padding::default().left(10).right(10).top(6).bottom(6))
        .size(13)
        .style(|_theme, _status| iced::widget::text_input::Style {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            icon: C_TEXT_MUTED,
            placeholder: C_TEXT_MUTED,
            value: C_TEXT_PRIMARY,
            selection: Color {
                a: 0.35,
                ..C_ACCENT
            },
        });

    let search_block = container(search_input)
        .width(Length::Fixed(300.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        });

    let status_seg = status_segment(state.filter);

    let sort_label = text("SORT").size(11).color(C_TEXT_MUTED);
    let sort_seg = sort_segment(state.achievement_sort);
    let sort_row = row![sort_label, sort_seg]
        .spacing(6)
        .align_y(Alignment::Center);

    let inner = row![
        search_block,
        status_seg,
        space().width(Length::Fill),
        sort_row,
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .padding(Padding::default().left(16).right(16).top(8).bottom(8));

    container(inner)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            ..container::Style::default()
        })
        .into()
}

fn status_segment(current: AchievementFilter) -> Element<'static, GameViewMessage> {
    let filters = [
        AchievementFilter::All,
        AchievementFilter::Unlocked,
        AchievementFilter::Locked,
    ];

    let divider_el = || {
        container(space())
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(20.0))
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(C_BORDER)),
                ..container::Style::default()
            })
    };

    let mut items: Vec<Element<'static, GameViewMessage>> = Vec::new();
    for (i, &f) in filters.iter().enumerate() {
        let active = current == f;
        let btn =
            button(
                text(f.label())
                    .size(12)
                    .color(if active { C_ACCENT } else { C_TEXT_MUTED }),
            )
            .on_press(GameViewMessage::FilterChanged(f))
            .padding(Padding::default().left(10).right(10).top(5).bottom(5))
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(if active {
                    Color {
                        a: 0.15,
                        ..C_ACCENT
                    }
                } else {
                    Color::TRANSPARENT
                })),
                border: iced::Border::default(),
                ..button::Style::default()
            });
        items.push(btn.into());
        if i < filters.len() - 1 {
            items.push(divider_el().into());
        }
    }

    container(row(items).align_y(Alignment::Center))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn sort_segment(current: AchievementSort) -> Element<'static, GameViewMessage> {
    let divider_el = || {
        container(space())
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(20.0))
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(C_BORDER)),
                ..container::Style::default()
            })
    };

    let mut items: Vec<Element<'static, GameViewMessage>> = Vec::new();
    let last_idx = AchievementSort::ALL.len() - 1;
    for (i, &s) in AchievementSort::ALL.iter().enumerate() {
        let active = current == s;
        let btn = button(text(s.short_label()).size(12).color(if active {
            C_ACCENT
        } else {
            C_TEXT_MUTED
        }))
        .on_press(GameViewMessage::AchievementSortChanged(s))
        .padding(Padding::default().left(10).right(10).top(5).bottom(5))
        .style(move |_theme, _status| button::Style {
            background: Some(iced::Background::Color(if active {
                Color {
                    a: 0.15,
                    ..C_ACCENT
                }
            } else {
                Color::TRANSPARENT
            })),
            border: iced::Border::default(),
            ..button::Style::default()
        });

        let with_tooltip: Element<'static, GameViewMessage> =
            tooltip_box(btn, s.tooltip(), tooltip::Position::Bottom);

        items.push(with_tooltip);
        if i < last_idx {
            items.push(divider_el().into());
        }
    }

    container(row(items).align_y(Alignment::Center))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn tier_rail(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let tier_map = compute_tier_map(&state.achievements);
    let all_tiers = [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Mythical,
        RarityTier::Legendary,
    ];

    let hidden_count = state
        .achievements
        .iter()
        .filter(|r| r.is_spoiler_hidden())
        .count();

    let tier_lbl = text("TIER").size(11).color(C_TEXT_MUTED);
    let mut chips: Vec<Element<'_, GameViewMessage>> = vec![tier_lbl.into()];

    for &tier in &all_tiers {
        let count = tier_map.values().filter(|&&v| v == tier).count();
        let color = tier_color(tier);
        let active = state.rarity_tier_set.contains(&tier);
        let bg_alpha: f32 = if active { 0.18 } else { 0.10 };
        let border_alpha: f32 = if active { 0.40 } else { 0.20 };

        let dot = container(space())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let pill_inner = row![
            dot,
            text(tier.label()).size(11).color(color),
            text(format!("{count}"))
                .size(11)
                .color(Color { a: 0.65, ..color }),
        ]
        .spacing(4)
        .align_y(Alignment::Center);

        let chip = button(pill_inner)
            .on_press(GameViewMessage::RarityTierToggled(tier))
            .padding(Padding::default().left(8).right(10).top(4).bottom(4))
            .style(move |_theme, _status| button::Style {
                background: Some(iced::Background::Color(Color {
                    a: bg_alpha,
                    ..color
                })),
                border: iced::Border {
                    color: Color {
                        a: border_alpha,
                        ..color
                    },
                    width: 1.0,
                    radius: 14.0.into(),
                },
                text_color: color,
                ..button::Style::default()
            });

        chips.push(chip.into());
    }

    let hidden_active = state.include_hidden;
    let hidden_dot = container(space())
        .width(Length::Fixed(6.0))
        .height(Length::Fixed(6.0))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(Color {
                a: if hidden_active { 0.8 } else { 0.5 },
                ..C_TEXT_DIM
            })),
            border: iced::Border {
                radius: 3.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let hidden_inner = row![
        hidden_dot,
        text("Hidden").size(11).color(if hidden_active {
            C_TEXT_MUTED
        } else {
            C_TEXT_DIM
        }),
        text(format!("{hidden_count}")).size(11).color(C_TEXT_DIM),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let hidden_chip = button(hidden_inner)
        .on_press(GameViewMessage::HiddenPillToggled)
        .padding(Padding::default().left(8).right(10).top(4).bottom(4))
        .style(move |_theme, _status| button::Style {
            background: None,
            border: iced::Border {
                color: Color {
                    a: if hidden_active { 0.6 } else { 0.35 },
                    ..C_BORDER
                },
                width: 1.0,
                radius: 14.0.into(),
            },
            text_color: C_TEXT_DIM,
            ..button::Style::default()
        });

    chips.push(hidden_chip.into());
    chips.push(space().width(Length::Fill).into());

    let any_active = !state.rarity_tier_set.is_empty() || state.include_hidden;
    if any_active {
        let clear_btn = button(text("Clear").size(11).color(C_TEXT_DIM))
            .on_press(GameViewMessage::RarityFilterCleared)
            .padding(Padding::default().left(6).right(6).top(3).bottom(3))
            .style(|_theme, _status| button::Style {
                background: None,
                border: iced::Border::default(),
                text_color: C_TEXT_DIM,
                ..button::Style::default()
            });
        chips.push(clear_btn.into());
    }

    let rail_row = row(chips)
        .spacing(6)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(16).right(16).top(6).bottom(6));

    container(rail_row)
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            ..container::Style::default()
        })
        .into()
}

fn action_footer<'a>(
    state: &'a GameViewState,
    filtered_count: usize,
) -> Element<'a, GameViewMessage> {
    let total = state.achievements.len();

    let count_label: Element<'_, GameViewMessage> = row![
        text(format!("{filtered_count}"))
            .size(12)
            .color(C_TEXT_PRIMARY),
        text(format!(" of {total} achievements"))
            .size(12)
            .color(C_TEXT_MUTED),
    ]
    .into();

    let mk_outlined =
        |lbl: &'static str, tc: Color, m: GameViewMessage| -> Element<'_, GameViewMessage> {
            button(text(lbl).size(12).color(tc))
                .on_press(m)
                .padding(Padding::default().left(12).right(12).top(6).bottom(6))
                .style(move |_theme, status| {
                    let hovered =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: if hovered {
                            Some(iced::Background::Color(C_HOVER))
                        } else {
                            None
                        },
                        border: iced::Border {
                            color: C_BORDER,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        text_color: tc,
                        ..button::Style::default()
                    }
                })
                .into()
        };

    let unlock_btn = mk_outlined(
        "Unlock all",
        C_ACCENT,
        GameViewMessage::BulkAction(BulkOp::Unlock),
    );
    let lock_btn = mk_outlined(
        "Lock all",
        C_TEXT_MUTED,
        GameViewMessage::BulkAction(BulkOp::Lock),
    );
    let invert_btn = mk_outlined(
        "Invert",
        C_TEXT_MUTED,
        GameViewMessage::BulkAction(BulkOp::Invert),
    );

    let vert_divider = container(space())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(20.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_BORDER)),
            ..container::Style::default()
        });

    let reset_btn = button(
        row![
            text("\u{26A0}").size(12).color(C_DANGER),
            text(" Reset").size(12).color(C_DANGER),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(GameViewMessage::ResetClicked)
    .padding(Padding::default().left(12).right(12).top(6).bottom(6))
    .style(|_theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: if hovered {
                Some(iced::Background::Color(Color {
                    a: 0.12,
                    ..C_DANGER
                }))
            } else {
                None
            },
            border: iced::Border {
                color: Color {
                    a: 0.30,
                    ..C_DANGER
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            text_color: C_DANGER,
            ..button::Style::default()
        }
    });

    let action_row = row![
        count_label,
        space().width(Length::Fill),
        unlock_btn,
        lock_btn,
        invert_btn,
        vert_divider,
        reset_btn,
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .padding(Padding::default().left(16).right(16).top(8).bottom(8));

    let top_rule = container(space())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_BORDER)),
            ..container::Style::default()
        });

    container(column![top_rule, action_row].spacing(0))
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            ..container::Style::default()
        })
        .into()
}

fn tier_color(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_GREEN,
        RarityTier::Uncommon => C_CYAN,
        RarityTier::Rare => C_PURPLE,
        RarityTier::Mythical => C_MYTHICAL,
        RarityTier::Legendary => C_LEGENDARY,
    }
}

fn icon_glow_style(tier: Option<RarityTier>, glow_pulse: f32) -> container::Style {
    match tier {
        Some(RarityTier::Legendary) => {
            let alpha = 0.75 + 0.25 * glow_pulse;
            let blur = 22.0 + 16.0 * glow_pulse;
            container::Style {
                shadow: iced::Shadow {
                    color: Color {
                        a: alpha,
                        ..C_LEGENDARY
                    },
                    offset: iced::Vector::new(0.0, 0.0),
                    blur_radius: blur,
                },
                border: iced::Border {
                    color: C_LEGENDARY,
                    width: 3.0,
                    radius: 8.0.into(),
                },
                ..container::Style::default()
            }
        }
        Some(RarityTier::Mythical) => container::Style {
            shadow: iced::Shadow {
                color: Color {
                    a: 0.45,
                    ..C_MYTHICAL
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 16.0,
            },
            border: iced::Border {
                color: C_MYTHICAL,
                width: 2.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        },
        Some(RarityTier::Rare) => container::Style {
            shadow: iced::Shadow {
                color: Color { a: 0.7, ..C_PURPLE },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            border: iced::Border {
                color: C_PURPLE,
                width: 1.5,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        },
        Some(RarityTier::Uncommon) => container::Style {
            shadow: iced::Shadow {
                color: Color { a: 0.7, ..C_CYAN },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            ..container::Style::default()
        },
        Some(RarityTier::Common) => container::Style {
            shadow: iced::Shadow {
                color: Color { a: 0.7, ..C_GREEN },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 14.0,
            },
            ..container::Style::default()
        },
        None => container::Style {
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                offset: iced::Vector::new(1.5, 1.5),
                blur_radius: 3.0,
            },
            ..container::Style::default()
        },
    }
}

fn achievement_list(state: &GameViewState, skeleton_phase: f32) -> Element<'_, GameViewMessage> {
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

    let grid = responsive(move |size| {
        let (cols, gap) = compute_ach_grid(size.width, ACH_CARD_WIDTH, ACH_MIN_GAP);

        let mut rows_col: iced::widget::Column<'_, GameViewMessage> = column![]
            .spacing(ACH_CARD_GAP as u32)
            .padding(Padding::default().top(8).bottom(4));

        for chunk in cards.chunks(cols) {
            let mut r: iced::widget::Row<'_, GameViewMessage> =
                row![space().width(Length::Fixed(gap))].align_y(Alignment::Start);
            for entry in chunk {
                let tier = tier_map.get(&entry.data.id).copied();
                r = r.push(achievement_card_widget(
                    entry,
                    ACH_CARD_WIDTH,
                    query_owned.clone(),
                    glow_pulse,
                    tier,
                    skeleton_phase,
                ));
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

fn build_skeleton_ach_grid(
    state: &GameViewState,
    skeleton_phase: f32,
) -> Element<'_, GameViewMessage> {
    let count = if state.achievements.is_empty() {
        6
    } else {
        state.achievements.len()
    };

    let grid = responsive(move |size| {
        let (cols, gap) = compute_ach_grid(size.width, ACH_CARD_WIDTH, ACH_MIN_GAP);

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

fn build_skeleton_ach_card(card_w: f32, phase: f32) -> Element<'static, GameViewMessage> {
    let icon = skeleton_box(ACH_CARD_ICON, ACH_CARD_ICON, phase);

    let title_w = card_w * 0.60;
    let desc_w = card_w * 0.80;

    let text_col = column![
        skeleton_box(title_w, 13.0, phase),
        skeleton_box(desc_w, 11.0, phase),
    ]
    .spacing(4);

    let top_row = row![icon, text_col]
        .spacing(8)
        .align_y(Alignment::Start)
        .padding(Padding::from([8u16, 8]));

    let pill1 = skeleton_box(80.0, 18.0, phase);
    let pill2 = skeleton_box(60.0, 18.0, phase);

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
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(C_CURRENT_LINE)),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    card_container.into()
}

fn achievement_card_widget<'a>(
    row: &'a AchievementRow,
    card_w: f32,
    search_query: String,
    glow_pulse: f32,
    tier: Option<RarityTier>,
    _skeleton_phase: f32,
) -> Element<'a, GameViewMessage> {
    let effective = row.effective_achieved();
    let spoiler_hidden = row.is_spoiler_hidden();

    let icon_el: Element<'_, GameViewMessage> = if spoiler_hidden {
        container(text("\u{2754}").size(22).color(Color { a: 0.5, ..C_MUTED }))
            .width(Length::Fixed(ACH_CARD_ICON))
            .height(Length::Fixed(ACH_CARD_ICON))
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
        container(
            image(handle)
                .width(Length::Fixed(ACH_CARD_ICON))
                .height(Length::Fixed(ACH_CARD_ICON))
                .opacity(opacity),
        )
        .style(move |_theme| icon_glow_style(tier, glow_pulse))
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
                .size(20)
                .color(if effective { C_GREEN } else { C_MUTED }),
        )
        .width(Length::Fixed(ACH_CARD_ICON))
        .height(Length::Fixed(ACH_CARD_ICON))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(icon_bg)),
            border: dracula_border_radius(6.0),
            ..container::Style::default()
        })
        .into()
    };

    let display_name = if spoiler_hidden {
        "Hidden Achievement".to_owned()
    } else {
        row.data.display_name.clone()
    };

    let name_color = if row.is_dirty { C_YELLOW } else { C_FG };
    let name_label: Element<'_, GameViewMessage> =
        if !row.is_dirty && !spoiler_hidden && !search_query.is_empty() {
            if let Some((before, matched, after)) = highlight_split(&display_name, &search_query) {
                let before = before.to_owned();
                let matched = matched.to_owned();
                let after = after.to_owned();
                container(
                    rich_text![
                        span(before).color(C_FG),
                        span(matched)
                            .color(C_YELLOW)
                            .background(Color { a: 0.2, ..C_YELLOW }),
                        span(after).color(C_FG),
                    ]
                    .on_link_click(iced::never)
                    .size(13)
                    .wrapping(text::Wrapping::Word)
                    .line_height(text::LineHeight::Relative(1.2)),
                )
                .width(Length::Fill)
                .height(Length::Fixed(36.0))
                .into()
            } else {
                container(
                    text(display_name)
                        .size(13)
                        .color(name_color)
                        .wrapping(text::Wrapping::Word)
                        .line_height(text::LineHeight::Relative(1.2)),
                )
                .width(Length::Fill)
                .height(Length::Fixed(36.0))
                .into()
            }
        } else {
            container(
                text(display_name)
                    .size(13)
                    .color(name_color)
                    .wrapping(text::Wrapping::Word)
                    .line_height(text::LineHeight::Relative(1.2)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(36.0))
            .into()
        };

    let description = if spoiler_hidden {
        "Hidden until revealed".to_owned()
    } else {
        row.data.description.clone()
    };

    let desc_color = if spoiler_hidden {
        Color { a: 0.5, ..C_MUTED }
    } else if !effective {
        C_LOCKED_DESC
    } else {
        C_TEXT_SECONDARY
    };

    let desc_label: Element<'_, GameViewMessage> = if !spoiler_hidden && !search_query.is_empty() {
        if let Some((before, matched, after)) = highlight_split(&description, &search_query) {
            let before = before.to_owned();
            let matched = matched.to_owned();
            let after = after.to_owned();
            container(
                rich_text![
                    span(before).color(desc_color),
                    span(matched)
                        .color(C_YELLOW)
                        .background(Color { a: 0.2, ..C_YELLOW }),
                    span(after).color(desc_color),
                ]
                .on_link_click(iced::never)
                .size(11)
                .wrapping(text::Wrapping::Word),
            )
            .width(Length::Fill)
            .height(Length::Fixed(30.0))
            .into()
        } else {
            container(
                text(description)
                    .size(11)
                    .color(desc_color)
                    .wrapping(text::Wrapping::Word),
            )
            .width(Length::Fill)
            .height(Length::Fixed(30.0))
            .into()
        }
    } else {
        container(
            text(description)
                .size(11)
                .color(desc_color)
                .wrapping(text::Wrapping::Word),
        )
        .width(Length::Fill)
        .height(Length::Fixed(30.0))
        .into()
    };

    let text_col = column![name_label, desc_label].spacing(2);

    let top_row = row![icon_el, text_col]
        .spacing(8)
        .align_y(Alignment::Start)
        .padding(Padding::from([8u16, 8]));

    let badge_text = row.status_label();
    let is_locked_badge = badge_text == "Locked";
    let badge_color = match badge_text {
        "Protected" => C_ORANGE,
        "Pending" => C_YELLOW,
        "Unlocked" => C_GREEN,
        _ => C_TEXT_MUTED,
    };

    let badge_text_color = if is_locked_badge {
        C_LOCKED_DESC
    } else {
        Color {
            a: 0.9,
            ..badge_color
        }
    };
    let badge = pill(
        text(badge_text).size(11).color(badge_text_color),
        badge_color,
    );

    let rarity_badge: Option<Element<'_, GameViewMessage>> = if spoiler_hidden {
        None
    } else if let (Some(t), Some(pct)) = (tier, row.rarity_percent) {
        let tc = tier_color(t);
        let label = format!("{} \u{00B7} {:.1}%", t.label(), pct);
        let label_text = text(label).size(11).color(Color { a: 0.95, ..tc });
        let rb = pill(label_text, tc).with_dot(tc);
        Some(rb.into())
    } else {
        None
    };

    let bottom_row: Element<'_, GameViewMessage> = if spoiler_hidden {
        let reveal_id = row.data.id.clone();
        let reveal_btn = button(text("Reveal").size(11).color(C_ACCENT))
            .on_press(GameViewMessage::RevealHidden(reveal_id))
            .padding(Padding::default().left(12).right(12).top(3).bottom(3))
            .style(|_t, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(Color {
                        a: if hovered { 0.28 } else { 0.18 },
                        ..C_ACCENT
                    })),
                    border: Border {
                        color: Color {
                            a: if hovered { 0.65 } else { 0.45 },
                            ..C_ACCENT
                        },
                        width: 1.0,
                        radius: 12.0.into(),
                    },
                    text_color: C_ACCENT,
                    ..button::Style::default()
                }
            });

        row![reveal_btn, space().width(Length::Fill), badge]
            .spacing(4)
            .align_y(Alignment::Center)
            .padding(Padding::default().left(8).right(8).bottom(8))
            .into()
    } else {
        let mut right_group: iced::widget::Row<'_, GameViewMessage> =
            row![].spacing(6).align_y(Alignment::Center);
        if let Some(rb) = rarity_badge {
            right_group = right_group.push(rb);
        }
        right_group = right_group.push(badge);

        container(right_group)
            .width(Length::Fill)
            .align_x(Alignment::End)
            .padding(Padding::default().right(8).bottom(8))
            .into()
    };

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
        .height(Length::Fixed(ACH_CARD_HEIGHT));

    let toggle_id = row.data.id.clone();
    let is_hidden_card = spoiler_hidden;
    let glow_color: Option<Color> = if effective && !spoiler_hidden {
        tier.map(tier_color)
    } else {
        None
    };

    let mut c = card(card_container)
        .theme(AppTheme::Dark)
        .on_press(GameViewMessage::AchievementToggled(toggle_id));

    if is_hidden_card {
        c = c
            .accent(palette(AppTheme::Dark).border)
            .accent_border_width(1.0, 1.0)
            .accent_alpha(0.40, 0.55)
            .radius(10.0);
    } else if let Some(gc) = glow_color {
        c = c
            .accent(gc)
            .accent_border_width(1.0, 2.0)
            .accent_alpha(0.45, 0.85)
            .radius(8.0);
    } else {
        c = c.radius(8.0);
    }

    c.into()
}

fn stats_tab(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let consent_check = iced::widget::checkbox(state.stats_edit_consent)
        .on_toggle(GameViewMessage::StatsConsentToggled)
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

    let rows: Vec<Element<'_, GameViewMessage>> = state
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

fn stat_row_widget(row: &StatRow, editing_enabled: bool) -> Element<'_, GameViewMessage> {
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

    let value_col: Element<'_, GameViewMessage> = if can_edit {
        text_input("", &row.edit_text)
            .on_input(|s| GameViewMessage::StatEdited(row.data.id.clone(), s))
            .on_submit(GameViewMessage::StatEditCommitted(row.data.id.clone()))
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

    let dirty_dot: Element<'_, GameViewMessage> = if row.is_dirty {
        text("*").size(12).color(C_YELLOW).into()
    } else {
        space().width(10).into()
    };

    let name_col: Element<'_, GameViewMessage> = row![
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

    let type_col: Element<'_, GameViewMessage> = text(type_badge)
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

fn footer_bar(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let dirty = state.dirty_count();
    let has_errors = state.has_stat_errors();
    let is_busy = matches!(
        state.phase,
        GameViewPhase::Saving | GameViewPhase::Resetting
    );

    let cancel_label = if dirty > 0 {
        format!(
            "Cancel  {dirty} change{}",
            if dirty == 1 { "" } else { "s" }
        )
    } else {
        "Cancel".to_owned()
    };

    let cancel_btn = if dirty > 0 && !is_busy {
        button(text(cancel_label).size(13).color(C_FG))
            .on_press(GameViewMessage::DiscardChanges)
            .padding(Padding::from([8u16, 16]))
            .style(|_t, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: if hovered {
                        Some(iced::Background::Color(Color { a: 0.2, ..C_MUTED }))
                    } else {
                        None
                    },
                    border: iced::Border {
                        color: C_MUTED,
                        width: 2.0,
                        radius: 4.0.into(),
                    },
                    text_color: C_FG,
                    ..button::Style::default()
                }
            })
    } else {
        button(text(cancel_label).size(13).color(Color { a: 0.4, ..C_FG }))
            .padding(Padding::from([8u16, 16]))
            .style(|_t, _status| button::Style {
                background: None,
                border: iced::Border {
                    color: Color { a: 0.3, ..C_MUTED },
                    width: 2.0,
                    radius: 4.0.into(),
                },
                text_color: Color { a: 0.4, ..C_FG },
                ..button::Style::default()
            })
    };

    let apply_label = if dirty > 0 {
        format!("Apply  {dirty} change{}", if dirty == 1 { "" } else { "s" })
    } else {
        "Apply Changes".to_owned()
    };

    let apply_enabled = dirty > 0 && !has_errors && !is_busy;
    let apply_btn = if apply_enabled {
        button(text(apply_label).size(13))
            .on_press(GameViewMessage::ApplyChanges)
            .padding(Padding::from([8u16, 16]))
            .style(|_t, status| {
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                let bg_color = if hovered {
                    Color {
                        r: (C_PURPLE.r * 0.9 + 0.1).min(1.0),
                        g: (C_PURPLE.g * 0.9 + 0.1).min(1.0),
                        b: (C_PURPLE.b * 0.9 + 0.1).min(1.0),
                        a: 1.0,
                    }
                } else {
                    C_PURPLE
                };
                button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: dracula_border_radius(4.0),
                    text_color: Color::BLACK,
                    shadow: if hovered {
                        iced::Shadow {
                            color: Color { a: 0.6, ..C_PURPLE },
                            offset: iced::Vector::new(0.0, 0.0),
                            blur_radius: 8.0,
                        }
                    } else {
                        iced::Shadow::default()
                    },
                    ..button::Style::default()
                }
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

    let spinner_el: Element<'_, GameViewMessage> = if is_busy {
        text(spinner_frame(state.spinner_angle))
            .size(16)
            .color(C_PURPLE)
            .into()
    } else {
        space().width(20).into()
    };

    row![
        space().width(Length::Fill),
        spinner_el,
        cancel_btn,
        apply_btn
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn reset_modal(state: &GameViewState) -> Element<'_, GameViewMessage> {
    let scope_stats = radio_option(
        "Stats only",
        "Resets all stat counters to their default values.",
        state.reset_scope == ResetScope::StatsOnly,
        GameViewMessage::ResetScopeSelected(ResetScope::StatsOnly),
    );

    let scope_all = radio_option(
        "Stats + Achievements",
        "Resets stats AND locks all achievements. This cannot be undone.",
        state.reset_scope == ResetScope::StatsAndAchievements,
        GameViewMessage::ResetScopeSelected(ResetScope::StatsAndAchievements),
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

    let confirm_input_label = text(format!(
        "Type \"{name}\" to confirm:",
        name = state.game_name.trim()
    ))
    .size(12)
    .color(C_MUTED);

    let confirm_input = text_input(state.game_name.trim(), &state.reset_confirm_input)
        .on_input(GameViewMessage::ResetConfirmInputChanged)
        .size(13)
        .padding(Padding::from([6u16, 10]))
        .style(|_theme, _status| iced::widget::text_input::Style {
            background: iced::Background::Color(Color {
                r: 0.12,
                g: 0.13,
                b: 0.17,
                a: 1.0,
            }),
            border: iced::Border {
                color: C_MUTED,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: C_MUTED,
            placeholder: Color { a: 0.3, ..C_MUTED },
            value: C_FG,
            selection: Color {
                a: 0.35,
                ..C_PURPLE
            },
        });

    let confirm_gate = column![confirm_input_label, confirm_input].spacing(4);

    let confirm_enabled = state.reset_confirm_matches();

    let confirm_label = match state.reset_scope {
        ResetScope::StatsOnly => "Reset Stats \u{26A0}",
        ResetScope::StatsAndAchievements => "Reset Stats + Achievements \u{26A0}",
        ResetScope::Pending => "Reset \u{26A0}",
    };
    let confirm_btn = {
        let base = button(text(confirm_label).size(13).color(if confirm_enabled {
            Color::WHITE
        } else {
            Color {
                a: 0.4,
                ..Color::WHITE
            }
        }))
        .padding(Padding::from([8u16, 16]))
        .style(move |_t, _s| button::Style {
            background: Some(iced::Background::Color(Color {
                a: if confirm_enabled { 1.0 } else { 0.3 },
                ..C_RED
            })),
            border: dracula_border_radius(4.0),
            ..button::Style::default()
        });
        if confirm_enabled {
            base.on_press(GameViewMessage::ResetConfirmed)
        } else {
            base
        }
    };

    let cancel_btn = button(text("Cancel").size(13))
        .on_press(GameViewMessage::ResetCancelled)
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
        confirm_gate,
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
    on_press: GameViewMessage,
) -> Element<'a, GameViewMessage> {
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

fn saving_overlay<'a>(angle: f32, label: &'a str) -> Element<'a, GameViewMessage> {
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

fn highlight_split<'a>(source: &'a str, query: &str) -> Option<(&'a str, &'a str, &'a str)> {
    if query.is_empty() {
        return None;
    }
    let lower_source = source.to_lowercase();
    let lower_query = query.to_lowercase();
    let byte_offset = lower_source.find(&lower_query)?;
    let match_end = byte_offset + lower_query.len();
    let before = &source[..byte_offset];
    let matched = &source[byte_offset..match_end];
    let after = &source[match_end..];
    Some((before, matched, after))
}

#[cfg(test)]
mod skeleton_polish_tests {
    use super::{ACH_CARD_HEIGHT, ACH_CARD_ICON, ACH_CARD_WIDTH, C_LOCKED_DESC};
    use crate::theme::C_TEXT_MUTED;
    use iced::Color;

    #[test]
    fn skeleton_card_height_matches_hydrated_card_height() {
        assert_eq!(
            ACH_CARD_HEIGHT, 140.0,
            "skeleton and hydrated paths both use ACH_CARD_HEIGHT — must agree"
        );
    }

    #[test]
    fn skeleton_icon_size_matches_hydrated_icon_size() {
        assert_eq!(
            ACH_CARD_ICON, 64.0,
            "icon placeholder size must match real icon size"
        );
    }

    #[test]
    fn card_width_constant_is_reasonable() {
        const { assert!(ACH_CARD_WIDTH >= 200.0 && ACH_CARD_WIDTH <= 400.0) };
    }

    #[test]
    fn locked_desc_color_is_lighter_than_text_muted() {
        let locked = C_LOCKED_DESC;
        let muted = C_TEXT_MUTED;
        let luminance = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        assert!(
            luminance(locked) >= luminance(muted),
            "C_LOCKED_DESC must be lighter than or equal to C_TEXT_MUTED — locked descriptions should be more readable"
        );
    }

    #[test]
    fn locked_desc_color_has_correct_rgb() {
        let Color { r, g, b, .. } = C_LOCKED_DESC;
        let r8 = (r * 255.0).round() as u8;
        let g8 = (g * 255.0).round() as u8;
        let b8 = (b * 255.0).round() as u8;
        assert_eq!(r8, 0x99, "red channel");
        assert_eq!(g8, 0x94, "green channel");
        assert_eq!(b8, 0xb0, "blue channel");
    }

    #[test]
    fn skeleton_grid_uses_same_card_dimensions_as_hydrated() {
        let title_w = ACH_CARD_WIDTH * 0.60;
        let desc_w = ACH_CARD_WIDTH * 0.80;
        assert!(
            title_w < ACH_CARD_WIDTH,
            "title skeleton must be narrower than card"
        );
        assert!(
            desc_w < ACH_CARD_WIDTH,
            "desc skeleton must be narrower than card"
        );
        assert!(
            title_w < desc_w,
            "title skeleton must be narrower than desc skeleton"
        );
    }
}

#[cfg(test)]
mod ach_grid_tests {
    use super::compute_ach_grid;

    #[test]
    fn fixed_card_width_with_uniform_gaps() {
        let (cols, gap) = compute_ach_grid(1000.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 40.0).abs() < 0.01, "expected gap=40, got {gap}");
    }

    #[test]
    fn min_gap_floor_kicks_in() {
        let (cols, gap) = compute_ach_grid(1010.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 42.0).abs() < 0.01, "expected gap=42, got {gap}");
    }

    #[test]
    fn single_column_below_card_width() {
        let (cols, gap) = compute_ach_grid(150.0, 200.0, 12.0);
        assert_eq!(cols, 1);
        assert_eq!(gap, 0.0);
    }

    #[test]
    fn gap_never_negative() {
        let (_cols, gap) = compute_ach_grid(50.0, 260.0, 12.0);
        assert!(gap >= 0.0);
    }
}
