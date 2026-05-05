use std::collections::HashMap;

use iced::widget::{
    button, column, container, image as img_widget, responsive, row, scrollable, text, text_input,
    tooltip,
};
use iced::widget::Id as WidgetId;
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::GameCacheEntry;
use crate::capsule_cache::CapsuleSize;
use crate::skeleton::skeleton_box;
use crate::theme::{
    C_ACCENT, C_ACCENT_DARK, C_APP, C_BORDER, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED,
    C_TEXT_PRIMARY,
};

use super::ProfileViewState;
use super::profile::{compute_profile_summary, profile_widget, top5_closest_to_complete};
use super::types::{
    CapsuleAsset, GameEntry, LibrarySort, ProfileViewMessage, ProfileViewPhase,
};

const CARD_GAP: f32 = 12.0;

const C_PLACEHOLDER: Color = Color::from_rgb(0.188, 0.192, 0.247);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_TEXT: Color = Color::from_rgb(0.973, 0.973, 0.949);

const C_GOLD: Color = Color::from_rgb(1.0, 0.85, 0.4);
const C_PURPLE_BAR: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_MAGENTA_BAR: Color = Color::from_rgb(1.0, 0.4, 0.85);
const C_CYAN_BAR: Color = Color::from_rgb(0.545, 0.914, 0.992);

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

fn total_card_height(capsule_h: f32) -> f32 {
    capsule_h + 32.0 + 4.0 + 8.0 + 9.0
}

pub fn render_with_cache_actions<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
) -> Element<'a, crate::Message> {
    render_inner(state, user_profile, cached_entries, skeleton_phase)
}

fn render_inner<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
) -> Element<'a, crate::Message> {
    let header = build_header(state);

    let profile_section = build_profile_section(state, user_profile, cached_entries);

    let body: Element<'_, crate::Message> = match &state.phase {
        ProfileViewPhase::Scanning => center_text("Scanning library\u{2026}"),
        ProfileViewPhase::Error(e) => error_view(e),
        ProfileViewPhase::Loaded => {
            let visible = state.visible_games();

            if visible.is_empty() {
                center_text("No games found.")
            } else {
                build_grid(state, visible, skeleton_phase)
            }
        }
    };

    let footer = build_footer(state);

    let mut col = column![header];
    col = col.push(profile_section);
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
    profile_widget(
        user_profile,
        &summary,
        top5,
        state.loader_phase(),
        state.loader_hiding_since,
        state.games.len(),
    )
}

fn build_header(state: &ProfileViewState) -> Element<'_, crate::Message> {
    let title_block = build_title_block(state.games.len());
    let search_block = build_search_block(state);
    let sort_block = build_sort_segment(state.sort);
    let size_block = build_size_segment(state.capsule_size);
    let rescan_btn = build_rescan_button();
    let settings_btn = build_icon_button("\u{2699}", "Settings \u{2014} coming soon");
    let about_btn = build_icon_button("\u{24D8}", "About \u{2014} coming soon");

    let header_row = row![
        title_block,
        search_block,
        sort_block,
        size_block,
        rescan_btn,
        settings_btn,
        about_btn,
    ]
    .spacing(12)
    .padding(Padding::default().left(16).right(16).top(12).bottom(12))
    .align_y(Alignment::Center);

    container(header_row)
        .width(Length::Fill)
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn build_title_block(game_count: usize) -> Element<'static, crate::Message> {
    let title = text("Library").size(22).color(C_ACCENT);
    let count = text(format!("{game_count} games"))
        .size(12)
        .color(C_TEXT_DIM);
    row![title, count]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

pub fn library_search_id() -> WidgetId {
    WidgetId::new("library-search")
}

fn build_search_block(state: &ProfileViewState) -> Element<'_, crate::Message> {
    let magnifier = text("\u{1F50D}").size(13).color(C_TEXT_MUTED);

    let input = text_input("Search games\u{2026}", &state.search)
        .id(library_search_id())
        .on_input(|s| crate::Message::ProfileView(ProfileViewMessage::SearchChanged(s)))
        .padding(Padding::default().left(4).right(4).top(6).bottom(6))
        .size(13)
        .style(
            |_theme: &iced::Theme, _status| iced::widget::text_input::Style {
                background: iced::Background::Color(C_SURFACE),
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                icon: C_TEXT_MUTED,
                placeholder: C_TEXT_MUTED,
                value: C_TEXT_PRIMARY,
                selection: Color { a: 0.3, ..C_ACCENT },
            },
        )
        .width(Length::Fill);

    let kbd_badge = container(text("Ctrl K").size(10).color(C_TEXT_DIM))
        .padding(Padding::default().left(6).right(6).top(2).bottom(2))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_BORDER)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        });

    let inner_row = row![magnifier, input, kbd_badge]
        .spacing(6)
        .align_y(Alignment::Center);

    container(inner_row)
        .width(Length::Fill)
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                color: C_BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .padding(Padding::default().left(8).right(8).top(0).bottom(0))
        .into()
}

