use std::borrow::Cow;

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::ui::theme::{palette, theme_from_iced};

pub fn closest_row<'a, M: 'a + Clone>(
    image: Element<'a, M>,
    primary: impl Into<Cow<'a, str>>,
    secondary: impl Into<Cow<'a, str>>,
    pct_label: impl Into<Cow<'a, str>>,
    on_press: M,
) -> Element<'a, M> {
    let primary_label = text(primary.into())
        .size(12)
        .style(|t: &iced::Theme| iced::widget::text::Style {
            color: Some(palette(theme_from_iced(t)).text_primary),
        })
        .wrapping(text::Wrapping::None);
    let secondary_label =
        text(secondary.into())
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_muted),
            });

    let info_col = column![
        container(primary_label).width(Length::Fill).clip(true),
        secondary_label,
    ]
    .spacing(1)
    .width(Length::Fill);

    let pct_chip =
        text(pct_label.into())
            .size(13)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).accent),
            });

    let row_content = row![image, info_col, pct_chip]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(Padding::default().left(6).right(6).top(5).bottom(5));

    button(row_content)
        .on_press(on_press)
        .padding(0)
        .style(|t: &iced::Theme, status| {
            let p = palette(theme_from_iced(t));
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if hovered {
                    p.hover
                } else {
                    p.control_surface
                })),
                border: Border {
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .into()
}
