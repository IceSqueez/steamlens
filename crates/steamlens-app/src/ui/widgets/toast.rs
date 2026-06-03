use std::borrow::Cow;
use std::marker::PhantomData;

use iced::border::Radius;
use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::ui::theme::{palette, theme_from_iced};

const C_SUCCESS: Color = Color::from_rgb(0.427, 0.788, 0.498);
const C_ERROR: Color = Color::from_rgb(0.863, 0.392, 0.392);
const INFO_BLUE: Color = Color::from_rgb(0.373, 0.643, 0.827);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    Success,
    #[default]
    Info,
    Error,
}

impl Kind {
    fn accent(self) -> Color {
        match self {
            Kind::Success => C_SUCCESS,
            Kind::Info => INFO_BLUE,
            Kind::Error => C_ERROR,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Kind::Success => "\u{2713}",
            Kind::Info => "i",
            Kind::Error => "\u{2715}",
        }
    }
}

pub fn toast<'a, M: 'a + Clone>() -> Toast<'a, M> {
    Toast::default()
}

pub struct Toast<'a, M> {
    kind: Kind,
    title: Cow<'a, str>,
    body: Option<Cow<'a, str>>,
    action: Option<(Cow<'a, str>, M)>,
    on_close: Option<M>,
    on_hover_enter: Option<M>,
    on_hover_exit: Option<M>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M> Default for Toast<'a, M> {
    fn default() -> Self {
        Self {
            kind: Kind::default(),
            title: Cow::Borrowed(""),
            body: None,
            action: None,
            on_close: None,
            on_hover_enter: None,
            on_hover_exit: None,
            _phantom: PhantomData,
        }
    }
}

impl<'a, M: 'a + Clone> Toast<'a, M> {
    pub fn kind(mut self, k: Kind) -> Self {
        self.kind = k;
        self
    }

    pub fn title(mut self, t: impl Into<Cow<'a, str>>) -> Self {
        self.title = t.into();
        self
    }

    pub fn body(mut self, t: impl Into<Cow<'a, str>>) -> Self {
        self.body = Some(t.into());
        self
    }

    pub fn action(mut self, label: impl Into<Cow<'a, str>>, on_press: M) -> Self {
        self.action = Some((label.into(), on_press));
        self
    }

    pub fn on_close(mut self, msg: M) -> Self {
        self.on_close = Some(msg);
        self
    }

    pub fn on_hover_enter(mut self, msg: M) -> Self {
        self.on_hover_enter = Some(msg);
        self
    }

    pub fn on_hover_exit(mut self, msg: M) -> Self {
        self.on_hover_exit = Some(msg);
        self
    }
}

impl<'a, M: 'a + Clone> From<Toast<'a, M>> for Element<'a, M> {
    fn from(t: Toast<'a, M>) -> Self {
        let accent = t.kind.accent();
        let icon = text(t.kind.glyph()).size(14).color(accent);

        let mut info_col = column![text(t.title).size(12).style(|t: &iced::Theme| {
            iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_primary),
            }
        })]
        .spacing(1);
        if let Some(body) = t.body {
            info_col = info_col.push(text(body).size(10).style(|t: &iced::Theme| {
                iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).text_muted),
                }
            }));
        }

        let mut content_row = row![icon, info_col.width(Length::Fill)]
            .spacing(10)
            .align_y(Alignment::Center);

        if let Some((label, msg)) = t.action {
            content_row = content_row.push(link_button(label, msg));
        }

        if let Some(close_msg) = t.on_close {
            content_row = content_row.push(close_button(close_msg));
        }

        let inner = container(content_row)
            .width(Length::Fill)
            .padding(Padding::default().left(14).right(14).top(10).bottom(10))
            .style(move |t: &iced::Theme| container::Style {
                background: Some(Background::Color(palette(theme_from_iced(t)).surface)),
                border: Border {
                    color: Color { a: 0.30, ..accent },
                    width: 1.0,
                    radius: Radius {
                        top_left: 0.0,
                        bottom_left: 0.0,
                        top_right: 6.0,
                        bottom_right: 6.0,
                    },
                },
                ..container::Style::default()
            });

        let composed = container(inner)
            .width(Length::Fill)
            .padding(Padding::default().left(3))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: Radius {
                        top_left: 6.0,
                        bottom_left: 6.0,
                        top_right: 6.0,
                        bottom_right: 6.0,
                    },
                    ..Border::default()
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.40),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                ..container::Style::default()
            });

        match (t.on_hover_enter, t.on_hover_exit) {
            (Some(enter), Some(exit)) => mouse_area(composed).on_enter(enter).on_exit(exit).into(),
            (Some(enter), None) => mouse_area(composed).on_enter(enter).into(),
            (None, Some(exit)) => mouse_area(composed).on_exit(exit).into(),
            (None, None) => composed.into(),
        }
    }
}

fn close_button<'a, M: 'a + Clone>(msg: M) -> Element<'a, M> {
    button(
        container(
            text("\u{00D7}")
                .size(14)
                .style(|t: &iced::Theme| iced::widget::text::Style {
                    color: Some(palette(theme_from_iced(t)).text_muted),
                }),
        )
        .width(Length::Fixed(18.0))
        .height(Length::Fixed(18.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .on_press(msg)
    .padding(0)
    .style(move |t: &iced::Theme, status| {
        let p = palette(theme_from_iced(t));
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        iced::widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            text_color: if hovered {
                p.text_primary
            } else {
                p.text_muted
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn link_button<'a, M: 'a + Clone>(label: impl Into<Cow<'a, str>>, msg: M) -> Element<'a, M> {
    let label: Cow<'a, str> = label.into();
    button(
        text(label)
            .size(10)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            }),
    )
    .on_press(msg)
    .padding(Padding::default().left(4).right(4).top(2).bottom(2))
    .style(move |t: &iced::Theme, status| {
        let p = palette(theme_from_iced(t));
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        iced::widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            text_color: if hovered {
                p.text_primary
            } else {
                p.text_muted
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}
