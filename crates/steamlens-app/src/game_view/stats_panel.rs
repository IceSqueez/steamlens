use std::sync::LazyLock;

use iced::widget::{Space, button, column, container, row, scrollable, svg, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use super::GameViewMessage;
use super::types::{StatRow, StatValue};
use crate::ui::theme::{palette, theme_from_iced};

const C_GREEN: Color = Color::from_rgb(0.427, 0.788, 0.498);
const C_DANGER: Color = Color::from_rgb(0.863, 0.392, 0.392);
const C_DIRTY: Color = Color::from_rgb(0.945, 0.980, 0.549);
const C_VALUE_IN_PROGRESS: Color = Color::from_rgb(
    0xaa as f32 / 255.0,
    0xa6 as f32 / 255.0,
    0xc0 as f32 / 255.0,
);

const PROGRESS_BAR_HEIGHT: f32 = 2.0;

static SVG_CHECK: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#6dc97f" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

static SVG_RESET: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#dc6464" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9"/><path d="M3 4v5h5"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

static SVG_SEARCH: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#6b6884" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><path d="m20 20-3-3"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

pub fn build_stats_panel<'a>(
    stats: &'a [StatRow],
    search_query: &'a str,
) -> Element<'a, GameViewMessage> {
    let header = build_header(stats);
    let search = build_search_input(search_query);

    let header_section = column![header, search]
        .spacing(12)
        .padding(Padding::default().left(2).right(2).top(2).bottom(14));

    let header_with_divider = column![header_section, divider()].spacing(0);

    let filtered: Vec<&StatRow> = stats
        .iter()
        .filter(|s| stat_matches(s, search_query))
        .collect();

    let list: Element<'a, GameViewMessage> = if filtered.is_empty() {
        container(
            text(if stats.is_empty() {
                "No stats available for this game"
            } else {
                "No stats match the search"
            })
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            }),
        )
        .width(Length::Fill)
        .padding(Padding::default().top(20).bottom(20))
        .align_x(Alignment::Center)
        .into()
    } else {
        let mut col = column![].spacing(0);
        for (idx, stat) in filtered.iter().enumerate() {
            col = col.push(stat_row(stat));
            if idx + 1 < filtered.len() {
                col = col.push(divider());
            }
        }
        scrollable(col).height(Length::Fill).into()
    };

    column![header_with_divider, list]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

fn build_header<'a>(stats: &'a [StatRow]) -> Element<'a, GameViewMessage> {
    let (total, maxed, in_progress) = summarize(stats);

    let title = text("IN-GAME STATISTICS")
        .size(11)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_muted),
        });
    let subtitle = text(format!(
        "{total} stats \u{00B7} {maxed} maxed \u{00B7} {in_progress} in progress"
    ))
    .size(10)
    .style(|t: &iced::Theme| iced::widget::text::Style {
        color: Some(palette(theme_from_iced(t)).text_dim),
    });

    let left_col = column![title, subtitle].spacing(2);

    let any_capped = stats.iter().any(|s| s.data.max_value.is_some());

    let cap_all_btn = action_button(
        SVG_CHECK.clone(),
        "Cap all",
        C_GREEN,
        GameViewMessage::StatsMaxAll,
        !any_capped,
    );
    let reset_all_btn = action_button(
        SVG_RESET.clone(),
        "Reset all",
        C_DANGER,
        GameViewMessage::StatsResetAll,
        false,
    );

    let action_row = row![cap_all_btn, reset_all_btn]
        .spacing(6)
        .align_y(Alignment::Center);

    row![left_col, Space::new().width(Length::Fill), action_row,]
        .align_y(Alignment::Center)
        .into()
}

fn action_button(
    icon: svg::Handle,
    label: &'static str,
    tint: Color,
    msg: GameViewMessage,
    disabled: bool,
) -> Element<'static, GameViewMessage> {
    let icon_el = svg(icon)
        .width(Length::Fixed(11.0))
        .height(Length::Fixed(11.0))
        .style(move |t: &iced::Theme, _status| {
            let color = if disabled {
                palette(theme_from_iced(t)).text_dim
            } else {
                tint
            };
            iced::widget::svg::Style { color: Some(color) }
        });

    let label_el = text(label).size(11).style(move |t: &iced::Theme| {
        let color = if disabled {
            palette(theme_from_iced(t)).text_dim
        } else {
            tint
        };
        iced::widget::text::Style { color: Some(color) }
    });

    let inner = row![icon_el, label_el]
        .spacing(5)
        .align_y(Alignment::Center);

    let mut btn = button(inner).padding(Padding::default().left(10).right(10).top(5).bottom(5));
    if !disabled {
        btn = btn.on_press(msg);
    }
    btn.style(move |t: &iced::Theme, status| {
        let hovered =
            !disabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        let text_color = if disabled {
            palette(theme_from_iced(t)).text_dim
        } else {
            tint
        };
        let bg_alpha = if disabled {
            0.04
        } else if hovered {
            0.20
        } else {
            0.12
        };
        let border_alpha = if disabled {
            0.18
        } else if hovered {
            0.55
        } else {
            0.35
        };
        button::Style {
            background: Some(Background::Color(Color {
                a: bg_alpha,
                ..tint
            })),
            border: Border {
                color: Color {
                    a: border_alpha,
                    ..tint
                },
                width: 1.0,
                radius: 5.0.into(),
            },
            text_color,
            ..button::Style::default()
        }
    })
    .into()
}

