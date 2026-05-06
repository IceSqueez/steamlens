use std::collections::HashMap;

use iced::widget::Id as WidgetId;
use iced::widget::{
    button, column, container, image as img_widget, mouse_area, responsive, row, scrollable, stack,
    text, text_input, tooltip,
};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::cache::GameCacheEntry;
use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::skeleton::skeleton_box;
use crate::theme::{
    C_ACCENT, C_ACCENT_DARK, C_APP, C_BORDER, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED,
    C_TEXT_PRIMARY,
};

use super::ProfileViewState;
use super::profile::{
    C_RARITY_COMMON, C_RARITY_LEGENDARY, C_RARITY_MYTHICAL, C_RARITY_RARE, C_RARITY_UNCOMMON,
};
use super::profile::{
    ProfileWidgetParams, compute_profile_summary, profile_widget, top5_closest_to_complete,
};
use super::types::{CapsuleAsset, GameEntry, LibrarySort, ProfileViewMessage, ProfileViewPhase};

const CARD_GAP: f32 = 12.0;
const MIN_GAP: f32 = 12.0;

fn compute_grid(viewport: f32, card_w: f32, min_gap: f32) -> (usize, f32) {
    let cols_max = ((viewport + min_gap) / (card_w + min_gap)).floor().max(1.0) as usize;

    let mut cols = cols_max;
    loop {
        let total_card_width = cols as f32 * card_w;
        let remainder = (viewport - total_card_width).max(0.0);
        let gap = remainder / (cols as f32 + 1.0);
        if gap >= min_gap || cols == 1 {
            let clamped_gap = gap.max(0.0);
            return (cols, clamped_gap);
        }
        cols -= 1;
    }
}

const C_PLACEHOLDER: Color = Color::from_rgb(0.188, 0.192, 0.247);
const C_MUTED: Color = Color::from_rgb(0.384, 0.447, 0.643);
const C_TEXT: Color = Color::from_rgb(0.973, 0.973, 0.949);

const C_GOLD: Color = Color::from_rgb(1.0, 0.85, 0.4);

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
    capsule_h + 8.0 + 9.0 + 24.0 + 8.0 + 8.0 + 24.0 + 8.0
}

pub fn render_with_cache_actions<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
    pinned: &'a [u32],
) -> Element<'a, crate::Message> {
    render_inner(
        state,
        user_profile,
        avatar_handle,
        cached_entries,
        skeleton_phase,
        pinned,
    )
}

fn render_inner<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
    pinned: &'a [u32],
) -> Element<'a, crate::Message> {
    let header = build_header(state);

    let profile_section = build_profile_section(
        state,
        user_profile,
        avatar_handle,
        cached_entries,
        skeleton_phase,
    );

    let body: Element<'_, crate::Message> = match &state.phase {
        ProfileViewPhase::Scanning => center_text("Scanning library\u{2026}"),
        ProfileViewPhase::Error(e) => error_view(e),
        ProfileViewPhase::Loaded => {
            let visible = state.visible_games(pinned);

            if visible.is_empty() {
                center_text("No games found.")
            } else {
                build_grid(state, visible, cached_entries, skeleton_phase, pinned)
            }
        }
    };

    let mut col = column![header];
    col = col.push(profile_section);
    col = col.push(body);

    col.spacing(0).into()
}

