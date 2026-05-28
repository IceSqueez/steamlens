use std::borrow::Cow;
use std::marker::PhantomData;
use std::time::Instant;

use iced::widget::{button, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use crate::ui::theme::{palette, theme_from_iced};

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
        cached_count: usize,
        noun: Cow<'a, str>,
        failed_count: usize,
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

    pub fn offline(mut self, cached_count: usize, noun: impl Into<Cow<'a, str>>) -> Self {
        self.mode = Mode::Offline {
            cached_count,
            noun: noun.into(),
            failed_count: 0,
        };
        self
    }

    pub fn failed(mut self, failed: usize) -> Self {
        if let Mode::Offline { failed_count, .. } = &mut self.mode {
            *failed_count = failed;
        }
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

pub struct StatusContext<'a> {
    pub total: usize,
    pub noun: &'a str,
    pub steam_running: Option<bool>,
    pub failed: usize,
    pub offline_cached_count: usize,
    pub last_sync: Option<Instant>,
}

pub fn derive_status_bar<'a, M: 'a + Clone>(
    ctx: StatusContext<'a>,
    scanning_phases: &[(&'a str, usize)],
) -> Option<Element<'a, M>> {
    if ctx.steam_running == Some(false) {
        if ctx.total == 0 {
            return None;
        }
        return Some(
            status_bar::<M>()
                .offline(ctx.offline_cached_count, ctx.noun)
                .failed(ctx.failed)
                .into(),
        );
    }
    if ctx.total == 0 {
        return None;
    }
    for (label, current) in scanning_phases {
        if *current < ctx.total {
            return Some(
                status_bar::<M>()
                    .scanning(*label, *current, ctx.total)
                    .into(),
            );
        }
    }
    Some(
        status_bar::<M>()
            .connected(ctx.total, ctx.noun, ctx.last_sync)
            .into(),
    )
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
                left = left.push(cluster_themed(DotKind::Connected, "Connected", false));
                left = left.push(text(format!("{count} {noun}")).size(11).style(
                    |t: &iced::Theme| iced::widget::text::Style {
                        color: Some(palette(theme_from_iced(t)).text_muted),
                    },
                ));
                if let Some(t) = last_sync {
                    let secs = t.elapsed().as_secs();
                    let sync_label = if secs < 60 {
                        "Last sync just now".to_owned()
                    } else {
                        format!("Last sync {}m ago", secs / 60)
                    };
                    left = left.push(text("\u{00B7}").size(11).style(|t: &iced::Theme| {
                        iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).text_muted),
                        }
                    }));
                    left = left.push(text(sync_label).size(11).style(|t: &iced::Theme| {
                        iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).text_muted),
                        }
                    }));
                }
            }
            Mode::Scanning {
                label,
                current,
                total,
            } => {
                left = left.push(cluster_themed(DotKind::Scanning, label, true));
                if total > 0 {
                    let ratio = (current as f32 / total as f32).clamp(0.0, 1.0);
                    left = left.push(progress_bar(ratio));
                    left = left.push(text(format!("{current} / {total}")).size(11).style(
                        |t: &iced::Theme| iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).text_muted),
                        },
                    ));
                }
            }
            Mode::Offline {
                cached_count,
                noun,
                failed_count,
            } => {
                left = left.push(cluster_themed(DotKind::Offline, "Offline", true));
                left = left.push(
                    text(format!("Cached: {cached_count} {noun}"))
                        .size(11)
                        .style(|t: &iced::Theme| iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).text_muted),
                        }),
                );
                if failed_count > 0 {
                    left = left.push(text("\u{00B7}").size(11).style(|t: &iced::Theme| {
                        iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).text_muted),
                        }
                    }));
                    left = left.push(text(format!("Failed: {failed_count}")).size(11).style(
                        |t: &iced::Theme| iced::widget::text::Style {
                            color: Some(palette(theme_from_iced(t)).dot_offline),
                        },
                    ));
                }
            }
        }

        let mut footer_row = row![left.width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center)
            .width(Length::Fill);

        if let Some((label, msg)) = b.retry {
            footer_row = footer_row.push(link_button(label, DotKind::Offline, msg));
        } else if let Some(msg) = b.reconnect {
            footer_row = footer_row.push(link_button_accent(msg));
        }

        let inner = container(footer_row)
            .width(Length::Fill)
            .padding(Padding::default().left(14).right(14).top(8).bottom(8))
            .style(|t: &iced::Theme| container::Style {
                background: Some(Background::Color(palette(theme_from_iced(t)).surface)),
                border: Border {
                    radius: 10.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });

        container(inner)
            .width(Length::Fill)
            .padding(Padding::default().left(16).right(16).top(0).bottom(12))
            .into()
    }
}

