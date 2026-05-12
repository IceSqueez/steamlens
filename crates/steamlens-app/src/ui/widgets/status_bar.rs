use std::borrow::Cow;
use std::marker::PhantomData;
use std::time::Instant;

use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::theme::{C_ACCENT, C_BORDER, C_SURFACE, C_TEXT_MUTED};

const CONNECTED_DOT: Color = Color::from_rgb(0.427, 0.788, 0.498);
const OFFLINE_DOT: Color = Color::from_rgb(0.941, 0.784, 0.478);
const TRACK_BG: Color = Color::from_rgba(0.16, 0.14, 0.22, 1.0);

pub fn status_bar<'a, M: 'a + Clone>() -> StatusBar<'a, M> {
    StatusBar::default()
}

enum Mode<'a> {
    Empty,
    Connected {
        count: usize,
        noun: Cow<'a, str>,
        last_sync: Option<Instant>,
    },
    Scanning {
        label: Cow<'a, str>,
        current: usize,
        total: usize,
    },
    Offline {
        cached_games: usize,
    },
}

pub struct StatusBar<'a, M> {
    mode: Mode<'a>,
    reconnect: Option<M>,
    retry: Option<(Cow<'a, str>, M)>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, M> Default for StatusBar<'a, M> {
    fn default() -> Self {
        Self {
            mode: Mode::Empty,
            reconnect: None,
            retry: None,
            _phantom: PhantomData,
        }
    }
}

impl<'a, M: 'a + Clone> StatusBar<'a, M> {
    pub fn connected(
        mut self,
        count: usize,
        noun: impl Into<Cow<'a, str>>,
        last_sync: Option<Instant>,
    ) -> Self {
        self.mode = Mode::Connected {
            count,
            noun: noun.into(),
            last_sync,
        };
        self
    }

    pub fn scanning(
        mut self,
        label: impl Into<Cow<'a, str>>,
        current: usize,
        total: usize,
    ) -> Self {
        self.mode = Mode::Scanning {
            label: label.into(),
            current,
            total,
        };
        self
    }

    pub fn offline(mut self, cached_games: usize) -> Self {
        self.mode = Mode::Offline { cached_games };
        self
    }

    pub fn on_reconnect(mut self, msg: M) -> Self {
        self.reconnect = Some(msg);
        self
    }

    pub fn retry(mut self, label: impl Into<Cow<'a, str>>, msg: M) -> Self {
        self.retry = Some((label.into(), msg));
        self
    }
}

impl<'a, M: 'a + Clone> From<StatusBar<'a, M>> for Element<'a, M> {
    fn from(b: StatusBar<'a, M>) -> Self {
        let mut left = row![].spacing(14).align_y(Alignment::Center);

        match b.mode {
            Mode::Empty => {}
            Mode::Connected {
                count,
                noun,
                last_sync,
            } => {
                left = left.push(cluster(CONNECTED_DOT, "Connected", C_TEXT_MUTED));
                left = left.push(text(format!("{count} {noun}")).size(11).color(C_TEXT_MUTED));
                if let Some(t) = last_sync {
                    let secs = t.elapsed().as_secs();
                    let sync_label = if secs < 60 {
                        "Last sync just now".to_owned()
                    } else {
                        format!("Last sync {}m ago", secs / 60)
                    };
                    left = left.push(text("\u{00B7}").size(11).color(C_TEXT_MUTED));
                    left = left.push(text(sync_label).size(11).color(C_TEXT_MUTED));
                }
            }
            Mode::Scanning {
                label,
                current,
                total,
            } => {
                left = left.push(cluster(C_ACCENT, label, C_ACCENT));
                if total > 0 {
                    let ratio = (current as f32 / total as f32).clamp(0.0, 1.0);
                    left = left.push(progress_bar(ratio));
                    left = left.push(
                        text(format!("{current} / {total}"))
                            .size(11)
                            .color(C_TEXT_MUTED),
                    );
                }
            }
            Mode::Offline { cached_games } => {
                left = left.push(cluster(OFFLINE_DOT, "Offline", OFFLINE_DOT));
                left = left.push(
                    text(format!("Cached: {cached_games} games"))
                        .size(11)
                        .color(C_TEXT_MUTED),
                );
            }
        }

        let mut footer_row = row![left.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(Length::Fill);

        if let Some((label, msg)) = b.retry {
            footer_row = footer_row.push(link_button(label, C_OFFLINE_RETRY, msg));
        } else if let Some(msg) = b.reconnect {
            footer_row = footer_row.push(link_button("Reconnect", C_ACCENT, msg));
        }

        container(footer_row)
            .width(Length::Fill)
            .padding(Padding::default().left(14).right(14).top(8).bottom(8))
            .style(|_: &iced::Theme| container::Style {
                background: Some(Background::Color(C_SURFACE)),
                border: Border {
                    color: C_BORDER,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            })
            .into()
    }
}

const C_OFFLINE_RETRY: Color = Color::from_rgb(0.941, 0.784, 0.478);

fn cluster<'a, M: 'a>(
    dot_color: Color,
    label: impl Into<Cow<'a, str>>,
    label_color: Color,
) -> Element<'a, M> {
    let label: Cow<'a, str> = label.into();
    let dot = container(iced::widget::Space::new())
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
    row![dot, text(label).size(11).color(label_color)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn progress_bar<'a, M: 'a>(ratio: f32) -> Element<'a, M> {
    let portion_fill = ((ratio * 1000.0).round() as u16).clamp(1, 1000);
    let portion_rest = 1000 - portion_fill;

    let fill = container(iced::widget::Space::new())
        .width(Length::FillPortion(portion_fill))
        .height(Length::Fixed(3.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(Background::Color(C_ACCENT)),
            border: Border {
                radius: 1.5.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let rest = container(iced::widget::Space::new())
        .width(Length::FillPortion(portion_rest))
        .height(Length::Fixed(3.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            ..container::Style::default()
        });

    container(
        container(row![fill, rest].width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fixed(3.0))
            .style(|_: &iced::Theme| container::Style {
                background: Some(Background::Color(TRACK_BG)),
                border: Border {
                    radius: 1.5.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .width(Length::Fixed(200.0))
    .into()
}

fn link_button<'a, M: 'a + Clone>(
    label: impl Into<Cow<'a, str>>,
    color: Color,
    msg: M,
) -> Element<'a, M> {
    let label: Cow<'a, str> = label.into();
    button(text(label).size(11).color(color))
        .on_press(msg)
        .padding(0)
        .style(move |_: &iced::Theme, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border::default(),
                text_color: if hovered {
                    Color { a: 0.85, ..color }
                } else {
                    color
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_builds() {
        let _: Element<'_, ()> = status_bar::<()>().connected(344, "games", None).into();
    }

    #[test]
    fn scanning_builds() {
        let _: Element<'_, ()> = status_bar::<()>()
            .scanning("Scanning library", 554, 626)
            .into();
    }

    #[test]
    fn offline_builds() {
        let _: Element<'_, ()> = status_bar().offline(344).on_reconnect(()).into();
    }
}
