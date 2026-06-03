use std::borrow::Cow;

use iced::widget::{container, tooltip};
use iced::{Background, Border, Element, Padding};

use crate::ui::theme::{palette, theme_from_iced};

pub fn tooltip_box<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    label: impl Into<Cow<'static, str>>,
    position: tooltip::Position,
) -> Element<'a, M> {
    let label: Cow<'static, str> = label.into();
    let tip = container(
        iced::widget::text(label)
            .size(11)
            .style(move |t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).text_primary),
            }),
    )
    .padding(
        Padding::default()
            .left(8u32)
            .right(8u32)
            .top(4u32)
            .bottom(4u32),
    )
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
    });

    tooltip(content, tip, position).into()
}
