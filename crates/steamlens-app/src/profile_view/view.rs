use std::collections::HashMap;

use iced::widget::{
    button, column, container, image as img_widget, pick_list, responsive, row, scrollable, text,
    text_input, tooltip,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::GameCacheEntry;
use crate::capsule_cache::CapsuleSize;

use super::ProfileViewState;
use super::profile::{compute_profile_summary, profile_widget, top5_closest_to_complete};
use super::types::{
    CapsuleState, GameEntry, LibrarySort, LoaderPhase, ProfileViewMessage, ProfileViewPhase,
};

const CARD_GAP: f32 = 12.0;

const C_SURFACE: Color = Color::from_rgb(0.267, 0.278, 0.353);
const C_PLACEHOLDER: Color = Color::from_rgb(0.188, 0.192, 0.247);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_TEXT: Color = Color::from_rgb(0.973, 0.973, 0.949);
const C_ACCENT: Color = Color::from_rgb(0.741, 0.576, 0.976);

const C_GOLD: Color = Color::from_rgb(1.0, 0.85, 0.4);
const C_PURPLE_BAR: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_MAGENTA_BAR: Color = Color::from_rgb(1.0, 0.4, 0.85);
const C_CYAN_BAR: Color = Color::from_rgb(0.545, 0.914, 0.992);

fn spinner_frame(angle: f32) -> &'static str {
    let frames = ["\u{25F4}", "\u{25F7}", "\u{25F6}", "\u{25F5}"];
    let idx = ((angle / 90.0) as usize) % frames.len();
    frames[idx]
}

fn capsule_dims(size: CapsuleSize) -> (f32, f32) {
    match size {
        CapsuleSize::Small => (120.0, 45.0),
        CapsuleSize::Medium => (231.0, 87.0),
        CapsuleSize::Large => (460.0, 215.0),
    }
}

fn card_width(size: CapsuleSize) -> f32 {
    let (capsule_w, _) = capsule_dims(size);
    capsule_w + 16.0
}

pub fn render_with_cache_actions<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
) -> Element<'a, crate::Message> {
    render_inner(state, true, user_profile, cached_entries)
}

fn render_inner<'a>(
    state: &'a ProfileViewState,
    show_cache_actions: bool,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
) -> Element<'a, crate::Message> {
    let header = if show_cache_actions {
        build_header_with_cache(state)
    } else {
        build_header(state)
    };

    let profile_section = build_profile_section(state, user_profile, cached_entries);

    let body: Element<'_, crate::Message> = match &state.phase {
        ProfileViewPhase::Scanning => center_text("Scanning library…"),
        ProfileViewPhase::Error(e) => error_view(e),
        ProfileViewPhase::Loaded => {
            let visible: Vec<&GameEntry> = state
                .visible_games()
                .into_iter()
                .filter(|g| g.progress.is_some())
                .collect();

            if visible.is_empty() && !state.games.is_empty() {
                center_text("Loading achievement data…")
            } else if visible.is_empty() {
                center_text("No games found.")
            } else {
                build_grid(state, visible)
            }
        }
    };

    let loader = build_unified_loader(state);
    let stream_indicator = build_stream_indicator(state);
    let footer = build_footer(state);

    let mut col = column![header];
    col = col.push(profile_section);
    if let Some(loader_el) = loader {
        col = col.push(loader_el);
    }
    if let Some(indicator) = stream_indicator {
        col = col.push(indicator);
    }
    col = col.push(body).push(footer);

    col.spacing(0).into()
}

fn build_profile_section<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
) -> Element<'a, crate::Message> {
    let summary = compute_profile_summary(cached_entries);
    let top5 = top5_closest_to_complete(&state.games, cached_entries);
    profile_widget(user_profile, &summary, top5, &state.capsule_handles)
}