fn build_profile_section<'a>(
    state: &'a ProfileViewState,
    user_profile: Option<&'a steamlens_core::UserProfile>,
    avatar_handle: Option<&'a iced::widget::image::Handle>,
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
) -> Element<'a, crate::Message> {
    let summary = compute_profile_summary(cached_entries);
    let top5 = top5_closest_to_complete(&state.games, cached_entries);
    profile_widget(ProfileWidgetParams {
        user_profile,
        avatar_handle,
        summary,
        top5,
        games_count: state.games.len(),
        skeleton_phase,
        hovered_bar_slice: state.hovered_bar_slice,
        capsule_handles: &state.capsule_handles,
        capsule_size: state.capsule_size,
    })
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
    let input = text_input("Search games\u{2026}", &state.search)
        .id(library_search_id())
        .on_input(|s| crate::Message::ProfileView(ProfileViewMessage::SearchChanged(s)))
        .padding(Padding::default().left(10).right(10).top(6).bottom(6))
        .size(13)
        .style(
            |_theme: &iced::Theme, _status| iced::widget::text_input::Style {
                background: iced::Background::Color(Color::TRANSPARENT),
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

    let inner_row = row![input, kbd_badge]
        .spacing(6)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(0).right(8));

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
    let order = [
        LibrarySort::NameAsc,
        LibrarySort::LastPlayed,
        LibrarySort::Completion,
    ];
    let items: Vec<(&'static str, Option<&'static str>, bool, crate::Message)> = order
        .iter()
        .map(|&s| {
            (
                s.short_label(),
                Some(s.tooltip()),
                current == s,
                crate::Message::ProfileView(ProfileViewMessage::SortChanged(s)),
            )
        })
        .collect();
    let segment = segment_row(&items);
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
            None,
            current == CapsuleSize::Small,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(CapsuleSize::Small)),
        ),
        (
            "M",
            None,
            current == CapsuleSize::Medium,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(
                CapsuleSize::Medium,
            )),
        ),
        (
            "L",
            None,
            current == CapsuleSize::Large,
            crate::Message::ProfileView(ProfileViewMessage::CapsuleSizeChanged(CapsuleSize::Large)),
        ),
    ]);
    row![label, segment]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn segment_row(
    items: &[(&'static str, Option<&'static str>, bool, crate::Message)],
) -> Element<'static, crate::Message> {
    let mut r = row![].spacing(0).align_y(Alignment::Center);
    let last_idx = items.len().saturating_sub(1);

    for (idx, (label, hint, active, msg)) in items.iter().enumerate() {
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

        let item_el: Element<'static, crate::Message> = match hint {
            Some(text_str) => tooltip(
                btn,
                container(text(*text_str).size(11).color(C_TEXT_PRIMARY))
                    .padding(Padding::default().left(8).right(8).top(4).bottom(4))
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(Color::from_rgba(
                            0.10, 0.09, 0.14, 0.95,
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
            .into(),
            None => btn.into(),
        };
        r = r.push(item_el);

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
    cached_entries: &'a HashMap<u32, GameCacheEntry>,
    skeleton_phase: f32,
    pinned: &'a [u32],
) -> Element<'a, crate::Message> {
    let capsule_size = state.capsule_size;
    let card_w = card_width(capsule_size);
    let hovered_card = state.hovered_card;

    let entries: Vec<&'a GameEntry> = visible;

    let grid = responsive(move |size| {
        let (cols, gap) = compute_grid(size.width, card_w, MIN_GAP);

        let mut rows_col: iced::widget::Column<'_, crate::Message> = column![]
            .spacing(CARD_GAP as u32)
            .padding(Padding::default().top(8).bottom(8));

        for chunk in entries.chunks(cols) {
            let mut r: iced::widget::Row<'_, crate::Message> =
                row![iced::widget::Space::new().width(Length::Fixed(gap))];
            for entry in chunk {
                let app_id = entry.app_id;
                let cached = cached_entries.get(&app_id);
                let tier_breakdown = cached.map(|e| e.tier_breakdown.as_slice()).unwrap_or(&[]);
                let genre = cached.and_then(|e| e.genre.as_deref());
                let is_pinned = pinned.contains(&app_id);
                let is_hovered = hovered_card == Some(app_id);
                r = r.push(build_card(
                    entry,
                    capsule_size,
                    card_w,
                    skeleton_phase,
                    tier_breakdown,
                    genre,
                    is_pinned,
                    is_hovered,
                ));
                r = r.push(iced::widget::Space::new().width(Length::Fixed(gap)));
            }
            let needed = cols - chunk.len();
            for _ in 0..needed {
                r = r.push(iced::widget::Space::new().width(Length::Fixed(card_w)));
                r = r.push(iced::widget::Space::new().width(Length::Fixed(gap)));
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

fn completion_tier_color(pct: f32) -> Color {
    if pct >= 100.0 {
        C_RARITY_LEGENDARY
    } else if pct >= 90.0 {
        C_RARITY_MYTHICAL
    } else if pct >= 75.0 {
        C_RARITY_RARE
    } else if pct >= 50.0 {
        C_RARITY_UNCOMMON
    } else if pct >= 25.0 {
        C_RARITY_COMMON
    } else {
        C_TEXT_MUTED
    }
}

fn rarity_color_for_tier(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_RARITY_COMMON,
        RarityTier::Uncommon => C_RARITY_UNCOMMON,
        RarityTier::Rare => C_RARITY_RARE,
        RarityTier::Mythical => C_RARITY_MYTHICAL,
        RarityTier::Legendary => C_RARITY_LEGENDARY,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_card<'a>(
    entry: &'a GameEntry,
    capsule_size: CapsuleSize,
    card_w: f32,
    skeleton_phase: f32,
    tier_breakdown: &'a [(RarityTier, u32)],
    genre: Option<&'a str>,
    is_pinned: bool,
    is_hovered: bool,
) -> Element<'a, crate::Message> {
    let app_id = entry.app_id;
    let (capsule_w, capsule_h) = capsule_dims(capsule_size);
    let total_h = total_card_height(capsule_h);

    if !entry.is_hydrated() {
        let inner =
            build_skeleton_card(entry, card_w, capsule_w, capsule_h, total_h, skeleton_phase);
        return mouse_area(inner)
            .on_enter(crate::Message::ProfileView(
                ProfileViewMessage::CardHoverEnter(app_id),
            ))
            .on_exit(crate::Message::ProfileView(
                ProfileViewMessage::CardHoverExit(app_id),
            ))
            .into();
    }

    let inner = build_hydrated_card(HydratedCardParams {
        entry,
        app_id,
        card_w,
        capsule_w,
        capsule_h,
        total_h,
        tier_breakdown,
        genre,
        is_pinned,
        is_hovered,
    });

    mouse_area(inner)
        .on_enter(crate::Message::ProfileView(
            ProfileViewMessage::CardHoverEnter(app_id),
        ))
        .on_exit(crate::Message::ProfileView(
            ProfileViewMessage::CardHoverExit(app_id),
        ))
        .into()
}

fn build_skeleton_card<'a>(
    entry: &'a GameEntry,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
    phase: f32,
) -> Element<'a, crate::Message> {
    let title_width_ratio = match entry.app_id % 5 {
        0 => 0.75,
        1 => 0.60,
        2 => 0.85,
        3 => 0.55,
        _ => 0.70,
    };

    let capsule_skel = skeleton_box(capsule_w, capsule_h, phase);
    let name_skel = skeleton_box(card_w * title_width_ratio, 12.0, phase);
    let counter_skel = skeleton_box(card_w * 0.18, 12.0, phase);
    let progress_skel = skeleton_box(card_w - 16.0, 8.0, phase);
    let tag_skel_genre = skeleton_box(card_w * 0.28, 18.0, phase);
    let tag_skel_pct = skeleton_box(card_w * 0.18, 18.0, phase);

    let separator_space = iced::widget::Space::new()
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(8.0));

    let name_row_skel = container(
        row![
            name_skel,
            iced::widget::Space::new().width(Length::Fill),
            counter_skel
        ]
        .align_y(Alignment::Center)
        .width(Length::Fixed(card_w - 16.0)),
    )
    .width(Length::Fixed(card_w))
    .height(Length::Fixed(24.0))
    .align_y(Alignment::Center)
    .padding(Padding::default().left(8).right(8));

    let bar_container = container(progress_skel)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(8.0))
        .padding(Padding::default().left(8).right(8));

    let bar_gap = iced::widget::Space::new()
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(8.0));

    let tags_row = container(
        row![
            tag_skel_genre,
            iced::widget::Space::new().width(Length::Fill),
            tag_skel_pct
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(card_w))
    .height(Length::Fixed(24.0))
    .padding(Padding::default().left(4).right(4).top(3).bottom(3));

    let card_inner = column![
        capsule_skel,
        separator_space,
        name_row_skel,
        bar_gap,
        bar_container,
        tags_row,
    ]
    .spacing(0);

    container(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_h))
        .padding(Padding::default().top(8))
        .style(|_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(C_SKELETON_BG)),
            border: iced::Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

const C_SKELETON_BG: Color = Color::from_rgb(0.267, 0.278, 0.353);

struct HydratedCardParams<'a> {
    entry: &'a GameEntry,
    app_id: u32,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
    tier_breakdown: &'a [(RarityTier, u32)],
    genre: Option<&'a str>,
    is_pinned: bool,
    is_hovered: bool,
}

