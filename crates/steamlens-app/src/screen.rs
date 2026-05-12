use std::sync::LazyLock;

use iced::widget::Id as WidgetId;
use iced::widget::{Space, button, column, container, image as img_widget, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

static HEADER_ICON: LazyLock<iced::widget::image::Handle> = LazyLock::new(|| {
    const BYTES: &[u8] = include_bytes!("../../../assets/icon-48.png");
    iced::widget::image::Handle::from_bytes(BYTES.to_vec())
});

use crate::ui::theme::{palette, theme_from_iced};

const FILTER_PAD_X: f32 = 12.0;
const FILTER_PAD_Y: f32 = 6.0;
const FILTER_RADIUS: f32 = 6.0;
const FILTER_SPACING: f32 = 6.0;
const STRIP_DIVIDER_SPACING: f32 = 12.0;

const SEGMENT_PAD_X: f32 = 10.0;
const SEGMENT_PAD_Y: f32 = 6.0;
const SEGMENT_RADIUS: f32 = 6.0;
const SEGMENT_DIVIDER_H: f32 = 20.0;

pub struct SearchConfig<'a> {
    pub placeholder: &'a str,
    pub value: &'a str,
    pub id: WidgetId,
}

pub struct FilterButton<'a> {
    pub label: std::borrow::Cow<'a, str>,
    pub selected: bool,
    pub on_press: crate::Message,
}

pub struct FilterStrip<'a> {
    pub buttons: Vec<FilterButton<'a>>,
}

pub struct SegmentItem<'a> {
    pub label: std::borrow::Cow<'a, str>,
    pub tooltip: Option<&'a str>,
    pub selected: bool,
    pub on_press: crate::Message,
}

pub struct SegmentedControlConfig<'a> {
    pub label: Option<&'a str>,
    pub items: Vec<SegmentItem<'a>>,
}

pub struct AppHeaderContent<'a> {
    pub search: Option<SearchConfig<'a>>,
    pub segments: Vec<SegmentedControlConfig<'a>>,
    pub screen_actions: Vec<Element<'a, crate::Message>>,
    pub leading: Option<Element<'a, crate::Message>>,
    pub status_filter: Option<FilterStrip<'a>>,
    pub category_filter: Option<Element<'a, crate::Message>>,
    pub theme: crate::ui::theme::AppTheme,
}

pub fn render_app_header(content: AppHeaderContent<'_>) -> Element<'_, crate::Message> {
    let mut top_row = row![].spacing(12).align_y(Alignment::Center);

    {
        let brand_icon = img_widget(HEADER_ICON.clone())
            .width(Length::Fixed(26.0))
            .height(Length::Fixed(26.0));
        let title_text = text("SteamLens").size(22).style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            iced::widget::text::Style {
                color: Some(p.accent),
            }
        });
        let brand_row = row![brand_icon, title_text]
            .spacing(8)
            .align_y(Alignment::Center);
        top_row = top_row.push(brand_row);
    }

    if let Some(cfg) = content.search {
        top_row = top_row.push(build_search_input(cfg));
    }
    for seg_cfg in content.segments {
        top_row = top_row.push(build_segmented_control(seg_cfg));
    }
    top_row = top_row.push(Space::new().width(Length::Fill));
    for action in content.screen_actions {
        top_row = top_row.push(action);
    }
    top_row = top_row.push(build_global_actions(content.theme));

    let top_row = top_row.width(Length::Fill);

    let has_second_row = content.leading.is_some()
        || content.status_filter.is_some()
        || content.category_filter.is_some();

    let inner: Element<'_, crate::Message> = if has_second_row {
        let mut second = row![].spacing(0).align_y(Alignment::Center);
        let mut prev_present = false;

        if let Some(leading) = content.leading {
            second = second.push(leading);
            prev_present = true;
        }

        if let Some(strip) = content.status_filter {
            if prev_present {
                second = second.push(build_strip_divider());
                second = second.push(Space::new().width(Length::Fixed(STRIP_DIVIDER_SPACING)));
            }
            second = second.push(build_filter_strip(strip));
            prev_present = true;
        }

        if let Some(category) = content.category_filter {
            if prev_present {
                second = second.push(Space::new().width(Length::Fixed(STRIP_DIVIDER_SPACING)));
                second = second.push(build_strip_divider());
                second = second.push(Space::new().width(Length::Fixed(STRIP_DIVIDER_SPACING)));
            }
            second = second.push(category);
        }

        let second_row = second.width(Length::Fill);

        column![top_row, second_row].spacing(8).into()
    } else {
        top_row.into()
    };

    container(inner)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12))
        .style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Color(p.surface)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn build_strip_divider() -> Element<'static, crate::Message> {
    container(Space::new())
        .width(Length::Fixed(1.0))
        .height(Length::Fixed(20.0))
        .style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Color(p.border)),
                ..container::Style::default()
            }
        })
        .into()
}

