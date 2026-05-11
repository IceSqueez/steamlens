//! Shared chrome for two-column summary panels (profile + game widgets).
//!
//! Each consumer constructs the per-view bits (header image+labels, top5 rows),
//! everything else — colors, tier helpers, count cards, rarity bar with ticks,
//! the 5fr/2fr surrounding frame, and a generic "closest-to-..." right-column
//! list — lives here.

use std::borrow::Cow;
use std::marker::PhantomData;

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::game_view::types::RarityTier;
use crate::theme::{C_ACCENT, C_HOVER, C_SURFACE, C_TEXT_DIM, C_TEXT_MUTED, C_TEXT_PRIMARY};
use crate::ui::theme::{AppTheme, palette};
use crate::ui::widgets::bar::{BarSegment, segmented_bar};

pub const C_RARITY_COMMON: Color = Color::from_rgb(0.314, 0.980, 0.482);
pub const C_RARITY_UNCOMMON: Color = Color::from_rgb(0.545, 0.914, 0.992);
pub const C_RARITY_RARE: Color = Color::from_rgb(0.741, 0.576, 0.976);
pub const C_RARITY_MYTHICAL: Color = Color::from_rgb(1.0, 0.4, 0.85);
pub const C_RARITY_LEGENDARY: Color = Color::from_rgb(1.0, 0.85, 0.4);

const RARITY_CARD_MAX_WIDTH: f32 = 124.0;
const RARITY_CARD_GAP: f32 = 16.0;
const RARITY_CARDS_MAX_WIDTH: f32 = RARITY_CARD_MAX_WIDTH * 5.0 + RARITY_CARD_GAP * 4.0;
const RARITY_CARD_HEIGHT: f32 = 64.0;
const RARITY_CARD_SHORT_THRESHOLD: f32 = 95.0;
const BAR_HEIGHT: f32 = 16.0;
const BAR_RADIUS: f32 = 6.0;
const WIDGET_ROW_HEIGHT: f32 = 325.0;

fn short_rarity_label_str(label: &str) -> &'static str {
    match label {
        "COMMON" => "COM",
        "UNCOMMON" => "UNC",
        "RARE" => "RARE",
        "MYTHICAL" => "MYTH",
        "LEGENDARY" => "LEG",
        _ => "",
    }
}

pub fn rarity_color(tier: RarityTier) -> Color {
    match tier {
        RarityTier::Common => C_RARITY_COMMON,
        RarityTier::Uncommon => C_RARITY_UNCOMMON,
        RarityTier::Rare => C_RARITY_RARE,
        RarityTier::Mythical => C_RARITY_MYTHICAL,
        RarityTier::Legendary => C_RARITY_LEGENDARY,
    }
}

pub fn rarity_label(tier: RarityTier) -> &'static str {
    match tier {
        RarityTier::Common => "COMMON",
        RarityTier::Uncommon => "UNCOMMON",
        RarityTier::Rare => "RARE",
        RarityTier::Mythical => "MYTHICAL",
        RarityTier::Legendary => "LEGENDARY",
    }
}

pub fn overall_tier_color(unlocked_pct: f32) -> Color {
    if unlocked_pct >= 100.0 {
        C_RARITY_LEGENDARY
    } else if unlocked_pct >= 75.0 {
        C_RARITY_MYTHICAL
    } else if unlocked_pct >= 50.0 {
        C_RARITY_RARE
    } else if unlocked_pct >= 25.0 {
        C_RARITY_UNCOMMON
    } else if unlocked_pct > 0.0 {
        C_RARITY_COMMON
    } else {
        C_TEXT_MUTED
    }
}

pub fn tick_lit_at(unlocked_pct: f32, threshold: u8) -> bool {
    unlocked_pct > 0.0 && unlocked_pct >= threshold as f32
}

pub fn format_thousands(n: u32) -> String {
    format_thousands_u64(n as u64)
}