fn build_hydrated_card<'a>(p: HydratedCardParams<'a>) -> Element<'a, crate::Message> {
    let HydratedCardParams {
        entry,
        app_id,
        card_w,
        capsule_w,
        capsule_h,
        total_h,
        tier_breakdown,
        genre,
        is_pinned,
        is_hovered,
    } = p;
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

    let hover_overlay: Element<'_, crate::Message> = if is_hovered {
        build_hover_overlay(app_id, is_pinned, card_w, capsule_h)
    } else {
        iced::widget::Space::new()
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(capsule_h))
            .into()
    };

    let capsule_stack = stack![capsule_area, hover_overlay];

    let separator = container(iced::widget::rule::horizontal(1))
        .padding(Padding::default().left(8).right(8).top(8).bottom(0))
        .width(Length::Fixed(card_w));

    let name_row = build_name_row(entry, card_w);

    let tier_bar = build_tier_stacked_bar(
        tier_breakdown,
        entry.progress.as_ref().map(|p| p.earned).unwrap_or(0),
        entry.progress.as_ref().map(|p| p.total).unwrap_or(0),
        card_w,
    );

    let tags_row = build_tags_row(entry, card_w, genre);

    let card_inner = column![capsule_stack, separator, name_row, tier_bar, tags_row,].spacing(0);

    let card = container(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_h))
        .padding(Padding::default().top(8));

    let is_gold = entry
        .progress
        .as_ref()
        .is_some_and(|p| p.total > 0 && p.earned >= p.total);
    let card_btn = button(card)
        .padding(0)
        .on_press(crate::Message::ProfileView(
            ProfileViewMessage::GameSelected(app_id),
        ))
        .style(move |_: &iced::Theme, status| {
            let btn_hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let effectively_hovered = is_hovered || btn_hovered;

            let bg = if effectively_hovered {
                Color {
                    r: (C_SURFACE.r * 1.18).min(1.0),
                    g: (C_SURFACE.g * 1.18).min(1.0),
                    b: (C_SURFACE.b * 1.18).min(1.0),
                    a: 1.0,
                }
            } else {
                C_SURFACE
            };

            let border_color = if is_gold && effectively_hovered {
                C_GOLD
            } else if effectively_hovered {
                C_ACCENT
            } else if is_gold {
                Color { a: 0.5, ..C_GOLD }
            } else {
                Color::TRANSPARENT
            };

            let border_width = if effectively_hovered || is_gold {
                2.0
            } else {
                0.0
            };

            let shadow = if is_gold {
                iced::Shadow {
                    color: Color::from_rgba(
                        C_GOLD.r,
                        C_GOLD.g,
                        C_GOLD.b,
                        if effectively_hovered { 0.5 } else { 0.25 },
                    ),
                    offset: iced::Vector::new(0.0, 0.0),
                    blur_radius: if effectively_hovered { 14.0 } else { 6.0 },
                }
            } else if effectively_hovered {
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

    card_btn.into()
}

fn build_tier_stacked_bar<'a>(
    tier_breakdown: &'a [(RarityTier, u32)],
    total_earned: u32,
    total_achievements: u32,
    card_w: f32,
) -> Element<'a, crate::Message> {
    const BAR_H: f32 = 8.0;
    const BAR_RADIUS: f32 = 4.0;
    const TIER_ORDER: [RarityTier; 5] = [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Mythical,
        RarityTier::Legendary,
    ];
    const C_TRACK: Color = Color::from_rgba(0.4, 0.45, 0.65, 0.25);

    let inner_w = card_w - 16.0;
    let locked_count = total_achievements.saturating_sub(total_earned);
    let has_any = total_achievements > 0;

    let bar: Element<'_, crate::Message> = if !has_any {
        container(iced::widget::Space::new())
            .width(Length::Fixed(inner_w))
            .height(Length::Fixed(BAR_H))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_TRACK)),
                border: iced::Border {
                    radius: BAR_RADIUS.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else {
        let total_f = total_achievements as f32;
        let mut segments: iced::widget::Row<'_, crate::Message> = row![].spacing(0);

        let active_tiers: Vec<(RarityTier, u32)> = TIER_ORDER
            .iter()
            .filter_map(|t| {
                let c = tier_breakdown
                    .iter()
                    .find(|(tt, _)| tt == t)
                    .map(|(_, c)| *c)
                    .unwrap_or(0);
                if c > 0 { Some((*t, c)) } else { None }
            })
            .collect();
        let has_locked = locked_count > 0;
        let earned_segs = active_tiers.len();

        for (i, (tier, count)) in active_tiers.iter().enumerate() {
            let seg_w = (*count as f32 / total_f * inner_w).max(1.0);
            let color = rarity_color_for_tier(*tier);
            let is_first = i == 0;
            let is_last_earned = i + 1 == earned_segs && !has_locked;
            let radius = iced::border::Radius {
                top_left: if is_first { BAR_RADIUS } else { 0.0 },
                bottom_left: if is_first { BAR_RADIUS } else { 0.0 },
                top_right: if is_last_earned { BAR_RADIUS } else { 0.0 },
                bottom_right: if is_last_earned { BAR_RADIUS } else { 0.0 },
            };

            if i > 0 {
                let gap = container(iced::widget::Space::new())
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(BAR_H))
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(C_TRACK)),
                        ..container::Style::default()
                    });
                segments = segments.push(gap);
            }

            let seg = container(iced::widget::Space::new())
                .width(Length::Fixed(seg_w))
                .height(Length::Fixed(BAR_H))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: iced::Border {
                        radius,
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                });
            segments = segments.push(seg);
        }

        if has_locked {
            if earned_segs > 0 {
                let gap = container(iced::widget::Space::new())
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(BAR_H))
                    .style(|_: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(C_TRACK)),
                        ..container::Style::default()
                    });
                segments = segments.push(gap);
            }
            let locked_w = (locked_count as f32 / total_f * inner_w).max(1.0);
            let radius = iced::border::Radius {
                top_left: if earned_segs == 0 { BAR_RADIUS } else { 0.0 },
                bottom_left: if earned_segs == 0 { BAR_RADIUS } else { 0.0 },
                top_right: BAR_RADIUS,
                bottom_right: BAR_RADIUS,
            };
            let locked_seg = container(iced::widget::Space::new())
                .width(Length::Fixed(locked_w))
                .height(Length::Fixed(BAR_H))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(C_TRACK)),
                    border: iced::Border {
                        radius,
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                });
            segments = segments.push(locked_seg);
        }

        container(segments)
            .width(Length::Fixed(inner_w))
            .height(Length::Fixed(BAR_H))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(C_TRACK)),
                border: iced::Border {
                    radius: BAR_RADIUS.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    };

    container(bar)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(BAR_H))
        .padding(Padding::default().left(8).right(8))
        .into()
}

