use iced::widget::{column, container};
use iced::{Background, Border, Element, Length, Padding};

use crate::ui::theme::{palette, theme_from_iced};

pub struct ScreenContent<'a, M> {
    pub header: Element<'a, M>,
    pub top: Option<Element<'a, M>>,
    pub body: Element<'a, M>,
    pub footer: Option<Element<'a, M>>,
}

pub fn compose_screen<'a, M: 'a>(content: ScreenContent<'a, M>) -> Element<'a, M> {
    let header_chrome = container(content.header)
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

    let mut col = column![header_chrome].spacing(0);

    if let Some(top) = content.top {
        col = col.push(top);
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
            header: text("h").into(),
            top: None,
            body: text("b").into(),
            footer: None,
        };
        let _: Element<'_, ()> = compose_screen(content);
    }

    #[test]
    fn compose_screen_full_content() {
        let content = ScreenContent::<()> {
            header: text("h").into(),
            top: Some(text("t").into()),
            body: text("b").into(),
            footer: Some(text("f").into()),
        };
        let _: Element<'_, ()> = compose_screen(content);
    }
}
