use iced::widget::{button, container};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

const INACTIVE_BG_ALPHA: f32 = 0.10;
const INACTIVE_BORDER_ALPHA: f32 = 0.20;

pub fn pill<'a, M: 'a>(content: impl Into<Element<'a, M>>, tint: Color) -> Pill<'a, M> {
    Pill {
        content: content.into(),
        tint,
        radius: 12.0,
        pad_h: 8,
        pad_v: 3,
        bg_alpha: 0.15,
        border_alpha: 0.40,
        shadow: None,
        dot: None,
        on_press: None,
        selected: None,
    }
}

pub struct Pill<'a, M> {
    content: Element<'a, M>,
    tint: Color,
    radius: f32,
    pad_h: u32,
    pad_v: u32,
    bg_alpha: f32,
    border_alpha: f32,
    shadow: Option<Shadow>,
    dot: Option<Color>,
    on_press: Option<M>,
    selected: Option<bool>,
}

impl<'a, M: 'a> Pill<'a, M> {
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }

    pub fn padding(mut self, horizontal: u32, vertical: u32) -> Self {
        self.pad_h = horizontal;
        self.pad_v = vertical;
        self
    }

    pub fn bg_alpha(mut self, a: f32) -> Self {
        self.bg_alpha = a;
        self
    }

    pub fn border_alpha(mut self, a: f32) -> Self {
        self.border_alpha = a;
        self
    }

    pub fn glow(mut self, color: Color) -> Self {
        self.shadow = Some(Shadow {
            color,
            offset: Vector::new(0.0, 0.0),
            blur_radius: 10.0,
        });
        self
    }

    pub fn with_dot(mut self, dot_color: Color) -> Self {
        self.dot = Some(dot_color);
        self
    }

    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn selected(mut self, is_selected: bool) -> Self {
        self.selected = Some(is_selected);
        self
    }
}

impl<'a, M: 'a + Clone> From<Pill<'a, M>> for Element<'a, M> {
    fn from(p: Pill<'a, M>) -> Self {
        let tint = p.tint;
        let (bg_alpha, border_alpha) = match p.selected {
            Some(false) => (INACTIVE_BG_ALPHA, INACTIVE_BORDER_ALPHA),
            _ => (p.bg_alpha, p.border_alpha),
        };
        let bg = Color {
            a: bg_alpha,
            ..tint
        };
        let border_color = Color {
            a: border_alpha,
            ..tint
        };
        let radius = p.radius;
        let shadow = p.shadow.unwrap_or_default();
        let padding = Padding::default()
            .left(p.pad_h)
            .right(p.pad_h)
            .top(p.pad_v)
            .bottom(p.pad_v);

        let inner: Element<'a, M> = if let Some(dot_color) = p.dot {
            let dot_widget = container(iced::widget::Space::new())
                .width(Length::Fixed(6.0))
                .height(Length::Fixed(6.0))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: Border {
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                });
            iced::widget::row![dot_widget, p.content]
                .spacing(5)
                .align_y(Alignment::Center)
                .into()
        } else {
            p.content
        };

        if let Some(msg) = p.on_press {
            button(inner)
                .on_press(msg)
                .padding(padding)
                .style(move |_: &iced::Theme, _status| button::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: radius.into(),
                    },
                    shadow,
                    text_color: tint,
                    ..button::Style::default()
                })
                .into()
        } else {
            container(inner)
                .padding(padding)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: radius.into(),
                    },
                    shadow,
                    ..container::Style::default()
                })
                .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Color;

    #[test]
    fn plain_text_pill_constructs() {
        let _: Element<'_, ()> =
            pill(iced::widget::text("hello"), Color::from_rgb(0.5, 0.5, 1.0)).into();
    }

    #[test]
    fn pill_with_all_options_constructs() {
        let _: Element<'_, ()> = pill(iced::widget::text("x"), Color::from_rgb(1.0, 0.5, 0.5))
            .radius(12.0)
            .padding(10, 3)
            .bg_alpha(0.18)
            .border_alpha(0.5)
            .glow(Color::from_rgba(1.0, 0.5, 0.5, 0.5))
            .into();
    }

    #[test]
    fn pill_with_dot_constructs() {
        let _: Element<'_, ()> = pill(iced::widget::text("rare"), Color::from_rgb(0.8, 0.5, 1.0))
            .with_dot(Color::from_rgb(0.8, 0.5, 1.0))
            .into();
    }
}