fn build_hover_overlay<'a>(
    app_id: u32,
    is_pinned: bool,
    card_w: f32,
    capsule_h: f32,
) -> Element<'a, crate::Message> {
    let pin_label = if is_pinned {
        "\u{2299} Unpin"
    } else {
        "\u{2299} Pin"
    };
    let pin_btn =
        button(
            text(pin_label)
                .size(11)
                .color(if is_pinned { C_ACCENT } else { C_TEXT_PRIMARY }),
        )
        .on_press(crate::Message::ToggleGamePin(app_id))
        .padding(Padding::default().left(10).right(10).top(4).bottom(4))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(iced::Background::Color(if hovered {
                    Color { a: 0.90, ..C_HOVER }
                } else {
                    Color {
                        a: 0.75,
                        ..C_SURFACE
                    }
                })),
                border: iced::Border {
                    color: if is_pinned {
                        Color { a: 0.6, ..C_ACCENT }
                    } else {
                        C_BORDER
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                text_color: if is_pinned { C_ACCENT } else { C_TEXT_PRIMARY },
                ..button::Style::default()
            }
        });

    container(pin_btn)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h))
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .padding(Padding::default().top(8).right(8))
        .into()
}

fn build_name_row<'a>(entry: &'a GameEntry, card_w: f32) -> Element<'a, crate::Message> {
    let name_text = text(entry.name.as_deref().unwrap_or(""))
        .size(12)
        .color(C_TEXT)
        .wrapping(text::Wrapping::None);

    let counter: Element<'_, crate::Message> = match entry.progress.as_ref() {
        Some(p) if p.total > 0 => text(format!("{} / {}", p.earned, p.total))
            .size(11)
            .color(C_MUTED)
            .into(),
        _ => iced::widget::Space::new().width(Length::Shrink).into(),
    };

    let inner = row![
        name_text,
        iced::widget::Space::new().width(Length::Fill),
        counter
    ]
    .align_y(Alignment::Center)
    .spacing(4);

    container(inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(24.0))
        .align_y(Alignment::Center)
        .padding(Padding::default().left(8).right(8).top(4).bottom(0))
        .into()
}