pub fn format_thousands_u64(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

pub fn format_remaining(remaining: u64) -> String {
    format!("{} achievements remaining", format_thousands_u64(remaining))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WidgetSummary {
    pub earned_total: u32,
    pub achievement_total: u32,
    pub legendary_count: u32,
    pub mythical_count: u32,
    pub rare_count: u32,
    pub uncommon_count: u32,
    pub common_count: u32,
}

impl WidgetSummary {
    pub fn rated_unlocked(&self) -> u32 {
        self.legendary_count
            + self.mythical_count
            + self.rare_count
            + self.uncommon_count
            + self.common_count
    }

    pub fn unrated_unlocked(&self) -> u32 {
        self.earned_total.saturating_sub(self.rated_unlocked())
    }

    pub fn locked(&self) -> u32 {
        self.achievement_total.saturating_sub(self.earned_total)
    }

    pub fn unlocked_pct(&self) -> f32 {
        if self.achievement_total > 0 {
            self.earned_total as f32 / self.achievement_total as f32 * 100.0
        } else {
            0.0
        }
    }

    pub fn pct_to_go(&self) -> f64 {
        if self.achievement_total > 0 {
            self.locked() as f64 / self.achievement_total as f64 * 100.0
        } else {
            0.0
        }
    }
}

pub fn breakdown_label<'a, M: 'a>() -> Element<'a, M> {
    text("ACHIEVEMENTS BREAKDOWN")
        .size(10)
        .color(C_TEXT_MUTED)
        .into()
}

pub fn earnings_row<'a, M: 'a>(summary: &WidgetSummary) -> Element<'a, M> {
    let earned = summary.earned_total;
    let total = summary.achievement_total;
    let pct = if total > 0 {
        earned as f64 / total as f64 * 100.0
    } else {
        0.0
    };

    let earned_text = text(format_thousands(earned))
        .size(20)
        .color(C_TEXT_PRIMARY);
    let total_text = text(format!("/ {}", format_thousands(total)))
        .size(20)
        .color(C_TEXT_DIM);
    let pct_text = text(format!("{pct:.1}% unlocked")).size(12).color(C_ACCENT);

    let counter_row = row![earned_text, total_text]
        .spacing(6)
        .align_y(Alignment::Center);

    column![counter_row, pct_text]
        .spacing(2)
        .align_x(Alignment::End)
        .into()
}

pub fn breakdown_row<'a, M: 'a>(summary: &WidgetSummary) -> Element<'a, M> {
    row![
        breakdown_label::<M>(),
        iced::widget::Space::new().width(Length::Fill),
        earnings_row::<M>(summary),
    ]
    .align_y(Alignment::End)
    .width(Length::Fill)
    .into()
}

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
        let label_text = text(display_label).size(11).color(C_TEXT_MUTED);

        let info_col = column![number, label_text, pct_text]
            .spacing(2)
            .padding(Padding::default().left(8).right(6).top(6).bottom(6));

        row![stripe, info_col].into()
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

    let remaining_label = text(format_remaining(remaining as u64))
        .size(11)
        .color(C_TEXT_MUTED);
    let pct_label = text(format!("{pct_to_go:.1}% to go"))
        .size(11)
        .color(C_TEXT_DIM);

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

pub fn tick_marks<'a, M: 'a + Clone>(unlocked_pct: f32) -> Element<'a, M> {
    const THRESHOLDS: [u8; 5] = [0, 25, 50, 75, 100];
    let lit_color = overall_tier_color(unlocked_pct);

    let mut ticks_row: iced::widget::Row<'a, M> = row![].spacing(0);

    for (i, threshold) in THRESHOLDS.iter().enumerate() {
        let lit = tick_lit_at(unlocked_pct, *threshold);
        let tick_color = if lit { lit_color } else { C_TEXT_MUTED };

        let dot = container(iced::widget::Space::new())
            .width(Length::Fixed(6.0))
            .height(Length::Fixed(6.0))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(tick_color)),
                border: Border {
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });

        let label = text(format!("{threshold}%")).size(14).color(tick_color);
        let tick_unit = row![dot, label].spacing(3).align_y(Alignment::Center);

        let tick_pct = *threshold as f32;
        let fill_before = if i == 0 {
            tick_pct - 0.5
        } else {
            tick_pct - THRESHOLDS[i - 1] as f32 - 0.5
        };
        let fill_before = fill_before.max(0.0) as u16;

        if fill_before > 0 {
            ticks_row = ticks_row.push(
                iced::widget::Space::new()
                    .width(Length::FillPortion(fill_before))
                    .height(Length::Fixed(20.0)),
            );
        }
        ticks_row = ticks_row.push(tick_unit);
    }

    ticks_row
        .width(Length::Fill)
        .height(Length::Fixed(20.0))
        .into()
}