/// Unified 3-phase loader strip shown below the profile widget and above the
/// games grid.  Returns `None` only when the loader has finished its γ fade-out
/// (elapsed >= 300 ms), causing it to be fully unmounted from the layout.
///
/// - Phase α (Alpha): no games discovered yet — indeterminate 3-dot pulse.
/// - Phase β (Beta): games loading — determinate fill bar with game count.
/// - Phase γ (Gamma): all games have progress — fades out over 300 ms.
fn build_unified_loader<'a>(state: &'a ProfileViewState) -> Option<Element<'a, crate::Message>> {
    let phase = state.loader_phase();

    let alpha_opacity = match phase {
        LoaderPhase::Gamma => {
            let elapsed = state
                .loader_hiding_since
                .map(|t| t.elapsed().as_millis())
                .unwrap_or(0);
            if elapsed >= 300 {
                return None;
            }
            let progress = elapsed as f32 / 300.0;
            1.0 - progress
        }
        _ => 1.0,
    };

    let loader_content: Element<'_, crate::Message> = match phase {
        LoaderPhase::Alpha => {
            let pulse = state.loader_pulse_phase;
            let pulse_dots = build_pulse_dots(pulse);
            row![
                pulse_dots,
                text("Discovering your library…").size(12).color(Color {
                    a: alpha_opacity,
                    ..C_MUTED
                }),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }

        LoaderPhase::Beta { loaded, total } => {
            let frac = if total > 0 {
                loaded as f32 / total as f32
            } else {
                0.0
            };
            let bar = build_determinate_bar(frac, 200.0, alpha_opacity);
            row![
                bar,
                text(format!("{loaded} / {total} games loaded"))
                    .size(12)
                    .color(Color {
                        a: alpha_opacity,
                        ..C_MUTED
                    }),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }

        LoaderPhase::Gamma => {
            let bar = build_determinate_bar(1.0, 200.0, alpha_opacity);
            row![
                bar,
                text("Library ready").size(12).color(Color {
                    a: alpha_opacity,
                    ..C_MUTED
                }),
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
        }
    };

    let banner = container(loader_content)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(6).bottom(6))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(Color::from_rgba(
                0.18,
                0.14,
                0.28,
                0.6 * alpha_opacity,
            ))),
            border: iced::Border {
                color: Color::from_rgba(0.741, 0.576, 0.976, 0.3 * alpha_opacity),
                width: 0.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        });

    Some(banner.into())
}

fn build_pulse_dots<'a>(pulse: f32) -> Element<'a, crate::Message> {
    let dot_count = 3usize;
    let mut dots_row = row![].spacing(4).align_y(Alignment::Center);

    for i in 0..dot_count {
        let offset = i as f32 / dot_count as f32;
        let phase = (pulse + offset) % 1.0;
        let brightness = if phase < 0.5 {
            0.4 + phase * 1.2
        } else {
            1.0 - (phase - 0.5) * 1.2
        };
        let brightness = brightness.clamp(0.4, 1.0);
        let dot_color = Color {
            r: C_ACCENT.r * brightness,
            g: C_ACCENT.g * brightness,
            b: C_ACCENT.b * brightness,
            a: 1.0,
        };
        let dot = container(iced::widget::Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(dot_color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });
        dots_row = dots_row.push(dot);
    }

    dots_row.into()
}

fn build_determinate_bar<'a>(
    fraction: f32,
    width: f32,
    opacity: f32,
) -> Element<'a, crate::Message> {
    let fill_w = (fraction.clamp(0.0, 1.0) * width).max(0.0);
    let bar_color = Color {
        a: opacity,
        ..C_ACCENT
    };

    let fill = container(iced::widget::Space::new())
        .width(Length::Fixed(fill_w))
        .height(Length::Fixed(4.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(bar_color)),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let track_color = Color::from_rgba(0.3, 0.3, 0.35, 0.5 * opacity);

    let track = container(fill)
        .width(Length::Fixed(width))
        .height(Length::Fixed(4.0))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(track_color)),
            border: iced::Border {
                radius: 2.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    track.into()
}

fn build_stream_indicator(state: &ProfileViewState) -> Option<Element<'_, crate::Message>> {
    if !state.is_streaming() {
        return None;
    }
    let total = state.games.len();
    let revealed = state.games.iter().filter(|g| g.revealed).count();

    let indicator_row = row![
        text(spinner_frame(state.spinner_angle))
            .size(13)
            .color(C_MUTED),
        text(format!("Loading {revealed} / {total} games…"))
            .size(12)
            .color(C_MUTED),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .padding(Padding::default().left(16).right(16).top(4).bottom(4));

    Some(container(indicator_row).width(Length::Fill).into())
}

fn build_header(state: &ProfileViewState) -> Element<'_, crate::Message> {
    build_header_inner(state, false)
}

fn build_header_with_cache(state: &ProfileViewState) -> Element<'_, crate::Message> {
    build_header_inner(state, true)
}