fn build_tags_row<'a>(
    entry: &'a GameEntry,
    card_w: f32,
    genre: Option<&'a str>,
) -> Element<'a, crate::Message> {
    let progress = entry.progress.as_ref();

    let completion_tag: Option<Element<'_, crate::Message>> = progress.and_then(|p| {
        if p.total == 0 {
            return None;
        }
        let pct = p.earned as f32 / p.total as f32 * 100.0;
        let tier_color = completion_tier_color(pct);
        let is_legendary = pct >= 100.0;

        let dot = container(iced::widget::Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(tier_color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let pct_text = text(format!("{pct:.0}%")).size(11).color(tier_color);

        let pill_inner = row![dot, pct_text].spacing(5).align_y(Alignment::Center);

        let shadow = if is_legendary {
            iced::Shadow {
                color: Color {
                    a: 0.5,
                    ..C_RARITY_LEGENDARY
                },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 10.0,
            }
        } else {
            iced::Shadow::default()
        };

        let pill = container(pill_inner)
            .padding(Padding::default().left(10).right(10).top(3).bottom(3))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color {
                    a: 0.15,
                    ..tier_color
                })),
                border: iced::Border {
                    color: Color {
                        a: 0.4,
                        ..tier_color
                    },
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow,
                ..container::Style::default()
            });

        Some(pill.into())
    });

    let genre_tag: Option<Element<'_, crate::Message>> = genre.map(|g| {
        let label = text(g).size(10).color(C_TEXT_MUTED);
        container(label)
            .padding(Padding::default().left(8).right(8).top(2).bottom(2))
            .style(|_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(Color {
                    a: 0.10,
                    ..C_TEXT_MUTED
                })),
                border: iced::Border {
                    color: Color {
                        a: 0.20,
                        ..C_TEXT_MUTED
                    },
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    });

    let mut left_tags: iced::widget::Row<'_, crate::Message> =
        row![].spacing(6).align_y(Alignment::Center);

    if let Some(gtag) = genre_tag {
        left_tags = left_tags.push(gtag);
    }

    let mut tags = row![left_tags, iced::widget::Space::new().width(Length::Fill)]
        .spacing(0)
        .align_y(Alignment::Center);

    if let Some(ctag) = completion_tag {
        tags = tags.push(ctag);
    }

    container(tags)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(24.0))
        .padding(Padding::default().left(4).right(4).top(3).bottom(3))
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

