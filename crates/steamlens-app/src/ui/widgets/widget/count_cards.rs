use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::game_view::types::RarityTier;
use crate::ui::theme::{palette, theme_from_iced};

use super::format::{format_remaining, format_thousands};
use super::rarity_visuals::{rarity_color, rarity_label, short_rarity_label_str};
use super::summary::WidgetSummary;

const RARITY_CARD_MAX_WIDTH: f32 = 124.0;
const RARITY_CARD_GAP: f32 = 16.0;
const RARITY_CARDS_MAX_WIDTH: f32 = RARITY_CARD_MAX_WIDTH * 5.0 + RARITY_CARD_GAP * 4.0;
const RARITY_CARD_HEIGHT: f32 = 75.0;
const RARITY_CARD_SHORT_THRESHOLD: f32 = 95.0;

pub fn count_card<'a, M: 'a + Clone>(
    accent: Color,
    label: &'static str,
    count: u32,
    pct: f64,
) -> Element<'a, M> {
    let body = iced::widget::responsive(move |size| {
        let display_label: &'static str = if size.width < RARITY_CARD_SHORT_THRESHOLD {
            short_rarity_label_str(label)
        } else {
            label
        };

        let stripe = container(iced::widget::Space::new())
            .width(Length::Fixed(3.0))
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(accent)),
                ..container::Style::default()
            });

        let number = text(format_thousands(count)).size(18).color(accent);
        let pct_text = text(format!("{pct:.1}%"))
            .size(10)
            .color(Color { a: 0.75, ..accent });
        let label_text =
            text(display_label)
                .size(11)
                .style(|t: &iced::Theme| iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).text_muted),
                });

        let info_col = column![number, label_text, pct_text]
            .spacing(2)
            .padding(Padding::default().left(8).right(6).top(6).bottom(6));

        row![stripe, info_col]
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into()
    });

    container(body)
        .width(Length::Fill)
        .height(Length::Fixed(RARITY_CARD_HEIGHT))
        .clip(true)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.08, ..accent })),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

pub fn rarity_cards<'a, M: 'a + Clone>(summary: &WidgetSummary) -> Element<'a, M> {
    let tiers: [(RarityTier, u32); 5] = [
        (RarityTier::Common, summary.common_count),
        (RarityTier::Uncommon, summary.uncommon_count),
        (RarityTier::Rare, summary.rare_count),
        (RarityTier::Mythical, summary.mythical_count),
        (RarityTier::Legendary, summary.legendary_count),
    ];
    let total = summary.achievement_total;

    let mut cards = row![].spacing(RARITY_CARD_GAP);
    for (tier, count) in tiers {
        let pct = if total > 0 {
            count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        cards = cards.push(count_card::<M>(
            rarity_color(tier),
            rarity_label(tier),
            count,
            pct,
        ));
    }

    container(cards.width(Length::Fill))
        .width(Length::Fill)
        .max_width(RARITY_CARDS_MAX_WIDTH)
        .into()
}

pub fn cards_separator<'a, M: 'a + Clone>(summary: &WidgetSummary) -> Element<'a, M> {
    let remaining = summary.locked();
    let pct_to_go = summary.pct_to_go();

    let remaining_label =
        text(format_remaining(remaining as u64))
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            });
    let pct_label = text(format!("{pct_to_go:.1}% to go"))
        .size(11)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_dim),
        });

    let label_row = row![
        remaining_label,
        iced::widget::Space::new().width(Length::Fill),
        pct_label,
    ]
    .align_y(Alignment::Center);

    column![iced::widget::rule::horizontal(1), label_row]
        .spacing(6)
        .into()
}