fn build_header_inner(
    state: &ProfileViewState,
    show_cache_actions: bool,
) -> Element<'_, crate::Message> {
    let title = text("Library").size(22).color(C_ACCENT);

    let search = text_input("Search games…", &state.search)
        .on_input(|s| crate::Message::ProfileView(ProfileViewMessage::SearchChanged(s)))
        .padding(8)
        .size(13)
        .width(Length::Fixed(260.0));

    let sort_label = text("Sort:").size(13).color(C_MUTED);
    let sort_pick = pick_list(
        &[LibrarySort::LastPlayed, LibrarySort::NameAsc][..],
        Some(state.sort),
        |s| crate::Message::ProfileView(ProfileViewMessage::SortChanged(s)),
    )
    .text_size(13);

    let size_label = text("Size:").size(13).color(C_MUTED);
    let size_pick = pick_list(
        &[CapsuleSize::Small, CapsuleSize::Medium, CapsuleSize::Large][..],
        Some(state.capsule_size),
        |s| crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(s)),
    )
    .text_size(13);

    let rescan_btn = button(text("Rescan").size(12))
        .on_press(crate::Message::ProfileView(
            ProfileViewMessage::RescanRequested,
        ))
        .padding(Padding::default().left(10).right(10).top(6).bottom(6));

    let mut right_controls = row![sort_label, sort_pick, size_label, size_pick, rescan_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    if show_cache_actions {
        let clear_btn = button(text("Clear cache").size(11).color(C_MUTED))
            .on_press(crate::Message::ClearAllCache)
            .padding(Padding::default().left(8).right(8).top(4).bottom(4))
            .style(|_theme, status| {
                let hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: if hovered {
                        Some(iced::Background::Color(Color { a: 0.12, ..C_MUTED }))
                    } else {
                        None
                    },
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });
        right_controls = right_controls.push(clear_btn);
    }

    let header_row = row![
        title,
        iced::widget::Space::new().width(Length::Fill),
        search,
        right_controls,
    ]
    .spacing(12)
    .padding(Padding::default().left(16).right(16).top(12).bottom(12))
    .align_y(Alignment::Center);

    container(header_row)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(Color {
                    r: palette.background.r * 0.85,
                    g: palette.background.g * 0.85,
                    b: palette.background.b * 0.85,
                    a: 1.0,
                })),
                ..container::Style::default()
            }
        })
        .into()
}