#[cfg(test)]
mod grid_tests {
    use super::compute_grid;

    #[test]
    fn fixed_card_width_with_uniform_gaps() {
        let (cols, gap) = compute_grid(1000.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 40.0).abs() < 0.01, "expected gap=40, got {gap}");
    }

    #[test]
    fn min_gap_floor_kicks_in() {
        let (cols, gap) = compute_grid(1010.0, 200.0, 12.0);
        assert_eq!(cols, 4);
        assert!((gap - 42.0).abs() < 0.01, "expected gap=42, got {gap}");
    }

    #[test]
    fn single_column_below_card_width() {
        let (cols, gap) = compute_grid(150.0, 200.0, 12.0);
        assert_eq!(cols, 1);
        assert_eq!(gap, 0.0);
    }

    #[test]
    fn exact_fit_no_remainder_falls_back_to_fewer_cols() {
        let (cols, gap) = compute_grid(1000.0, 250.0, 12.0);
        assert_eq!(cols, 3);
        let expected_gap = (1000.0 - 3.0 * 250.0) / 4.0;
        assert!(
            (gap - expected_gap).abs() < 0.01,
            "expected gap={expected_gap}, got {gap}"
        );
    }

    #[test]
    fn single_column_gap_is_centered() {
        let (cols, gap) = compute_grid(300.0, 200.0, 12.0);
        assert_eq!(cols, 1);
        let expected_gap = (300.0 - 200.0) / 2.0;
        assert!(
            (gap - expected_gap).abs() < 0.01,
            "expected gap={expected_gap}, got {gap}"
        );
    }

    #[test]
    fn gap_never_negative() {
        let (_cols, gap) = compute_grid(50.0, 200.0, 12.0);
        assert!(gap >= 0.0);
    }
}