fn build_filter_strip(strip: FilterStrip<'_>) -> Element<'_, crate::Message> {
    row(strip.buttons.into_iter().map(build_filter_button))
        .spacing(FILTER_SPACING)
        .align_y(Alignment::Center)
        .into()
}

fn build_filter_button(cfg: FilterButton<'_>) -> Element<'_, crate::Message> {
    let selected = cfg.selected;
    let label_text = text(cfg.label).size(12).style(move |t: &iced::Theme| {
        let p = palette(theme_from_iced(t));
        iced::widget::text::Style {
            color: Some(if selected { p.accent } else { p.text_muted }),
        }
    });

    button(label_text)
        .on_press(cfg.on_press)
        .padding(
            Padding::default()
                .left(FILTER_PAD_X)
                .right(FILTER_PAD_X)
                .top(FILTER_PAD_Y)
                .bottom(FILTER_PAD_Y),
        )
        .style(move |t: &iced::Theme, status| {
            let p = palette(theme_from_iced(t));
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(Background::Color(if selected {
                    Color {
                        a: 0.15,
                        ..p.accent
                    }
                } else if hovered {
                    p.hover
                } else {
                    Color::TRANSPARENT
                })),
                border: Border {
                    color: if selected { p.accent } else { p.border },
                    width: 1.0,
                    radius: FILTER_RADIUS.into(),
                },
                text_color: if selected {
                    p.accent
                } else if hovered {
                    p.text_primary
                } else {
                    p.text_muted
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn build_segmented_control(cfg: SegmentedControlConfig<'_>) -> Element<'_, crate::Message> {
    if cfg.items.is_empty() {
        return Space::new()
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into();
    }

    let last_idx = cfg.items.len().saturating_sub(1);
    let mut items: Vec<Element<'_, crate::Message>> = Vec::new();

    for (idx, item) in cfg.items.into_iter().enumerate() {
        let selected = item.selected;
        let hint = item.tooltip;

        let btn = button(text(item.label).size(12).style(move |t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            iced::widget::text::Style {
                color: Some(if selected { p.accent } else { p.text_muted }),
            }
        }))
        .on_press(item.on_press)
        .padding(
            Padding::default()
                .left(SEGMENT_PAD_X)
                .right(SEGMENT_PAD_X)
                .top(SEGMENT_PAD_Y)
                .bottom(SEGMENT_PAD_Y),
        )
        .style(move |t: &iced::Theme, status| {
            let p = palette(theme_from_iced(t));
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(Background::Color(if selected {
                    Color {
                        a: 0.15,
                        ..p.accent
                    }
                } else if hovered {
                    p.hover
                } else {
                    Color::TRANSPARENT
                })),
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                text_color: if selected {
                    p.accent
                } else if hovered {
                    p.text_primary
                } else {
                    p.text_muted
                },
                ..iced::widget::button::Style::default()
            }
        });

        let item_el: Element<'_, crate::Message> = if let Some(hint_str) = hint {
            use crate::ui::widgets::tooltip_box::tooltip_box;
            tooltip_box(
                btn,
                hint_str.to_owned(),
                iced::widget::tooltip::Position::Bottom,
            )
        } else {
            btn.into()
        };

        items.push(item_el);

        if idx < last_idx {
            let divider = container(Space::new())
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(SEGMENT_DIVIDER_H))
                .style(|t: &iced::Theme| {
                    let p = palette(theme_from_iced(t));
                    container::Style {
                        background: Some(Background::Color(p.border)),
                        ..container::Style::default()
                    }
                });
            items.push(divider.into());
        }
    }

    let segment_container =
        container(row(items).align_y(Alignment::Center)).style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Color(p.surface)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: SEGMENT_RADIUS.into(),
                },
                ..container::Style::default()
            }
        });

    if let Some(label_str) = cfg.label {
        row![
            text(label_str).size(11).style(|t: &iced::Theme| {
                let p = palette(theme_from_iced(t));
                iced::widget::text::Style {
                    color: Some(p.text_muted),
                }
            }),
            segment_container
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        segment_container.into()
    }
}

fn build_search_input(cfg: SearchConfig<'_>) -> Element<'_, crate::Message> {
    let input = text_input(cfg.placeholder, cfg.value)
        .id(cfg.id)
        .on_input(crate::Message::GlobalSearchChanged)
        .padding(Padding::default().left(10).right(10).top(6).bottom(6))
        .size(13)
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
                placeholder: p.text_muted,
                value: p.text_primary,
                selection: Color { a: 0.3, ..p.accent },
            }
        })
        .width(Length::Fill);

    let kbd_badge = container(text("Ctrl K").size(10).style(|t: &iced::Theme| {
        let p = palette(theme_from_iced(t));
        iced::widget::text::Style {
            color: Some(p.text_dim),
        }
    }))
    .padding(Padding::default().left(6).right(6).top(2).bottom(2))
    .style(|t: &iced::Theme| {
        let p = palette(theme_from_iced(t));
        container::Style {
            background: Some(Background::Color(p.border)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        }
    });

    let inner_row = row![input, kbd_badge]
        .spacing(6)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(0).right(8));

    container(inner_row)
        .width(Length::Fixed(320.0))
        .style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            container::Style {
                background: Some(Background::Color(p.surface)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            }
        })
        .padding(Padding::default().left(8).right(8).top(0).bottom(0))
        .into()
}