#[derive(Clone, Copy)]
enum DotKind {
    Connected,
    Offline,
    Scanning,
}

fn dot_color_for(kind: DotKind, p: &crate::ui::theme::ThemePalette) -> Color {
    match kind {
        DotKind::Connected => p.dot_connected,
        DotKind::Offline => p.dot_offline,
        DotKind::Scanning => p.accent,
    }
}

fn cluster_themed<'a, M: 'a>(
    kind: DotKind,
    label: impl Into<Cow<'a, str>>,
    label_matches_dot: bool,
) -> Element<'a, M> {
    let label: Cow<'a, str> = label.into();
    let dot = container(iced::widget::Space::new())
        .width(Length::Fixed(6.0))
        .height(Length::Fixed(6.0))
        .style(move |t: &iced::Theme| container::Style {
            background: Some(Background::Color(dot_color_for(
                kind,
                palette(theme_from_iced(t)),
            ))),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let label_text = text(label).size(11).style(move |t: &iced::Theme| {
        let p = palette(theme_from_iced(t));
        let color = if label_matches_dot {
            dot_color_for(kind, p)
        } else {
            p.text_muted
        };
        iced::widget::text::Style { color: Some(color) }
    });
    row![dot, label_text]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

fn link_button_accent<'a, M: 'a + Clone>(msg: M) -> Element<'a, M> {
    button(
        text("Reconnect")
            .size(11)
            .style(|t: &iced::Theme| iced::widget::text::Style {
                color: Some(palette(theme_from_iced(t)).accent),
            }),
    )
    .on_press(msg)
    .padding(0)
    .style(|t: &iced::Theme, status| {
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        let color = palette(theme_from_iced(t)).accent;
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

fn progress_bar<'a, M: 'a>(ratio: f32) -> Element<'a, M> {
    let portion_fill = ((ratio * 1000.0).round() as u16).clamp(1, 1000);
    let portion_rest = 1000 - portion_fill;

    let fill = container(iced::widget::Space::new())
        .width(Length::FillPortion(portion_fill))
        .height(Length::Fixed(3.0))
        .style(|t: &iced::Theme| container::Style {
            background: Some(Background::Color(palette(theme_from_iced(t)).accent)),
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
            .style(|t: &iced::Theme| container::Style {
                background: Some(Background::Color(palette(theme_from_iced(t)).border)),
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
    kind: DotKind,
    msg: M,
) -> Element<'a, M> {
    let label: Cow<'a, str> = label.into();
    button(
        text(label)
            .size(11)
            .style(move |t: &iced::Theme| iced::widget::text::Style {
                color: Some(dot_color_for(kind, palette(theme_from_iced(t)))),
            }),
    )
    .on_press(msg)
    .padding(0)
    .style(move |t: &iced::Theme, status| {
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        let base = dot_color_for(kind, palette(theme_from_iced(t)));
        iced::widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            border: Border::default(),
            text_color: if hovered {
                Color { a: 0.85, ..base }
            } else {
                base
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
        let _: Element<'_, ()> = status_bar().offline(344, "games").on_reconnect(()).into();
    }
}
