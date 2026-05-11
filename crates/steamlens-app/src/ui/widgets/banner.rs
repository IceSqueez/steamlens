use std::borrow::Cow;
use std::marker::PhantomData;

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::theme::{C_ACCENT, C_TEXT_MUTED, C_TEXT_PRIMARY};

const C_WARNING: Color = Color::from_rgb(0.941, 0.784, 0.478);
const C_ERROR: Color = Color::from_rgb(0.863, 0.392, 0.392);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    #[default]
    Info,
    Warning,
    Error,
}

impl Severity {
    fn accent(self) -> Color {
        match self {
            Severity::Info => C_ACCENT,
            Severity::Warning => C_WARNING,
            Severity::Error => C_ERROR,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Severity::Info => "\u{2139}",
            Severity::Warning => "\u{26A0}",
            Severity::Error => "\u{26D4}",
        }
    }

    fn action_filled(self) -> bool {
        !matches!(self, Severity::Info)
    }
}

pub fn banner<'a, M: 'a + Clone>() -> Banner<'a, M> {
    Banner::default()
}

pub struct Banner<'a, M> {
    severity: Severity,
    title: Cow<'a, str>,
    text: Option<Cow<'a, str>>,
    action: Option<(Cow<'a, str>, M)>,
    on_dismiss: Option<M>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M> Default for Banner<'a, M> {
    fn default() -> Self {
        Self {
            severity: Severity::default(),
            title: Cow::Borrowed(""),
            text: None,
            action: None,
            on_dismiss: None,
            _phantom: PhantomData,
        }
    }
}

impl<'a, M: 'a + Clone> Banner<'a, M> {
    pub fn severity(mut self, s: Severity) -> Self {
        self.severity = s;
        self
    }

    pub fn title(mut self, t: impl Into<Cow<'a, str>>) -> Self {
        self.title = t.into();
        self
    }

    pub fn text(mut self, t: impl Into<Cow<'a, str>>) -> Self {
        self.text = Some(t.into());
        self
    }

    pub fn action(mut self, label: impl Into<Cow<'a, str>>, on_press: M) -> Self {
        self.action = Some((label.into(), on_press));
        self
    }

    pub fn on_dismiss(mut self, msg: M) -> Self {
        self.on_dismiss = Some(msg);
        self
    }
}

impl<'a, M: 'a + Clone> From<Banner<'a, M>> for Element<'a, M> {
    fn from(b: Banner<'a, M>) -> Self {
        let accent = b.severity.accent();
        let bg = Color { a: 0.08, ..accent };
        let border_color = Color { a: 0.30, ..accent };

        let icon = text(b.severity.glyph()).size(14).color(accent);

        let text_col: Element<'a, M> = match b.text {
            Some(sub) => column![
                text(b.title).size(13).color(C_TEXT_PRIMARY),
                text(sub).size(11).color(C_TEXT_MUTED),
            ]
            .spacing(2)
            .into(),
            None => text(b.title).size(13).color(C_TEXT_PRIMARY).into(),
        };

        let mut content_row = row![icon, text_col].spacing(12).align_y(Alignment::Center);
        content_row = content_row.push(iced::widget::Space::new().width(Length::Fill));

        let filled = b.severity.action_filled();
        if let Some((label, msg)) = b.action {
            content_row = content_row.push(action_button(label, accent, filled, msg));
        }

        if let Some(msg) = b.on_dismiss {
            content_row = content_row.push(dismiss_button(msg));
        }

        let card = container(content_row)
            .width(Length::Fill)
            .padding(Padding::default().left(14).right(14).top(10).bottom(10))
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..container::Style::default()
            });

        let stripe = container(iced::widget::Space::new())
            .width(Length::Fixed(3.0))
            .height(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 1.5.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });

        container(row![stripe, card].spacing(0).align_y(Alignment::Center))
            .width(Length::Fill)
            .into()
    }
}

fn action_button<'a, M: 'a + Clone>(
    label: Cow<'a, str>,
    accent: Color,
    filled: bool,
    msg: M,
) -> Element<'a, M> {
    let (bg_idle, bg_hover) = if filled { (0.15, 0.25) } else { (0.0, 0.10) };
    button(text(label).size(11).color(accent))
        .on_press(msg)
        .padding(Padding::default().left(12).right(12).top(4).bottom(4))
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(Background::Color(Color {
                    a: if hovered { bg_hover } else { bg_idle },
                    ..accent
                })),
                border: Border {
                    color: Color { a: 0.40, ..accent },
                    width: 1.0,
                    radius: 5.0.into(),
                },
                text_color: accent,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn dismiss_button<'a, M: 'a + Clone>(msg: M) -> Element<'a, M> {
    button(text("\u{2715}").size(11).color(C_TEXT_MUTED))
        .on_press(msg)
        .padding(Padding::default().left(4).right(4).top(2).bottom(2))
        .style(|_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(Background::Color(if hovered {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                } else {
                    Color::TRANSPARENT
                })),
                border: Border::default(),
                text_color: C_TEXT_MUTED,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_banner_constructs() {
        let _: Element<'_, ()> = banner().title("hello").into();
    }

    #[test]
    fn full_banner_constructs() {
        let _: Element<'_, ()> = banner::<()>()
            .severity(Severity::Warning)
            .title("Steam is not running")
            .text("Showing cached data from 2 hours ago.")
            .action("Retry", ())
            .on_dismiss(())
            .into();
    }
}
