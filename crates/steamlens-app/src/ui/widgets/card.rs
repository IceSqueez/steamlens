use iced::widget::{button, container};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::ui::theme::{palette, theme_from_iced};

const DEFAULT_RADIUS: f32 = 8.0;

pub fn card<'a, M: Clone + 'a>(content: impl Into<Element<'a, M>>) -> Card<'a, M> {
    Card {
        content: content.into(),
        on_press: None,
        accent: None,
        gold: false,
        border_accent: false,
        radius: DEFAULT_RADIUS,
        forced_hover: None,
        width: None,
        height: None,
        padding: None,
        accent_border_width_default: 2.0,
        accent_border_width_hover: 2.0,
        accent_alpha_default: 0.50,
        accent_alpha_hover: 1.00,
    }
}

pub struct Card<'a, M> {
    content: Element<'a, M>,
    on_press: Option<M>,
    accent: Option<Color>,
    gold: bool,
    border_accent: bool,
    radius: f32,
    forced_hover: Option<bool>,
    width: Option<Length>,
    height: Option<Length>,
    padding: Option<Padding>,
    accent_border_width_default: f32,
    accent_border_width_hover: f32,
    accent_alpha_default: f32,
    accent_alpha_hover: f32,
}

impl<'a, M: Clone + 'a> Card<'a, M> {
    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn accent(mut self, color: Color) -> Self {
        self.accent = Some(color);
        self
    }

    pub fn gold_when(mut self, is_gold: bool) -> Self {
        self.gold = is_gold;
        self
    }

    pub fn border_accent_when(mut self, use_border: bool) -> Self {
        self.border_accent = use_border;
        self
    }

    pub fn accent_maybe(mut self, color: Option<Color>) -> Self {
        self.accent = color;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = Some(padding.into());
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.forced_hover = Some(hovered);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn accent_border_width(mut self, default: f32, hover: f32) -> Self {
        self.accent_border_width_default = default;
        self.accent_border_width_hover = hover;
        self
    }

    pub fn accent_alpha(mut self, default: f32, hover: f32) -> Self {
        self.accent_alpha_default = default;
        self.accent_alpha_hover = hover;
        self
    }
}

impl<'a, M: Clone + 'a> From<Card<'a, M>> for Element<'a, M> {
    fn from(card: Card<'a, M>) -> Self {
        let fixed_accent = card.accent;
        let gold = card.gold;
        let border_accent = card.border_accent;
        let radius = card.radius;
        let forced_hover = card.forced_hover;
        let acc_w_default = card.accent_border_width_default;
        let acc_w_hover = card.accent_border_width_hover;
        let acc_a_default = card.accent_alpha_default;
        let acc_a_hover = card.accent_alpha_hover;

        let mut wrapped = container(card.content);
        if let Some(width) = card.width {
            wrapped = wrapped.width(width);
        }
        if let Some(height) = card.height {
            wrapped = wrapped.height(height);
        }
        if let Some(padding) = card.padding {
            wrapped = wrapped.padding(padding);
        }

        let mut button = button(wrapped)
            .padding(0)
            .style(move |t: &iced::Theme, status| {
                let p = palette(theme_from_iced(t));
                let surface = p.surface;
                let neutral_hover_border = p.accent;
                let accent = if gold {
                    Some(p.rarity_legendary)
                } else if border_accent {
                    Some(p.border)
                } else {
                    fixed_accent
                };

                let hovered = forced_hover.unwrap_or(matches!(
                    status,
                    button::Status::Hovered | button::Status::Pressed
                ));

                let bg = if hovered { p.hover } else { surface };

                let (border, shadow) = match (accent, hovered) {
                    (Some(color), true) => (
                        Border {
                            color: Color {
                                a: acc_a_hover,
                                ..color
                            },
                            width: acc_w_hover,
                            radius: radius.into(),
                        },
                        Shadow {
                            color: Color { a: 0.50, ..color },
                            offset: Vector::new(0.0, 0.0),
                            blur_radius: 16.0,
                        },
                    ),
                    (Some(color), false) => (
                        Border {
                            color: Color {
                                a: acc_a_default,
                                ..color
                            },
                            width: acc_w_default,
                            radius: radius.into(),
                        },
                        Shadow {
                            color: Color { a: 0.30, ..color },
                            offset: Vector::new(0.0, 0.0),
                            blur_radius: 10.0,
                        },
                    ),
                    (None, true) => (
                        Border {
                            color: neutral_hover_border,
                            width: 2.0,
                            radius: radius.into(),
                        },
                        Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.60),
                            offset: Vector::new(0.0, 8.0),
                            blur_radius: 18.0,
                        },
                    ),
                    (None, false) => (
                        Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: radius.into(),
                        },
                        Shadow {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
                            offset: Vector::new(0.0, 4.0),
                            blur_radius: 10.0,
                        },
                    ),
                };

                button::Style {
                    background: Some(Background::Color(bg)),
                    border,
                    shadow,
                    ..button::Style::default()
                }
            });

        if let Some(message) = card.on_press {
            button = button.on_press(message);
        }

        button.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text;

    #[test]
    fn card_default_constructs_without_panic() {
        let _: Element<'_, ()> = card(text("hi")).into();
    }

    #[test]
    fn card_with_all_setters_constructs() {
        let _: Element<'_, ()> = card(text("body"))
            .accent(Color::from_rgb(1.0, 0.85, 0.4))
            .radius(6.0)
            .hovered(true)
            .width(Length::Fixed(200.0))
            .height(Length::Fixed(100.0))
            .into();
    }

    #[test]
    fn card_with_on_press_constructs() {
        #[derive(Clone)]
        struct Msg;
        let _: Element<'_, Msg> = card(text("body")).on_press(Msg).into();
    }
}