fn build_sort_segment(current: LibrarySort) -> Element<'static, crate::Message> {
    let label = text("SORT").size(11).color(C_TEXT_MUTED);
    let segment = segment_row(&[
        (
            "Last played",
            current == LibrarySort::LastPlayed,
            crate::Message::ProfileView(ProfileViewMessage::SortChanged(LibrarySort::LastPlayed)),
        ),
        (
            "A\u{2013}Z",
            current == LibrarySort::NameAsc,
            crate::Message::ProfileView(ProfileViewMessage::SortChanged(LibrarySort::NameAsc)),
        ),
    ]);
    row![label, segment]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn build_size_segment(current: CapsuleSize) -> Element<'static, crate::Message> {
    let label = text("SIZE").size(11).color(C_TEXT_MUTED);
    let segment = segment_row(&[
        (
            "S",
            current == CapsuleSize::Small,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(CapsuleSize::Small)),
        ),
        (
            "M",
            current == CapsuleSize::Medium,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(
                CapsuleSize::Medium,
            )),
        ),
        (
            "L",
            current == CapsuleSize::Large,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(CapsuleSize::Large)),
        ),
    ]);
    row![label, segment]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn segment_row(items: &[(&'static str, bool, crate::Message)]) -> Element<'static, crate::Message> {
    let mut r = row![].spacing(0).align_y(Alignment::Center);
    let last_idx = items.len().saturating_sub(1);

    for (idx, (label, active, msg)) in items.iter().enumerate() {
        let active = *active;
        let msg = msg.clone();

        let btn = button(
            text(*label)
                .size(12)
                .color(if active { C_ACCENT } else { C_TEXT_MUTED }),
        )
        .on_press(msg)
        .padding(Padding::default().left(10).right(10).top(6).bottom(6))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            let bg = if active {
                Some(iced::Background::Color(Color {
                    r: C_ACCENT.r,
                    g: C_ACCENT.g,
                    b: C_ACCENT.b,
                    a: 0.15,
                }))
            } else if hovered {
                Some(iced::Background::Color(C_HOVER))
            } else {
                Some(iced::Background::Color(Color::TRANSPARENT))
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                text_color: if active { C_ACCENT } else { C_TEXT_MUTED },
                ..iced::widget::button::Style::default()
            }
        });

        r = r.push(btn);

        if idx < last_idx {
            let divider = container(iced::widget::Space::new().width(1.0).height(20.0))
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(20.0))
                .style(|_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(C_BORDER)),
                    ..container::Style::default()
                });
            r = r.push(divider);
        }
    }

    container(r)
        .style(|_: &iced::Theme| container::Style {
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

fn build_rescan_button() -> Element<'static, crate::Message> {
    button(
        row![
            text("\u{21BB}").size(12).color(C_APP),
            text("Rescan").size(12).color(C_APP),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .on_press(crate::Message::ProfileView(
        ProfileViewMessage::RescanRequested,
    ))
    .padding(Padding::default().left(14).right(14).top(7).bottom(7))
    .style(|_: &iced::Theme, status| {
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        iced::widget::button::Style {
            background: Some(iced::Background::Color(if hovered {
                C_ACCENT_DARK
            } else {
                C_ACCENT
            })),
            border: iced::Border {
                radius: 6.0.into(),
                ..iced::Border::default()
            },
            text_color: C_APP,
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn build_icon_button(
    glyph: &'static str,
    toast_msg: &'static str,
) -> Element<'static, crate::Message> {
    button(
        container(text(glyph).size(14).color(C_TEXT_MUTED))
            .width(Length::Fixed(32.0))
            .height(Length::Fixed(32.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press(crate::Message::ToastRequest(toast_msg.to_owned()))
    .padding(0)
    .style(|_: &iced::Theme, status| {
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        iced::widget::button::Style {
            background: Some(iced::Background::Color(if hovered {
                C_HOVER
            } else {
                Color::TRANSPARENT
            })),
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
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn build_grid<'a>(
    state: &'a ProfileViewState,
    visible: Vec<&'a GameEntry>,
    skeleton_phase: f32,
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
                r = r.push(build_card(
                    entry,
                    capsule_size,
                    actual_card_w,
                    skeleton_phase,
                ));
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
    skeleton_phase: f32,
) -> Element<'_, crate::Message> {
    let app_id = entry.summary.app_id;
    let (capsule_w, capsule_h) = capsule_dims(capsule_size);
    let total_h = total_card_height(capsule_h);

    if !entry.is_hydrated() {
        return build_skeleton_card(entry, card_w, capsule_w, capsule_h, total_h, skeleton_phase);
    }

    build_hydrated_card(entry, app_id, card_w, capsule_w, capsule_h, total_h)
}

fn build_skeleton_card<'a>(
    entry: &'a GameEntry,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
    phase: f32,
) -> Element<'a, crate::Message> {
    let title_width_ratio = match entry.summary.app_id % 5 {
        0 => 0.75,
        1 => 0.60,
        2 => 0.85,
        3 => 0.55,
        _ => 0.70,
    };

    let capsule_skel = skeleton_box(capsule_w, capsule_h, phase);

    let name_skel = skeleton_box(card_w * title_width_ratio, 12.0, phase);

    let progress_skel = skeleton_box(card_w, 3.0, phase);

    let separator_space = iced::widget::Space::new()
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(4.0 + 8.0));

    let name_container = container(name_skel)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(32.0))
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(6).right(6).top(8).bottom(8));

    let card_inner =
        column![capsule_skel, separator_space, name_container, progress_skel,].spacing(0);

    container(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_h))
        .padding(Padding::default().top(8))
        .into()
}

fn build_hydrated_card<'a>(
    entry: &'a GameEntry,
    app_id: u32,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
) -> Element<'a, crate::Message> {
    let capsule_area: Element<'_, crate::Message> = match &entry.capsule {
        CapsuleAsset::Loaded { handle, .. } => container(
            container(
                img_widget(handle.clone())
                    .width(Length::Fixed(capsule_w))
                    .height(Length::Fixed(capsule_h)),
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

        CapsuleAsset::Pending => container(iced::widget::Space::new())
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

        CapsuleAsset::Unavailable => container(text("no image").size(10).color(C_MUTED))
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
        .height(Length::Fixed(total_h))
        .padding(Padding::default().top(8));

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