fn build_search_input(query: &str) -> Element<'_, GameViewMessage> {
    let icon = svg(SVG_SEARCH.clone())
        .width(Length::Fixed(12.0))
        .height(Length::Fixed(12.0));

    let input = text_input("Search stats...", query)
        .on_input(GameViewMessage::StatsSearchChanged)
        .size(12)
        .padding(Padding::default().left(6).right(8).top(6).bottom(6))
        .style(|t: &iced::Theme, _status| {
            let p = palette(theme_from_iced(t));
            iced::widget::text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                icon: p.text_muted,
                placeholder: p.text_dim,
                value: p.text_primary,
                selection: Color { a: 0.3, ..p.accent },
            }
        });

    let inner = row![icon, input].spacing(0).align_y(Alignment::Center);

    container(inner)
        .width(Length::Fill)
        .padding(Padding::default().left(9).right(2).top(0).bottom(0))
        .style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Color(p.hover)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn stat_row(row_data: &StatRow) -> Element<'_, GameViewMessage> {
    let (current, maxed, has_progress) = current_max_pair(row_data);
    let is_dirty = row_data.is_dirty;

    let name_text =
        text(row_data.data.display_name.clone())
            .size(12)
            .style(move |t: &iced::Theme| {
                let p = palette(theme_from_iced(t));
                let color = if is_dirty {
                    C_DIRTY
                } else if !has_progress {
                    p.text_dim
                } else {
                    p.text_primary
                };
                iced::widget::text::Style { color: Some(color) }
            });

    let dirty_dot: Option<Element<'_, GameViewMessage>> = if is_dirty {
        Some(
            container(Space::new())
                .width(Length::Fixed(6.0))
                .height(Length::Fixed(6.0))
                .style(|_: &iced::Theme| container::Style {
                    background: Some(Background::Color(C_DIRTY)),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into(),
        )
    } else {
        None
    };

    let max_badge: Option<Element<'_, GameViewMessage>> = if maxed && !is_dirty {
        let badge = text("MAX").size(9).color(C_GREEN);
        Some(
            container(badge)
                .padding(Padding::default().left(4).right(4).top(1).bottom(1))
                .style(|_: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color { a: 0.12, ..C_GREEN })),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .into(),
        )
    } else {
        None
    };

    let mut name_row = row![].spacing(6).align_y(Alignment::Center);
    if let Some(dot) = dirty_dot {
        name_row = name_row.push(dot);
    }
    name_row = name_row.push(name_text);
    if let Some(badge) = max_badge {
        name_row = name_row.push(badge);
    }
    let name_block: Element<'_, GameViewMessage> = name_row.into();

    let value_text = text(format_value(current, row_data.data.max_value, maxed))
        .size(10)
        .style(move |t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            let color = if is_dirty {
                C_DIRTY
            } else if !has_progress {
                p.text_dim
            } else if maxed {
                C_GREEN
            } else {
                C_VALUE_IN_PROGRESS
            };
            iced::widget::text::Style { color: Some(color) }
        });

    let header_row =
        row![name_block, Space::new().width(Length::Fill), value_text,].align_y(Alignment::Center);

    let progress_bar = build_progress_bar(current, row_data.data.max_value, maxed, is_dirty);

    let info_col = column![
        header_row,
        Space::new().height(Length::Fixed(4.0)),
        progress_bar
    ]
    .spacing(0)
    .width(Length::Fill);

    let is_protected = row_data.data.permission != 0;
    let has_cap = row_data.data.max_value.is_some();

    let mut actions = row![].spacing(4).align_y(Alignment::Center);
    if has_cap {
        actions = actions.push(max_button(row_data, maxed || is_protected));
    }
    actions = actions.push(reset_button(row_data, maxed, is_protected));

    let inner = row![info_col, actions]
        .spacing(10)
        .align_y(Alignment::Center);

    container(inner)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(9).bottom(9))
        .into()
}