pub fn rarity_bar<'a, M: 'a + Clone>(summary: WidgetSummary) -> RarityBar<'a, M> {
    RarityBar {
        summary,
        hovered: None,
        on_hover: None,
        _phantom: PhantomData,
    }
}

pub struct RarityBar<'a, M> {
    summary: WidgetSummary,
    hovered: Option<RarityTier>,
    on_hover: Option<Box<dyn Fn(Option<RarityTier>) -> M + 'a>>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M: 'a + Clone> RarityBar<'a, M> {
    pub fn hovered(mut self, tier: Option<RarityTier>) -> Self {
        self.hovered = tier;
        self
    }

    pub fn on_hover<F>(mut self, f: F) -> Self
    where
        F: Fn(Option<RarityTier>) -> M + 'a,
    {
        self.on_hover = Some(Box::new(f));
        self
    }
}

impl<'a, M: 'a + Clone> From<RarityBar<'a, M>> for Element<'a, M> {
    fn from(b: RarityBar<'a, M>) -> Self {
        let summary = b.summary;
        let tier_counts: [(RarityTier, u32); 5] = [
            (RarityTier::Common, summary.common_count),
            (RarityTier::Uncommon, summary.uncommon_count),
            (RarityTier::Rare, summary.rare_count),
            (RarityTier::Mythical, summary.mythical_count),
            (RarityTier::Legendary, summary.legendary_count),
        ];

        let total = summary.achievement_total;
        let total_for_pct = total.max(1);

        let mut segments: Vec<BarSegment> = Vec::new();
        let mut tier_at: Vec<Option<RarityTier>> = Vec::new();
        let mut tooltips: Vec<String> = Vec::new();

        for (tier, count) in tier_counts.iter() {
            if *count == 0 {
                continue;
            }
            let pct = *count as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: *count,
                color: rarity_color(*tier),
            });
            tier_at.push(Some(*tier));
            tooltips.push(format!(
                "{} {} \u{00B7} {:.1}%",
                format_thousands(*count),
                rarity_label(*tier),
                pct
            ));
        }

        let unrated = summary.unrated_unlocked();
        if unrated > 0 {
            let pct = unrated as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: unrated,
                color: C_ACCENT,
            });
            tier_at.push(None);
            tooltips.push(format!(
                "{} Unrated \u{00B7} {:.1}%",
                format_thousands(unrated),
                pct
            ));
        }

        let locked = summary.locked();
        if locked > 0 {
            let pct = locked as f64 / total_for_pct as f64 * 100.0;
            segments.push(BarSegment {
                weight: locked,
                color: palette(AppTheme::Dark).hover,
            });
            tier_at.push(None);
            tooltips.push(format!(
                "{} Locked \u{00B7} {:.1}%",
                format_thousands(locked),
                pct
            ));
        }

        let hovered_idx = b
            .hovered
            .and_then(|t| tier_at.iter().position(|x| *x == Some(t)));

        let tip_lookup = tooltips.clone();
        let mut bar_builder = segmented_bar(segments, Length::Fill, BAR_HEIGHT)
            .theme(AppTheme::Dark)
            .radius(BAR_RADIUS)
            .hovered(hovered_idx)
            .tooltip(move |idx| tip_lookup.get(idx).cloned().unwrap_or_default());

        if let Some(on_hover) = b.on_hover {
            let tier_lookup = tier_at.clone();
            bar_builder = bar_builder.on_hover(move |idx| {
                let tier = idx.and_then(|i| tier_lookup.get(i).copied().flatten());
                on_hover(tier)
            });
        }

        let bar: Element<'a, M> = bar_builder.into();
        let ticks_layer: Element<'a, M> = tick_marks(summary.unlocked_pct());

        column![bar, ticks_layer].spacing(4).into()
    }
}

