use iced::widget::{container, row};
use iced::{Background, Border, Element, Length, Padding};

use crate::ui::theme::{palette, theme_from_iced};

const WIDGET_ROW_HEIGHT: f32 = 325.0;

pub fn widget_panel<'a, M: 'a>(left: Element<'a, M>, right: Element<'a, M>) -> Element<'a, M> {
    let two_col_row = row![
        container(left)
            .width(Length::FillPortion(5))
            .height(Length::Fixed(WIDGET_ROW_HEIGHT))
            .padding(18)
            .style(|t: &iced::Theme| container::Style {
                background: Some(Background::Color(palette(theme_from_iced(t)).surface)),
                border: Border {
                    radius: 10.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
        container(right)
            .width(Length::FillPortion(3))
            .height(Length::Fixed(WIDGET_ROW_HEIGHT))
            .padding(16)
            .style(|t: &iced::Theme| container::Style {
                background: Some(Background::Color(palette(theme_from_iced(t)).surface)),
                border: Border {
                    radius: 10.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    ]
    .spacing(16);

    container(two_col_row)
        .width(Length::Fill)
        .padding(Padding::default().left(16).right(16).top(12).bottom(12))
        .into()
}