fn build_global_actions(
    current_theme: crate::ui::theme::AppTheme,
) -> Element<'static, crate::Message> {
    use crate::theme::{C_BORDER, C_HOVER, C_TEXT_MUTED, C_TEXT_PRIMARY};
    use crate::ui::widgets::tooltip_box::tooltip_box;

    let make_icon_btn = |glyph: &'static str, on_press: crate::Message| {
        button(
            container(text(glyph).size(14).color(C_TEXT_MUTED))
                .width(Length::Fixed(32.0))
                .height(Length::Fixed(32.0))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .on_press(on_press)
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
                    iced::Color::TRANSPARENT
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
    };

    let (theme_glyph, theme_tooltip) = match current_theme {
        crate::ui::theme::AppTheme::Dark => ("\u{263C}", "Switch to light theme"),
        crate::ui::theme::AppTheme::Light => ("\u{263E}", "Switch to dark theme"),
    };

    row![
        tooltip_box(
            make_icon_btn(theme_glyph, crate::Message::ToggleTheme),
            theme_tooltip,
            iced::widget::tooltip::Position::Bottom,
        ),
        tooltip_box(
            make_icon_btn("\u{24D8}", crate::Message::ShowAbout),
            "About",
            iced::widget::tooltip::Position::Bottom,
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

pub struct ScreenContent<'a, M> {
    pub top: Option<Element<'a, M>>,
    pub status_bar: Option<Element<'a, M>>,
    pub body: Element<'a, M>,
    pub footer: Option<Element<'a, M>>,
}

pub fn compose_screen<'a, M: 'a>(content: ScreenContent<'a, M>) -> Element<'a, M> {
    let mut col = column![].spacing(0);

    if let Some(top) = content.top {
        col = col.push(top);
    }

    if let Some(status_bar) = content.status_bar {
        col = col.push(status_bar);
    }

    col = col.push(
        container(content.body)
            .width(Length::Fill)
            .height(Length::Fill),
    );

    if let Some(footer) = content.footer {
        let footer_chrome = container(footer)
            .width(Length::Fill)
            .padding(Padding::default().left(16).right(16).top(12).bottom(12))
            .style(|t: &iced::Theme| {
                let p = palette(theme_from_iced(t));
                container::Style {
                    background: Some(Background::Color(p.surface)),
                    border: Border {
                        color: p.border,
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }
            });
        col = col.push(footer_chrome);
    }

    col.height(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text;

    #[test]
    fn compose_screen_minimal_content() {
        let content = ScreenContent::<()> {
            top: None,
            status_bar: None,
            body: text("b").into(),
            footer: None,
        };
        let _: Element<'_, ()> = compose_screen(content);
    }

    #[test]
    fn compose_screen_full_content() {
        let content = ScreenContent::<()> {
            top: Some(text("t").into()),
            status_bar: Some(text("s").into()),
            body: text("b").into(),
            footer: Some(text("f").into()),
        };
        let _: Element<'_, ()> = compose_screen(content);
    }

    #[test]
    fn build_segmented_control_with_label_and_items_compiles() {
        let items = vec![
            SegmentItem {
                label: std::borrow::Cow::Borrowed("A"),
                tooltip: Some("Option A"),
                selected: true,
                on_press: crate::Message::GoBack,
            },
            SegmentItem {
                label: std::borrow::Cow::Borrowed("B"),
                tooltip: None,
                selected: false,
                on_press: crate::Message::GoBack,
            },
        ];
        let cfg = SegmentedControlConfig {
            label: Some("TEST"),
            items,
        };
        let content = AppHeaderContent {
            search: None,
            segments: vec![cfg],
            screen_actions: vec![],
            leading: None,
            status_filter: None,
            category_filter: None,
            theme: crate::ui::theme::AppTheme::Dark,
        };
        let _: Element<'_, crate::Message> = render_app_header(content);
    }

    #[test]
    fn build_segmented_control_empty_items_returns_space() {
        let cfg = SegmentedControlConfig {
            label: Some("EMPTY"),
            items: vec![],
        };
        let content = AppHeaderContent {
            search: None,
            segments: vec![cfg],
            screen_actions: vec![],
            leading: None,
            status_filter: None,
            category_filter: None,
            theme: crate::ui::theme::AppTheme::Dark,
        };
        let _: Element<'_, crate::Message> = render_app_header(content);
    }
}
