use iced::widget::{
    button, column, container, image as img_widget, pick_list, responsive, row, scrollable, text,
    text_input,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::capsule_cache::CapsuleSize;

use super::LibraryState;
use super::types::{CapsuleState, GameEntry, LibraryMessage, LibraryPhase, LibrarySort};

const CARD_GAP: f32 = 12.0;

const C_SURFACE: Color = Color::from_rgb(0.267, 0.278, 0.353);
const C_PLACEHOLDER: Color = Color::from_rgb(0.188, 0.192, 0.247);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_TEXT: Color = Color::from_rgb(0.973, 0.973, 0.949);
const C_ACCENT: Color = Color::from_rgb(0.741, 0.576, 0.976);
const C_WARNING: Color = Color::from_rgb(0.545, 0.914, 0.992);

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

pub fn render(state: &LibraryState) -> Element<'_, crate::Message> {
    let header = build_header(state);

    let body: Element<'_, crate::Message> = match &state.phase {
        LibraryPhase::Scanning => center_text("Scanning library…"),
        LibraryPhase::Error(e) => error_view(e),
        LibraryPhase::Loaded => {
            let visible = state.visible_games();
            if visible.is_empty() {
                center_text("No games found.")
            } else {
                build_grid(state, visible)
            }
        }
    };

    let alpha_banner: Option<Element<'_, crate::Message>> = if state.has_opened_a_game {
        let banner = container(
            text("Switching games requires restart (alpha limitation).")
                .size(12)
                .color(C_WARNING),
        )
        .padding(Padding::default().left(16).right(16).top(6).bottom(6))
        .width(Length::Fill);
        Some(banner.into())
    } else {
        None
    };

    let stream_indicator = build_stream_indicator(state);
    let footer = build_footer(state);

    let mut col = column![header];
    if let Some(banner) = alpha_banner {
        col = col.push(banner);
    }
    if let Some(indicator) = stream_indicator {
        col = col.push(indicator);
    }
    col = col.push(body).push(footer);

    col.spacing(0).into()
}

fn build_stream_indicator(state: &LibraryState) -> Option<Element<'_, crate::Message>> {
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

fn build_header(state: &LibraryState) -> Element<'_, crate::Message> {
    let title = text("Library").size(22).color(C_ACCENT);

    let search = text_input("Search games…", &state.search)
        .on_input(|s| crate::Message::Library(LibraryMessage::SearchChanged(s)))
        .padding(8)
        .size(13)
        .width(Length::Fixed(260.0));

    let sort_label = text("Sort:").size(13).color(C_MUTED);
    let sort_pick = pick_list(
        &[LibrarySort::LastPlayed, LibrarySort::NameAsc][..],
        Some(state.sort),
        |s| crate::Message::Library(LibraryMessage::SortChanged(s)),
    )
    .text_size(13);

    let size_label = text("Size:").size(13).color(C_MUTED);
    let size_pick = pick_list(
        &[CapsuleSize::Small, CapsuleSize::Medium, CapsuleSize::Large][..],
        Some(state.capsule_size),
        |s| crate::Message::Library(LibraryMessage::CapsuleSizeChanged(s)),
    )
    .text_size(13);

    let rescan_btn = button(text("Rescan").size(12))
        .on_press(crate::Message::Library(LibraryMessage::RescanRequested))
        .padding(Padding::default().left(10).right(10).top(6).bottom(6));

    let right_controls = row![sort_label, sort_pick, size_label, size_pick, rescan_btn]
        .spacing(8)
        .align_y(Alignment::Center);

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
    state: &'a LibraryState,
    visible: Vec<&'a GameEntry>,
) -> Element<'a, crate::Message> {
    let capsule_size = state.capsule_size;
    let card_w = card_width(capsule_size);

    let entries: Vec<&'a GameEntry> = visible;

    let grid = responsive(move |size| {
        let available = size.width.max(card_w + CARD_GAP);
        let cols = ((available + CARD_GAP) / (card_w + CARD_GAP))
            .floor()
            .max(1.0) as usize;

        let mut rows_col: iced::widget::Column<'_, crate::Message> = column![]
            .spacing(CARD_GAP as u32)
            .padding(Padding::default().left(16).right(16).top(8).bottom(8));

        for chunk in entries.chunks(cols) {
            let mut r: iced::widget::Row<'_, crate::Message> = row![].spacing(CARD_GAP as u32);
            for entry in chunk {
                r = r.push(build_card(entry, capsule_size));
            }
            let needed = cols - chunk.len();
            for _ in 0..needed {
                r = r.push(iced::widget::Space::new().width(Length::Fixed(card_w)));
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

fn build_card(entry: &GameEntry, capsule_size: CapsuleSize) -> Element<'_, crate::Message> {
    let app_id = entry.summary.app_id;
    let (capsule_w, capsule_h) = capsule_dims(capsule_size);
    let card_w = card_width(capsule_size);

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
        capsule_area,
        separator,
        iced::widget::Space::new().height(Length::Fill),
        name_label,
    ]
    .spacing(0);

    let card = container(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_card_h))
        .padding(Padding::default().top(8))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SURFACE)),
            border: iced::Border {
                radius: 6.0.into(),
                ..iced::Border::default()
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 10.0,
            },
            ..container::Style::default()
        });

    button(card)
        .on_press(crate::Message::Library(LibraryMessage::GameSelected(
            app_id,
        )))
        .padding(0)
        .style(|_: &iced::Theme, status| {
            let border = if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                iced::Border {
                    color: C_ACCENT,
                    width: 2.0,
                    radius: 6.0.into(),
                }
            } else {
                iced::Border::default()
            };
            button::Style {
                background: None,
                border,
                ..button::Style::default()
            }
        })
        .into()
}

fn build_footer(state: &LibraryState) -> Element<'_, crate::Message> {
    let id_input = text_input("App ID (e.g. 105600)", &state.manual_app_id_input)
        .on_input(|s| crate::Message::Library(LibraryMessage::ManualAppIdChanged(s)))
        .on_submit(crate::Message::Library(
            LibraryMessage::ManualAppIdSubmitted,
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
            .on_press(crate::Message::Library(
                LibraryMessage::ManualAppIdSubmitted,
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
            .on_press(crate::Message::Library(LibraryMessage::RescanRequested))
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