fn build_grid<'a>(
    state: &'a ProfileViewState,
    visible: Vec<&'a GameEntry>,
) -> Element<'a, crate::Message> {
    let capsule_size = state.capsule_size;
    let card_w = card_width(capsule_size);

    let entries: Vec<&'a GameEntry> = visible;

    let grid = responsive(move |size| {
        const SIDE_PADDING: f32 = 16.0;
        let inner = (size.width - SIDE_PADDING * 2.0).max(card_w);
        let cols = ((inner + CARD_GAP) / (card_w + CARD_GAP)).floor().max(1.0) as usize;
        let actual_card_w =
            ((inner - CARD_GAP * (cols.saturating_sub(1)) as f32) / cols as f32).max(card_w);

        let mut rows_col: iced::widget::Column<'_, crate::Message> = column![]
            .spacing(CARD_GAP as u32)
            .padding(Padding::default().left(16).right(16).top(8).bottom(8));

        for chunk in entries.chunks(cols) {
            let mut r: iced::widget::Row<'_, crate::Message> = row![].spacing(CARD_GAP as u32);
            for entry in chunk {
                r = r.push(build_card(entry, capsule_size, actual_card_w));
            }
            let needed = cols - chunk.len();
            for _ in 0..needed {
                r = r.push(iced::widget::Space::new().width(Length::Fixed(actual_card_w)));
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

fn progress_bar_color(ratio: f32) -> Color {
    if ratio >= 0.9 {
        C_CYAN_BAR
    } else if ratio >= 0.5 {
        Color {
            r: C_MAGENTA_BAR.r * (1.0 - (ratio - 0.5) / 0.4) + C_CYAN_BAR.r * ((ratio - 0.5) / 0.4),
            g: C_MAGENTA_BAR.g * (1.0 - (ratio - 0.5) / 0.4) + C_CYAN_BAR.g * ((ratio - 0.5) / 0.4),
            b: C_MAGENTA_BAR.b * (1.0 - (ratio - 0.5) / 0.4) + C_CYAN_BAR.b * ((ratio - 0.5) / 0.4),
            a: 1.0,
        }
    } else {
        Color {
            r: C_PURPLE_BAR.r * (1.0 - ratio / 0.5) + C_MAGENTA_BAR.r * (ratio / 0.5),
            g: C_PURPLE_BAR.g * (1.0 - ratio / 0.5) + C_MAGENTA_BAR.g * (ratio / 0.5),
            b: C_PURPLE_BAR.b * (1.0 - ratio / 0.5) + C_MAGENTA_BAR.b * (ratio / 0.5),
            a: 1.0,
        }
    }
}

fn build_card(
    entry: &GameEntry,
    capsule_size: CapsuleSize,
    card_w: f32,
) -> Element<'_, crate::Message> {
    let app_id = entry.summary.app_id;
    let (capsule_w, capsule_h) = capsule_dims(capsule_size);

    let capsule_area: Element<'_, crate::Message> = match &entry.capsule {
        CapsuleState::Loaded {
            handle, opacity, ..
        } => container(
            container(
                img_widget(handle.clone())
                    .width(Length::Fixed(capsule_w))
                    .height(Length::Fixed(capsule_h))
                    .opacity(*opacity),
            )
            .style(|_: &iced::Theme| container::Style {
                shadow: iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                    offset: iced::Vector::new(2.0, 2.0),
                    blur_radius: 4.0,
                },
                ..container::Style::default()
            }),
        )
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h))
        .align_x(Alignment::Center)
        .into(),

        CapsuleState::Pending => container(iced::widget::Space::new())
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(capsule_h))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_PLACEHOLDER)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into(),

        CapsuleState::Unavailable => container(text("no image").size(10).color(C_MUTED))
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(capsule_h))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_PLACEHOLDER)),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into(),
    };

    let progress_overlay = build_progress_overlay(entry, card_w, capsule_h);

    let capsule_stack = iced::widget::stack![capsule_area, progress_overlay];

    let name_label = container(
        text(entry.summary.name.as_str())
            .size(12)
            .color(C_TEXT)
            .wrapping(text::Wrapping::Word)
            .line_height(text::LineHeight::Relative(1.2)),
    )
    .width(Length::Fixed(card_w))
    .height(Length::Fixed(32.0))
    .align_x(Alignment::Start)
    .align_y(Alignment::End)
    .padding(Padding::default().left(6).right(6).top(0).bottom(4));

    let total_card_h = capsule_h + 32.0 + 4.0 + 8.0 + 9.0;

    let separator = container(iced::widget::rule::horizontal(1))
        .padding(Padding::default().left(8).right(8).top(8).bottom(0))
        .width(Length::Fixed(card_w));

    let card_inner = column![
        capsule_stack,
        separator,
        iced::widget::Space::new().height(Length::Fill),
        name_label,
    ]
    .spacing(0);

    let card = container(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_card_h))
        .padding(Padding::default().top(8));

    let has_progress = entry.progress.is_some();
    let is_gold = entry
        .progress
        .as_ref()
        .is_some_and(|p| p.total > 0 && p.earned >= p.total);
    let tooltip_earned = entry.progress.as_ref().map(|p| p.earned).unwrap_or(0);
    let tooltip_total = entry.progress.as_ref().map(|p| p.total).unwrap_or(0);
    let tooltip_pct = if tooltip_total > 0 {
        (tooltip_earned as f32 / tooltip_total as f32 * 100.0) as u32
    } else {
        0
    };

    let card_btn = button(card)
        .padding(0)
        .on_press(crate::Message::ProfileView(
            ProfileViewMessage::GameSelected(app_id),
        ))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);

            let bg = if hovered {
                Color {
                    r: (C_SURFACE.r * 1.18).min(1.0),
                    g: (C_SURFACE.g * 1.18).min(1.0),
                    b: (C_SURFACE.b * 1.18).min(1.0),
                    a: 1.0,
                }
            } else {
                C_SURFACE
            };

            let border_color = if is_gold && hovered {
                C_GOLD
            } else if hovered {
                C_ACCENT
            } else if is_gold {
                Color { a: 0.5, ..C_GOLD }
            } else {
                Color::TRANSPARENT
            };

            let border_width = if hovered || is_gold { 2.0 } else { 0.0 };

            let shadow = if is_gold {
                iced::Shadow {
                    color: Color::from_rgba(
                        C_GOLD.r,
                        C_GOLD.g,
                        C_GOLD.b,
                        if hovered { 0.5 } else { 0.25 },
                    ),
                    offset: iced::Vector::new(0.0, 0.0),
                    blur_radius: if hovered { 14.0 } else { 6.0 },
                }
            } else if hovered {
                iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 18.0,
                }
            } else {
                iced::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                    offset: iced::Vector::new(0.0, 4.0),
                    blur_radius: 10.0,
                }
            };

            button::Style {
                background: Some(iced::Background::Color(bg)),
                border: iced::Border {
                    color: border_color,
                    width: border_width,
                    radius: 6.0.into(),
                },
                shadow,
                ..button::Style::default()
            }
        });

    if !has_progress {
        card_btn.into()
    } else {
        tooltip(
            card_btn,
            container(
                text(format!(
                    "{tooltip_earned} / {tooltip_total} achievements ({tooltip_pct}%)"
                ))
                .size(11)
                .color(C_TEXT),
            )
            .padding(Padding::default().left(8).right(8).top(4).bottom(4))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.15, 0.15, 0.2, 0.95,
                ))),
                border: iced::Border {
                    color: Color { a: 0.5, ..C_ACCENT },
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..container::Style::default()
            }),
            tooltip::Position::Bottom,
        )
        .into()
    }
}

