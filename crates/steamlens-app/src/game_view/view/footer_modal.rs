use iced::widget::{button, column, container, row, space, stack, text, text_input};
use iced::{Alignment, Color, Element, Length, Padding};

use super::grid::bulk_action_buttons;
use super::{C_MUTED, dracula_border_radius};
use crate::game_view::{GameViewMessage, GameViewPhase, GameViewState};
use crate::ui::theme::{palette, theme_from_iced};

pub(super) fn footer_bar(
    state: &GameViewState,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'_, GameViewMessage> {
    let p_outer = *palette(app_theme);
    let dirty = state.dirty_count();
    let has_errors = state.has_stat_errors();
    let is_busy = matches!(state.phase, GameViewPhase::Saving);

    let cancel_label = if dirty > 0 {
        format!(
            "Cancel  {dirty} change{}",
            if dirty == 1 { "" } else { "s" }
        )
    } else {
        "Cancel".to_owned()
    };

    let cancel_btn = if dirty > 0 && !is_busy {
        button(
            text(cancel_label)
                .size(12)
                .style(|t: &iced::Theme| iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).text_primary),
                }),
        )
        .on_press(GameViewMessage::DiscardChanges)
        .padding(Padding::default().left(12).right(12).top(6).bottom(6))
        .style(|t: &iced::Theme, status| {
            let p = palette(theme_from_iced(t));
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: if hovered {
                    Some(iced::Background::Color(Color { a: 0.2, ..C_MUTED }))
                } else {
                    None
                },
                border: iced::Border {
                    color: C_MUTED,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: p.text_primary,
                ..button::Style::default()
            }
        })
    } else {
        button(
            text(cancel_label)
                .size(12)
                .style(|t: &iced::Theme| iced::widget::text::Style {
                    color: Some(Color {
                        a: 0.4,
                        ..palette(theme_from_iced(t)).text_primary
                    }),
                }),
        )
        .padding(Padding::default().left(12).right(12).top(6).bottom(6))
        .style(|t: &iced::Theme, _status| {
            let p = palette(theme_from_iced(t));
            button::Style {
                background: None,
                border: iced::Border {
                    color: Color { a: 0.3, ..C_MUTED },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: Color {
                    a: 0.4,
                    ..p.text_primary
                },
                ..button::Style::default()
            }
        })
    };

    let apply_label = if dirty > 0 {
        format!("Apply  {dirty} change{}", if dirty == 1 { "" } else { "s" })
    } else {
        "Apply Changes".to_owned()
    };

    let apply_enabled = dirty > 0 && !has_errors && !is_busy && !state.cache_only;
    let apply_btn = if apply_enabled {
        button(text(apply_label).size(12))
            .on_press(GameViewMessage::ApplyClicked)
            .padding(Padding::default().left(12).right(12).top(6).bottom(6))
            .style(move |t: &iced::Theme, status| {
                let p = palette(theme_from_iced(t));
                let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
                let bg_color = if hovered {
                    Color {
                        r: (p.accent.r * 0.9 + 0.1).min(1.0),
                        g: (p.accent.g * 0.9 + 0.1).min(1.0),
                        b: (p.accent.b * 0.9 + 0.1).min(1.0),
                        a: 1.0,
                    }
                } else {
                    p.accent
                };
                button::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: dracula_border_radius(6.0),
                    text_color: Color::BLACK,
                    shadow: if hovered {
                        iced::Shadow {
                            color: Color { a: 0.6, ..p.accent },
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
        button(text(apply_label).size(12))
            .padding(Padding::default().left(12).right(12).top(6).bottom(6))
            .style(|t: &iced::Theme, _s| {
                let p = palette(theme_from_iced(t));
                button::Style {
                    background: Some(iced::Background::Color(Color { a: 0.3, ..p.accent })),
                    border: dracula_border_radius(6.0),
                    text_color: Color {
                        a: 0.4,
                        ..p.text_primary
                    },
                    ..button::Style::default()
                }
            })
    };

    let spinner_el: Element<'_, GameViewMessage> = if is_busy {
        text(spinner_frame(state.spinner_angle))
            .size(16)
            .color(p_outer.accent)
            .into()
    } else {
        space().width(20).into()
    };

    let bulk_buttons = bulk_action_buttons();

    let vert_divider = container(space())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(28.0))
        .style(|t: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(palette(theme_from_iced(t)).border)),
            ..container::Style::default()
        });

    row![
        bulk_buttons,
        space().width(Length::Fill),
        vert_divider,
        spinner_el,
        cancel_btn,
        apply_btn
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn apply_modal(
    state: &GameViewState,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'_, GameViewMessage> {
    let p = *palette(app_theme);
    let dirty = state.dirty_count();
    let dirty_label = format!(
        "You are about to commit {dirty} pending change{} to Steam.",
        if dirty == 1 { "" } else { "s" }
    );

    let warning_box = container(
        column![
            text("\u{26A0} This writes directly to Steam")
                .size(13)
                .color(p.severity.warning),
            text(
                "Stats and achievements will be persisted via Steam's stats API \
                 and become visible on your profile immediately. Use Cancel to \
                 keep your changes staged locally without committing."
            )
            .size(12)
            .color(Color {
                a: 0.90,
                ..p.text_primary
            }),
        ]
        .spacing(4),
    )
    .padding(Padding::from([8u16, 12]))
    .style(move |_theme| container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.08,
            ..p.severity.warning
        })),
        border: iced::Border {
            color: p.severity.warning,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    });

    let confirm_input_label = text("Type \"confirmed\" to apply:").size(12).color(C_MUTED);

    let confirm_input = text_input("confirmed", &state.apply_confirm_input)
        .on_input(GameViewMessage::ApplyConfirmInputChanged)
        .on_submit(GameViewMessage::ApplyConfirmed)
        .size(13)
        .padding(Padding::from([6u16, 10]))
        .style(move |_theme, _status| iced::widget::text_input::Style {
            background: iced::Background::Color(p.app),
            border: iced::Border {
                color: C_MUTED,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: C_MUTED,
            placeholder: Color { a: 0.3, ..C_MUTED },
            value: p.text_primary,
            selection: Color {
                a: 0.35,
                ..p.accent
            },
        });

    let confirm_gate = column![confirm_input_label, confirm_input].spacing(4);

    let confirm_enabled = state.apply_confirm_matches();

    let confirm_btn = {
        let base = button(text("Apply Changes").size(13).color(if confirm_enabled {
            Color::BLACK
        } else {
            Color {
                a: 0.4,
                ..Color::BLACK
            }
        }))
        .padding(Padding::from([8u16, 16]))
        .style(move |_t, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let bg = if !confirm_enabled {
                Color {
                    a: 0.30,
                    ..p.accent
                }
            } else if hovered {
                Color {
                    r: (p.accent.r * 0.9 + 0.1).min(1.0),
                    g: (p.accent.g * 0.9 + 0.1).min(1.0),
                    b: (p.accent.b * 0.9 + 0.1).min(1.0),
                    a: 1.0,
                }
            } else {
                p.accent
            };
            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: dracula_border_radius(4.0),
                text_color: Color::BLACK,
                ..button::Style::default()
            }
        });
        if confirm_enabled {
            base.on_press(GameViewMessage::ApplyConfirmed)
        } else {
            base
        }
    };

    let cancel_btn = button(text("Cancel").size(13).color(p.text_primary))
        .on_press(GameViewMessage::ApplyCancelled)
        .padding(Padding::from([8u16, 16]))
        .style(move |_t, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(iced::Background::Color(if hovered {
                    Color {
                        r: (p.border.r * 0.85 + 0.18).min(1.0),
                        g: (p.border.g * 0.85 + 0.18).min(1.0),
                        b: (p.border.b * 0.85 + 0.18).min(1.0),
                        a: 1.0,
                    }
                } else {
                    p.border
                })),
                border: iced::Border {
                    color: if hovered {
                        Color { a: 0.40, ..C_MUTED }
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: p.text_primary,
                ..button::Style::default()
            }
        });

    let button_row = row![cancel_btn, space().width(Length::Fill), confirm_btn]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().top(16));

    let modal_inner = column![
        text("\u{26A0}  Confirm Apply")
            .size(16)
            .color(p.text_primary),
        text(dirty_label).size(13).color(C_MUTED),
        warning_box,
        confirm_gate,
        button_row,
    ]
    .spacing(12)
    .padding(Padding::from(24u16));

    let modal_box = container(modal_inner)
        .width(Length::Fixed(480.0))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(p.surface)),
            border: iced::Border {
                color: p.border,
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

pub(super) fn saving_overlay<'a>(
    angle: f32,
    label: &'a str,
    app_theme: crate::ui::theme::AppTheme,
) -> Element<'a, GameViewMessage> {
    let p = palette(app_theme);
    let spinner = text(spinner_frame(angle)).size(24).color(p.accent);

    let content = column![spinner, text(label).size(14).color(p.text_primary)]
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

pub(super) fn spinner_frame(angle: f32) -> &'static str {
    let frames = ["\u{25F4}", "\u{25F7}", "\u{25F6}", "\u{25F5}"];
    let idx = ((angle / 90.0) as usize) % frames.len();
    frames[idx]
}
