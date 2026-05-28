use iced::widget::{column, container, image as img_widget, lazy, mouse_area, row, stack, text};
use iced::{Alignment, Color, Element, Length, Padding};

use super::card_parts::{
    build_hover_overlay, build_name_row, build_tags_row, build_tier_stacked_bar,
};
use super::dims::*;
use crate::capsule_cache::CapsuleSize;
use crate::game_view::types::RarityTier;
use crate::profile_view::types::{CapsuleAsset, GameEntry, ProfileViewMessage};
use crate::ui::theme::{palette, theme_from_iced};
use crate::ui::widgets::card::card;
use crate::ui::widgets::skeleton::skeleton_box;

#[derive(Hash, PartialEq, Eq)]
struct SkeletonCardDeps {
    app_id: u32,
    card_w_bits: u32,
    capsule_w_bits: u32,
    capsule_h_bits: u32,
    total_h_bits: u32,
}

impl SkeletonCardDeps {
    fn new(app_id: u32, card_w: f32, capsule_w: f32, capsule_h: f32, total_h: f32) -> Self {
        Self {
            app_id,
            card_w_bits: card_w.to_bits(),
            capsule_w_bits: capsule_w.to_bits(),
            capsule_h_bits: capsule_h.to_bits(),
            total_h_bits: total_h.to_bits(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_card<'a>(
    entry: &'a GameEntry,
    capsule_size: CapsuleSize,
    card_w: f32,
    skeleton_phase: f32,
    tier_breakdown: &'a [(RarityTier, u32)],
    genre: Option<&'a str>,
    is_pinned: bool,
    is_hovered: bool,
    hovered_tier: Option<RarityTier>,
) -> Element<'a, ProfileViewMessage> {
    let app_id = entry.app_id;
    let (capsule_w, capsule_h) = capsule_dims(capsule_size);
    let total_h = total_card_height(capsule_h);

    if !entry.is_hydrated() {
        let skel_deps = SkeletonCardDeps::new(app_id, card_w, capsule_w, capsule_h, total_h);
        let static_layer = lazy(skel_deps, move |_| {
            build_skeleton_card_static(app_id, card_w, capsule_w, capsule_h, total_h)
        });
        let shimmer = build_skeleton_shimmer_overlay(card_w, total_h, skeleton_phase);
        let card_stack = stack![static_layer, shimmer];
        return mouse_area(card_stack)
            .on_enter(ProfileViewMessage::CardHoverEnter(app_id))
            .on_exit(ProfileViewMessage::CardHoverExit(app_id))
            .into();
    }

    let deps = HydratedCardDeps::new(
        entry,
        capsule_size,
        card_w,
        tier_breakdown,
        genre,
        is_pinned,
        is_hovered,
        hovered_tier,
    );

    let owned = HydratedCardOwned::new(
        entry,
        card_w,
        capsule_w,
        capsule_h,
        total_h,
        tier_breakdown,
        genre,
        is_pinned,
        is_hovered,
        hovered_tier,
    );

    lazy(deps, move |_| {
        let inner = render_hydrated_card(&owned);
        let area = mouse_area(inner)
            .on_enter(ProfileViewMessage::CardHoverEnter(app_id))
            .on_exit(ProfileViewMessage::CardHoverExit(app_id));
        Element::<'static, ProfileViewMessage>::from(area)
    })
    .into()
}

#[derive(Hash, PartialEq, Eq)]
struct HydratedCardDeps {
    app_id: u32,
    capsule_size: CapsuleSize,
    card_w_bits: u32,
    capsule_state: u8,
    img_w: u32,
    img_h: u32,
    is_hovered: bool,
    is_pinned: bool,
    hovered_tier: Option<RarityTier>,
    progress_earned: Option<u32>,
    progress_total: Option<u32>,
    name_len: usize,
    tier_hash: u64,
    genre_hash: u64,
}

impl HydratedCardDeps {
    #[allow(clippy::too_many_arguments)]
    fn new(
        entry: &GameEntry,
        capsule_size: CapsuleSize,
        card_w: f32,
        tier_breakdown: &[(RarityTier, u32)],
        genre: Option<&str>,
        is_pinned: bool,
        is_hovered: bool,
        hovered_tier: Option<RarityTier>,
    ) -> Self {
        let (capsule_state, img_w, img_h) = match &entry.capsule {
            CapsuleAsset::Pending => (0u8, 0u32, 0u32),
            CapsuleAsset::Loaded { width, height, .. } => (1u8, *width, *height),
            CapsuleAsset::Unavailable => (2u8, 0u32, 0u32),
        };

        let tier_hash = hash_tier_breakdown(tier_breakdown);
        let genre_hash = hash_str_opt(genre);

        Self {
            app_id: entry.app_id,
            capsule_size,
            card_w_bits: card_w.to_bits(),
            capsule_state,
            img_w,
            img_h,
            is_hovered,
            is_pinned,
            hovered_tier,
            progress_earned: entry.progress.as_ref().map(|p| p.earned),
            progress_total: entry.progress.as_ref().map(|p| p.total),
            name_len: entry.name.as_deref().map(str::len).unwrap_or(0),
            tier_hash,
            genre_hash,
        }
    }
}

fn hash_tier_breakdown(breakdown: &[(RarityTier, u32)]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for (t, count) in breakdown {
        h = h.wrapping_mul(0x100000001b3);
        h ^= (*t as u64).wrapping_mul(0x9e3779b97f4a7c15);
        h = h.wrapping_mul(0x100000001b3);
        h ^= *count as u64;
    }
    h
}

fn hash_str_opt(s: Option<&str>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    if let Some(g) = s {
        for b in g.bytes() {
            h = h.wrapping_mul(0x100000001b3);
            h ^= b as u64;
        }
    }
    h
}

struct HydratedCardOwned {
    entry: GameEntry,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
    tier_breakdown: Vec<(RarityTier, u32)>,
    genre: Option<String>,
    is_pinned: bool,
    is_hovered: bool,
    hovered_tier: Option<RarityTier>,
}

impl HydratedCardOwned {
    #[allow(clippy::too_many_arguments)]
    fn new(
        entry: &GameEntry,
        card_w: f32,
        capsule_w: f32,
        capsule_h: f32,
        total_h: f32,
        tier_breakdown: &[(RarityTier, u32)],
        genre: Option<&str>,
        is_pinned: bool,
        is_hovered: bool,
        hovered_tier: Option<RarityTier>,
    ) -> Self {
        Self {
            entry: entry.clone(),
            card_w,
            capsule_w,
            capsule_h,
            total_h,
            tier_breakdown: tier_breakdown.to_vec(),
            genre: genre.map(str::to_owned),
            is_pinned,
            is_hovered,
            hovered_tier,
        }
    }
}

fn render_hydrated_card(p: &HydratedCardOwned) -> Element<'static, ProfileViewMessage> {
    let entry = &p.entry;
    let app_id = entry.app_id;
    let card_w = p.card_w;
    let capsule_h = p.capsule_h;
    let total_h = p.total_h;
    let is_hovered = p.is_hovered;
    let is_pinned = p.is_pinned;
    let hovered_tier = p.hovered_tier;

    let capsule_area: Element<'static, ProfileViewMessage> = match &entry.capsule {
        CapsuleAsset::Loaded {
            handle,
            width,
            height,
        } => {
            let (rendered_w, rendered_h) =
                fit_contain(*width as f32, *height as f32, p.capsule_w, capsule_h);
            let handle = handle.clone();
            container(
                container(
                    img_widget(handle)
                        .width(Length::Fixed(rendered_w))
                        .height(Length::Fixed(rendered_h)),
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
            .align_y(Alignment::Center)
            .into()
        }

        CapsuleAsset::Pending => container(iced::widget::Space::new())
            .width(Length::Fixed(card_w))
            .height(Length::Fixed(capsule_h))
            .style(|t: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(
                    palette(theme_from_iced(t)).placeholder,
                )),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into(),

        CapsuleAsset::Unavailable => {
            let label = entry.name.as_deref().unwrap_or("no image").to_owned();
            container(
                container(text(label).size(12).align_x(Alignment::Center).style(
                    |t: &iced::Theme| iced::widget::text::Style {
                        color: Some(palette(theme_from_iced(t)).text_muted),
                    },
                ))
                .width(Length::Fixed(p.capsule_w))
                .height(Length::Fixed(capsule_h))
                .padding(Padding::default().left(8).right(8))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|t: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(
                        palette(theme_from_iced(t)).placeholder,
                    )),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..iced::Border::default()
                    },
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
            .into()
        }
    };

    let hover_overlay: Element<'static, ProfileViewMessage> = if is_hovered || is_pinned {
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
        app_id,
        &p.tier_breakdown,
        entry.progress.as_ref().map(|pr| pr.earned).unwrap_or(0),
        entry.progress.as_ref().map(|pr| pr.total).unwrap_or(0),
        card_w,
        hovered_tier,
    );