fn build_progress_overlay<'a>(
    entry: &'a GameEntry,
    card_w: f32,
    capsule_h: f32,
) -> Element<'a, crate::Message> {
    const BAR_H: f32 = 3.0;

    let Some(progress) = entry.progress.as_ref() else {
        return iced::widget::Space::new()
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(capsule_h))
            .into();
    };

    let bar_element: Element<'_, crate::Message> = if progress.total == 0 {
        iced::widget::Space::new().into()
    } else if progress.earned == 0 {
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(BAR_H))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.47, 0.47, 0.51, 0.5,
                ))),
                border: iced::Border {
                    radius: 1.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else if progress.earned >= progress.total {
        container(iced::widget::Space::new())
            .width(Length::Fill)
            .height(Length::Fixed(BAR_H))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_GOLD)),
                border: iced::Border {
                    radius: 1.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else {
        let ratio = progress.earned as f32 / progress.total as f32;
        let bar_color = progress_bar_color(ratio);
        let fill_w = (ratio * card_w).max(2.0);

        let fill = container(iced::widget::Space::new())
            .width(Length::Fixed(fill_w))
            .height(Length::Fixed(BAR_H))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(bar_color)),
                border: iced::Border {
                    radius: 1.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let track = container(
            container(fill)
                .width(Length::Fixed(card_w))
                .height(Length::Fixed(BAR_H))
                .style(|_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(
                        0.2, 0.2, 0.25, 0.4,
                    ))),
                    border: iced::Border {
                        radius: 1.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
        );

        track.into()
    };

    let spacer = iced::widget::Space::new()
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h - BAR_H));

    container(column![spacer, bar_element].spacing(0))
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h))
        .into()
}

fn build_footer(state: &ProfileViewState) -> Element<'_, crate::Message> {
    let id_input = text_input("App ID (e.g. 105600)", &state.manual_app_id_input)
        .on_input(|s| crate::Message::ProfileView(ProfileViewMessage::ManualAppIdChanged(s)))
        .on_submit(crate::Message::ProfileView(
            ProfileViewMessage::ManualAppIdSubmitted,
        ))
        .padding(8)
        .size(13)
        .width(Length::Fixed(180.0));

    let can_open = state
        .manual_app_id_input
        .parse::<u32>()
        .map(|id| id > 0)
        .unwrap_or(false);

    let open_btn = if can_open {
        button(text("Open").size(13))
            .on_press(crate::Message::ProfileView(
                ProfileViewMessage::ManualAppIdSubmitted,
            ))
            .padding(Padding::default().left(14).right(14).top(8).bottom(8))
    } else {
        button(text("Open").size(13))
            .padding(Padding::default().left(14).right(14).top(8).bottom(8))
    };

    let hint = text("Open by App ID:").size(12).color(C_MUTED);

    let footer_row = row![hint, id_input, open_btn]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(16).right(16).top(10).bottom(10));

    container(footer_row)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let palette = theme.palette();
            container::Style {
                background: Some(iced::Background::Color(Color {
                    r: palette.background.r * 0.85,
                    g: palette.background.g * 0.85,
                    b: palette.background.b * 0.85,
                    a: 1.0,
                })),
                ..container::Style::default()
            }
        })
        .into()
}

fn center_text(msg: &str) -> Element<'_, crate::Message> {
    container(text(msg).size(14).color(C_MUTED))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_view(msg: &str) -> Element<'_, crate::Message> {
    let content = column![
        text("Library scan failed").size(18).color(C_ACCENT),
        text(msg).size(13).color(C_TEXT),
        button(text("Retry").size(13))
            .on_press(crate::Message::ProfileView(
                ProfileViewMessage::RescanRequested,
            ))
            .padding(Padding::default().left(14).right(14).top(8).bottom(8)),
    ]
    .spacing(12)
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}
