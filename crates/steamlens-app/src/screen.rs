use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::ui::theme::{palette, theme_from_iced};

pub struct AppHeaderContent<'a> {
    pub search: Option<Element<'a, crate::Message>>,
    pub screen_actions: Vec<Element<'a, crate::Message>>,
    pub second_row: Option<Element<'a, crate::Message>>,
}

pub fn render_app_header(content: AppHeaderContent<'_>) -> Element<'_, crate::Message> {
    let mut top_row = row![].spacing(12).align_y(Alignment::Center);

    {
        let title_el = container(text("SteamLens").size(22).style(|t: &iced::Theme| {
            let p = palette(theme_from_iced(t));
            iced::widget::text::Style {
                color: Some(p.accent),
            }
        }));
        top_row = top_row.push(title_el);
    }

    if let Some(search) = content.search {
        top_row = top_row.push(search);
    }
    for action in content.screen_actions {
        top_row = top_row.push(action);
    }
    top_row = top_row.push(build_global_actions());

    let inner: Element<'_, crate::Message> = match content.second_row {
        Some(second) => column![top_row, second].spacing(8).into(),
        None => top_row.into(),
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

fn build_global_actions() -> Element<'static, crate::Message> {
    use crate::theme::{C_BORDER, C_HOVER, C_TEXT_MUTED, C_TEXT_PRIMARY};

    let make_icon_btn = |glyph: &'static str, toast_msg: &'static str| {
        button(
            container(text(glyph).size(14).color(C_TEXT_MUTED))
                .width(Length::Fixed(32.0))
                .height(Length::Fixed(32.0))
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .on_press(crate::Message::GlobalToast(toast_msg.to_owned()))
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

    row![
        make_icon_btn("\u{2699}", "Settings \u{2014} coming soon"),
        make_icon_btn("\u{24D8}", "About \u{2014} coming soon"),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

pub struct ScreenContent<'a, M> {
    pub top: Option<Element<'a, M>>,
    pub body: Element<'a, M>,
    pub footer: Option<Element<'a, M>>,
}

pub fn compose_screen<'a, M: 'a>(content: ScreenContent<'a, M>) -> Element<'a, M> {
    let mut col = column![].spacing(0);

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
            top: None,
            body: text("b").into(),
            footer: None,
        };
        let _: Element<'_, ()> = compose_screen(content);
    }

    #[test]
    fn compose_screen_full_content() {
        let content = ScreenContent::<()> {
            top: Some(text("t").into()),
            body: text("b").into(),
            footer: Some(text("f").into()),
        };
        let _: Element<'_, ()> = compose_screen(content);
    }
}