pub fn widget_panel<'a, M: 'a>(left: Element<'a, M>, right: Element<'a, M>) -> Element<'a, M> {
    let surface_style = || container::Style {
        background: Some(Background::Color(C_SURFACE)),
        border: Border {
            radius: 10.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    };

    let two_col_row = row![
        container(left)
            .width(Length::FillPortion(5))
            .height(Length::Fixed(WIDGET_ROW_HEIGHT))
            .padding(18)
            .style(move |_: &iced::Theme| surface_style()),
        container(right)
            .width(Length::FillPortion(2))
            .height(Length::Fixed(WIDGET_ROW_HEIGHT))
            .padding(16)
            .style(move |_: &iced::Theme| surface_style()),
    ]
    .spacing(16);

    container(two_col_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12))
        .into()
}

pub fn closest_row<'a, M: 'a + Clone>(
    image: Element<'a, M>,
    primary: impl Into<Cow<'a, str>>,
    secondary: impl Into<Cow<'a, str>>,
    pct_label: impl Into<Cow<'a, str>>,
    on_press: M,
) -> Element<'a, M> {
    let primary_label = text(primary.into())
        .size(12)
        .color(C_TEXT_PRIMARY)
        .wrapping(text::Wrapping::None);
    let secondary_label = text(secondary.into()).size(11).color(C_TEXT_MUTED);

    let info_col = column![
        container(primary_label).width(Length::Fill).clip(true),
        secondary_label,
    ]
    .spacing(1)
    .width(Length::Fill);

    let pct_chip = text(pct_label.into()).size(13).color(C_ACCENT);

    let row_content = row![image, info_col, pct_chip]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(6).right(6).top(5).bottom(5));

    let row_container = container(row_content).style(|_: &iced::Theme| container::Style {
        background: Some(Background::Color(C_HOVER)),
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });

    button(row_container)
        .on_press(on_press)
        .padding(0)
        .style(|_: &iced::Theme, _status| button::Style {
            background: None,
            ..button::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_thousands_basic() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(42), "42");
        assert_eq!(format_thousands(1_234), "1,234");
        assert_eq!(format_thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn summary_derives_locked_and_pct() {
        let s = WidgetSummary {
            earned_total: 75,
            achievement_total: 100,
            legendary_count: 5,
            mythical_count: 10,
            rare_count: 15,
            uncommon_count: 20,
            common_count: 25,
        };
        assert_eq!(s.locked(), 25);
        assert_eq!(s.rated_unlocked(), 75);
        assert_eq!(s.unrated_unlocked(), 0);
        assert!((s.unlocked_pct() - 75.0).abs() < 0.01);
        assert!((s.pct_to_go() - 25.0).abs() < 0.01);
    }

    #[test]
    fn summary_zero_total_safe() {
        let s = WidgetSummary::default();
        assert_eq!(s.locked(), 0);
        assert_eq!(s.unlocked_pct(), 0.0);
        assert_eq!(s.pct_to_go(), 0.0);
    }

    #[test]
    fn overall_tier_color_thresholds() {
        assert_eq!(overall_tier_color(0.0), C_TEXT_MUTED);
        assert_eq!(overall_tier_color(1.0), C_RARITY_COMMON);
        assert_eq!(overall_tier_color(25.0), C_RARITY_UNCOMMON);
        assert_eq!(overall_tier_color(50.0), C_RARITY_RARE);
        assert_eq!(overall_tier_color(75.0), C_RARITY_MYTHICAL);
        assert_eq!(overall_tier_color(100.0), C_RARITY_LEGENDARY);
    }
}
