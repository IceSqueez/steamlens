use iced::widget::{column, container, image as img_widget, mouse_area, row, stack, text};
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
        let inner =
            build_skeleton_card(entry, card_w, capsule_w, capsule_h, total_h, skeleton_phase);
        return mouse_area(inner)
            .on_enter(ProfileViewMessage::CardHoverEnter(app_id))
            .on_exit(ProfileViewMessage::CardHoverExit(app_id))
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
        hovered_tier,
    });

    mouse_area(inner)
        .on_enter(ProfileViewMessage::CardHoverEnter(app_id))
        .on_exit(ProfileViewMessage::CardHoverExit(app_id))
        .into()
}

pub(super) fn build_skeleton_card<'a>(
    entry: &'a GameEntry,
    card_w: f32,
    capsule_w: f32,
    capsule_h: f32,
    total_h: f32,
    phase: f32,
) -> Element<'a, ProfileViewMessage> {
    use crate::ui::widgets::skeleton::SKEL_DEFAULT_RADIUS;

    let title_width_ratio = match entry.app_id % 5 {
        0 => 0.75,
        1 => 0.60,
        2 => 0.85,
        3 => 0.55,
        _ => 0.70,
    };

    let capsule_skel = container(skeleton_box(
        capsule_w,
        capsule_h,
        SKEL_DEFAULT_RADIUS,
        phase,
    ))
    .width(Length::Fixed(card_w))
    .height(Length::Fixed(capsule_h))
    .align_x(Alignment::Center);

    let name_skel = skeleton_box(
        card_w * title_width_ratio,
        CARD_NAME_TEXT_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
    );
    let counter_skel = skeleton_box(
        card_w * SKEL_COUNTER_PILL_WIDTH_RATIO,
        CARD_COUNTER_TEXT_SIZE,
        SKEL_DEFAULT_RADIUS,
        phase,
    );
    let progress_skel = skeleton_box(
        card_w - CARD_PROGRESS_BAR_INSET,
        CARD_PROGRESS_BAR_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
    );
    let tag_skel_genre = skeleton_box(
        card_w * SKEL_GENRE_PILL_WIDTH_RATIO,
        CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
    );
    let tag_skel_pct = skeleton_box(
        card_w * SKEL_COUNTER_PILL_WIDTH_RATIO,
        CARD_PILL_HEIGHT,
        SKEL_DEFAULT_RADIUS,
        phase,
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
const C_SKELETON_BG: Color = Color::from_rgb(0.267, 0.278, 0.353);

pub(super) struct HydratedCardParams<'a> {
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
    hovered_tier: Option<RarityTier>,
}

pub(super) fn build_hydrated_card<'a>(
    p: HydratedCardParams<'a>,
) -> Element<'a, ProfileViewMessage> {
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
        hovered_tier,
    } = p;
    let capsule_area: Element<'_, ProfileViewMessage> = match &entry.capsule {
        CapsuleAsset::Loaded {
            handle,
            width,
            height,
        } => {
            let (rendered_w, rendered_h) =
                fit_contain(*width as f32, *height as f32, capsule_w, capsule_h);
            container(
                container(
                    img_widget(handle.clone())
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
                .width(Length::Fixed(capsule_w))
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

    let hover_overlay: Element<'_, ProfileViewMessage> = if is_hovered || is_pinned {
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
        tier_breakdown,
        entry.progress.as_ref().map(|p| p.earned).unwrap_or(0),
        entry.progress.as_ref().map(|p| p.total).unwrap_or(0),
        card_w,
        hovered_tier,
    );

    let tags_row = build_tags_row(entry, card_w, genre);

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
        .is_some_and(|p| p.total > 0 && p.earned >= p.total);

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