    let tags_row = build_tags_row(entry, card_w, p.genre.as_deref());

    let card_inner = column![
        capsule_stack,
        separator,
        name_row,
        tier_bar,
        iced::widget::Space::new().height(Length::Fill),
        tags_row,
    ]
    .spacing(0);

    let is_gold = entry
        .progress
        .as_ref()
        .is_some_and(|pr| pr.total > 0 && pr.earned >= pr.total);

    card(card_inner)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(total_h))
        .padding(Padding::default().top(8))
        .radius(6.0)
        .hovered(is_hovered)
        .gold_when(is_gold)
        .on_press(ProfileViewMessage::GameSelected(app_id))
        .into()
}

fn build_skeleton_card_static(
    app_id: u32,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
) -> Element<'static, ProfileViewMessage> {
    use crate::ui::widgets::skeleton::SKEL_DEFAULT_RADIUS;

    let title_width_ratio = match app_id % 5 {
        0 => 0.75,
        1 => 0.60,
        2 => 0.85,
        3 => 0.55,
        _ => 0.70,
    };

    let capsule_skel = container(skeleton_box(capsule_w, capsule_h, SKEL_DEFAULT_RADIUS, 0.0))
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(capsule_h))
        .align_x(Alignment::Center);

    let name_skel = skeleton_box(
        card_w * title_width_ratio,
        CARD_NAME_TEXT_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        0.0,
    );
    let counter_skel = skeleton_box(
        card_w * SKEL_COUNTER_PILL_WIDTH_RATIO,
        CARD_COUNTER_TEXT_SIZE,
        SKEL_DEFAULT_RADIUS,
        0.0,
    );
    let progress_skel = skeleton_box(
        card_w - CARD_PROGRESS_BAR_INSET,
        CARD_PROGRESS_BAR_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        0.0,
    );
    let tag_skel_genre = skeleton_box(
        card_w * SKEL_GENRE_PILL_WIDTH_RATIO,
        CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        0.0,
    );
    let tag_skel_pct = skeleton_box(
        card_w * SKEL_COUNTER_PILL_WIDTH_RATIO,
        CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        0.0,
    );

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
    .height(Length::Fixed(CARD_NAME_ROW_HEIGHT))
    .align_y(Alignment::Center)
    .padding(
        Padding::default()
            .left(CARD_H_PAD)
            .right(CARD_H_PAD)
            .top(CARD_NAME_ROW_PAD_TOP)
            .bottom(0),
    );

    let bar_container = container(progress_skel)
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(CARD_PROGRESS_BAR_HEIGHT))
        .padding(Padding::default().left(CARD_H_PAD).right(CARD_H_PAD));

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
    .height(Length::Fixed(CARD_TAGS_ROW_HEIGHT))
    .padding(
        Padding::default()
            .left(CARD_H_PAD)
            .right(CARD_H_PAD)
            .top(CARD_TAGS_ROW_PAD_TOP)
            .bottom(CARD_TAGS_ROW_PAD_BOTTOM),
    )
    .align_y(Alignment::End);

    let card_inner = column![
        capsule_skel,
        separator_space,
        name_row_skel,
        bar_container,
        iced::widget::Space::new().height(Length::Fill),
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

fn build_skeleton_shimmer_overlay(
    card_w: f32,
    card_h: f32,
    phase: f32,
) -> Element<'static, ProfileViewMessage> {
    use iced::Radians;
    use iced::gradient;

    let band_half = 0.22f32;
    let lo = (phase - band_half).max(0.0);
    let hi = (phase + band_half).min(1.0);

    let angle = Radians(std::f32::consts::FRAC_PI_2);
    let shine = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
    let clear = Color::from_rgba(1.0, 1.0, 1.0, 0.0);

    let grad = gradient::Linear::new(angle)
        .add_stop(0.0, clear)
        .add_stop(lo.max(0.001), clear)
        .add_stop(phase.clamp(0.001, 0.999), shine)
        .add_stop(hi.min(0.999), clear)
        .add_stop(1.0, clear);

    container(iced::widget::Space::new())
        .width(Length::Fixed(card_w))
        .height(Length::Fixed(card_h))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Gradient(gradient::Gradient::Linear(grad))),
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
