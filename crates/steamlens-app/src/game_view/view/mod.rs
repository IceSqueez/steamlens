mod card;
mod footer_modal;
mod grid;

use iced::widget::Id as WidgetId;
use iced::widget::{button, column, container, opaque, stack, text};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::game_view::types::RarityTier;
use crate::game_view::{GameViewMessage, GameViewPhase, GameViewState};
use crate::screen::{ScreenContent, compose_screen};
use crate::ui::theme::{palette, theme_from_iced};

use footer_modal::{apply_modal, footer_bar, saving_overlay};
use grid::achievements_tab;

pub(super) fn dracula_border_radius(r: f32) -> iced::Border {
    iced::Border {
        radius: r.into(),
        ..iced::Border::default()
    }
}

pub fn achievement_search_id() -> WidgetId {
    WidgetId::new("achievement-search")
}

pub struct GameViewProps<'a> {
    pub skeleton_phase: f32,
    pub app_theme: crate::ui::theme::AppTheme,
    pub capsules: &'a crate::app_context::CapsuleStore,
}

pub fn render<'a>(
    state: &'a GameViewState,
    props: GameViewProps<'a>,
) -> Element<'a, GameViewMessage> {
    let GameViewProps {
        skeleton_phase,
        app_theme,
        capsules,
    } = props;
    match state.phase {
        GameViewPhase::Saving => {
            let base = loaded_view(state, skeleton_phase, app_theme, capsules);
            stack![
                base,
                opaque(saving_overlay(
                    state.spinner_angle,
                    "Saving changes...",
                    app_theme
                ))
            ]
            .into()
        }
        GameViewPhase::Connecting | GameViewPhase::WaitingStats | GameViewPhase::Ready => {
            let base = loaded_view(state, skeleton_phase, app_theme, capsules);
            if state.show_apply_modal {
                stack![base, opaque(apply_modal(state, app_theme))].into()
            } else {
                base
            }
        }
        GameViewPhase::Error => error_view(state, app_theme),
    }
}

pub(crate) fn tier_color(tier: RarityTier, theme: crate::ui::theme::AppTheme) -> Color {
    let p = palette(theme);
    match tier {
        RarityTier::Common => p.rarity_common,
        RarityTier::Uncommon => p.rarity_uncommon,
        RarityTier::Rare => p.rarity_rare,
        RarityTier::Mythical => p.rarity_mythical,
        RarityTier::Legendary => p.rarity_legendary,
    }
}

fn error_view(
    state: &GameViewState,
    theme: crate::ui::theme::AppTheme,
) -> Element<'_, GameViewMessage> {
    let p = palette(theme);
    let content = column![
        text("Failed to load").size(20).color(p.severity.error),
        text(&state.error_message).size(13).color(p.text_muted),
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

fn loaded_view<'a>(
    state: &'a GameViewState,
    skeleton_phase: f32,
    app_theme: crate::ui::theme::AppTheme,
    capsules: &'a crate::app_context::CapsuleStore,
) -> Element<'a, GameViewMessage> {
    use crate::game_view::widget::{GameWidgetParams, game_widget};

    let game_widget_el = game_widget(GameWidgetParams {
        app_id: state.app_id,
        game_name: state.game_name.as_str(),
        genre: state.genre.as_deref(),
        playtime_minutes: state.playtime_minutes,
        stats: state.stats.as_slice(),
        stats_search_query: state.stats_search_query.as_str(),
        capsules,
        skeleton_phase,
        hovered_bar_slice: state.hovered_bar_slice,
        summary: state.derived.summary,
        theme: app_theme,
    });
    let body = achievements_tab(state, skeleton_phase, app_theme);

    compose_screen(ScreenContent {
        top: Some(game_widget_el),
        status_bar: game_status_bar(state),
        body,
        footer: Some(footer_bar(state, app_theme)),
    })
}

fn game_status_bar(state: &GameViewState) -> Option<Element<'_, GameViewMessage>> {
    use crate::game_view::GameViewPhase;
    use crate::ui::widgets::status_bar::{StatusContext, derive_status_bar, status_bar};

    let total = state.achievements.len().max(state.expected_total as usize);
    let ready = state
        .achievements
        .iter()
        .filter(|r| {
            if r.is_spoiler_hidden() {
                return true;
            }
            r.data.icon.is_some() && r.rarity_percent.is_some()
        })
        .count();

    if state.cache_only {
        return if total == 0 {
            None
        } else {
            Some(
                status_bar::<GameViewMessage>()
                    .offline(total, "achievements")
                    .into(),
            )
        };
    }

    match state.phase {
        GameViewPhase::Error => None,
        GameViewPhase::Connecting | GameViewPhase::WaitingStats => derive_status_bar(
            StatusContext {
                total: total.max(1),
                noun: "achievements",
                steam_running: None,
                failed: 0,
                offline_cached_count: 0,
                last_sync: None,
            },
            &[("Loading achievements", ready)],
        ),
        GameViewPhase::Ready | GameViewPhase::Saving => derive_status_bar(
            StatusContext {
                total,
                noun: "achievements",
                steam_running: None,
                failed: 0,
                offline_cached_count: 0,
                last_sync: None,
            },
            &[("Loading achievement icons", ready)],
        ),
    }
}
pub(crate) fn build_back_leading() -> Element<'static, crate::Message> {
    button(
        text("\u{2039} Back")
            .size(13)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).accent),
            }),
    )
    .on_press(crate::Message::GoBack)
    .padding(Padding::from([0u16, 0]))
    .style(|_theme, _status| button::Style {
        background: None,
        border: iced::Border::default(),
        ..button::Style::default()
    })
    .into()
}

pub(crate) fn build_game_reload_button() -> Element<'static, crate::Message> {
    use crate::ui::widgets::tooltip_box::tooltip_box;

    let btn =
        button(
            container(text("\u{21BB}").size(16).style(|t: &iced::Theme| {
                iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).accent),
                }
            }))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
        )
        .on_press(crate::Message::GameView(GameViewMessage::ReloadRequested))
        .padding(0)
        .style(|t: &iced::Theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let accent = palette(theme_from_iced(t)).accent;
            button::Style {
                background: Some(iced::Background::Color(Color {
                    a: if hovered { 0.18 } else { 0.10 },
                    ..accent
                })),
                border: iced::Border {
                    color: Color {
                        a: if hovered { 0.55 } else { 0.40 },
                        ..accent
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: accent,
                ..button::Style::default()
            }
        });

    tooltip_box(
        btn,
        "Reload achievements & stats from Steam",
        iced::widget::tooltip::Position::Bottom,
    )
}