fn build_progress_bar(
    current: f64,
    max: Option<u64>,
    maxed: bool,
    is_dirty: bool,
) -> Element<'static, GameViewMessage> {
    let ratio = match max {
        Some(m) if m > 0 => (current / m as f64).clamp(0.0, 1.0) as f32,
        _ => 0.0,
    };

    let portion_fill = ((ratio * 1000.0).round() as u16).clamp(0, 1000);
    let portion_rest = 1000u16.saturating_sub(portion_fill);

    let fill_widget: Element<'static, GameViewMessage> = if portion_fill == 0 {
        Space::new()
            .width(Length::Shrink)
            .height(Length::Fixed(PROGRESS_BAR_HEIGHT))
            .into()
    } else {
        container(Space::new())
            .width(Length::FillPortion(portion_fill))
            .height(Length::Fixed(PROGRESS_BAR_HEIGHT))
            .style(move |t: &iced::Theme| {
                let fill_color = if is_dirty {
                    C_DIRTY
                } else if maxed {
                    C_GREEN
                } else {
                    palette(theme_from_iced(t)).accent
                };
                container::Style {
                    background: Some(Background::Color(fill_color)),
                    border: Border {
                        radius: 1.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            })
            .into()
    };

    let rest_widget: Element<'static, GameViewMessage> = container(Space::new())
        .width(Length::FillPortion(portion_rest.max(1)))
        .height(Length::Fixed(PROGRESS_BAR_HEIGHT))
        .into();

    container(row![fill_widget, rest_widget])
        .width(Length::Fill)
        .height(Length::Fixed(PROGRESS_BAR_HEIGHT))
        .style(|t: &iced::Theme| container::Style {
            background: Some(Background::Color(palette(theme_from_iced(t)).hover)),
            border: Border {
                radius: 1.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn neutral_row_button(
    label: &'static str,
    msg: GameViewMessage,
    disabled: bool,
) -> Element<'static, GameViewMessage> {
    let lbl = text(label).size(10).style(move |t: &iced::Theme| {
        let p = palette(theme_from_iced(t));
        let color = if disabled { p.text_dim } else { p.text_muted };
        iced::widget::text::Style { color: Some(color) }
    });

    let mut btn = button(lbl).padding(Padding::default().left(7).right(7).top(3).bottom(3));
    if !disabled {
        btn = btn.on_press(msg);
    }

    btn.style(move |t: &iced::Theme, status| {
        let hovered =
            !disabled && matches!(status, button::Status::Hovered | button::Status::Pressed);
        let p = palette(theme_from_iced(t));
        let label_color = if disabled { p.text_dim } else { p.text_muted };
        button::Style {
            background: if hovered {
                Some(Background::Color(Color {
                    a: 0.20,
                    ..p.text_muted
                }))
            } else {
                None
            },
            border: Border {
                color: if hovered {
                    Color {
                        a: 0.60,
                        ..p.text_muted
                    }
                } else {
                    p.border
                },
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: if hovered { p.text_primary } else { label_color },
            ..button::Style::default()
        }
    })
    .into()
}

fn reset_button(
    row_data: &StatRow,
    _maxed: bool,
    is_protected: bool,
) -> Element<'static, GameViewMessage> {
    let default = row_data.data.default_value.unwrap_or(0);
    let at_default = match row_data.data.value {
        StatValue::Int(v) => v as i64 == default,
        StatValue::Float(v) => (v as f64) == default as f64,
    };
    let disabled = at_default || is_protected;
    neutral_row_button(
        "Reset",
        GameViewMessage::StatsResetSingle(row_data.data.id.clone()),
        disabled,
    )
}

fn max_button(row_data: &StatRow, disabled: bool) -> Element<'static, GameViewMessage> {
    neutral_row_button(
        "Max",
        GameViewMessage::StatsMaxSingle(row_data.data.id.clone()),
        disabled,
    )
}

fn divider<M: 'static>() -> Element<'static, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|t: &iced::Theme| container::Style {
            background: Some(Background::Color(palette(theme_from_iced(t)).hover)),
            ..container::Style::default()
        })
        .into()
}

fn current_max_pair(r: &StatRow) -> (f64, bool, bool) {
    let current = match r.data.value {
        StatValue::Int(v) => v as f64,
        StatValue::Float(v) => v as f64,
    };
    let max = r.data.max_value.unwrap_or(0);
    let maxed = max > 0 && current >= max as f64;
    let has_progress = current > 0.0;
    (current, maxed, has_progress)
}

fn summarize(stats: &[StatRow]) -> (usize, usize, usize) {
    let mut maxed = 0;
    let mut in_progress = 0;
    for r in stats {
        let (current, is_maxed, has_progress) = current_max_pair(r);
        let _ = current;
        if is_maxed {
            maxed += 1;
        } else if has_progress {
            in_progress += 1;
        }
    }
    (stats.len(), maxed, in_progress)
}

fn stat_matches(r: &StatRow, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    r.data.display_name.to_lowercase().contains(&q) || r.data.id.to_lowercase().contains(&q)
}

fn format_value(current: f64, max: Option<u64>, maxed: bool) -> String {
    let current_str = format_human(current);
    match max {
        Some(m) if m > 0 => {
            let max_str = format_human(m as f64);
            if maxed {
                format!("{max_str} / {max_str}")
            } else {
                format!("{current_str} / {max_str}")
            }
        }
        _ => current_str,
    }
}

fn format_human(n: f64) -> String {
    let abs = n.abs();
    if abs >= 1_000_000.0 {
        format!("{:.0}M", n / 1_000_000.0)
    } else if abs >= 10_000.0 {
        format!("{:.0}k", n / 1_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}k", n / 1_000.0)
    } else if n.fract() == 0.0 {
        format!("{n:.0}")
    } else {
        format!("{n:.1}")
    }
}
