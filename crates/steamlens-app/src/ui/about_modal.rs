use std::sync::LazyLock;

use iced::gradient::Linear;
use iced::widget::{Space, button, column, container, image as img_widget, row, stack, svg, text};
use iced::{
    Alignment, Background, Border, Color, Element, Gradient, Length, Padding, Shadow, Vector,
};

const MODAL_WIDTH: f32 = 480.0;
const ICON_SIZE: f32 = 100.0;
const BTN_ICON_SIZE: f32 = 13.0;

const C_BG: Color = Color::from_rgb(
    0x1a as f32 / 255.0,
    0x18 as f32 / 255.0,
    0x25 as f32 / 255.0,
);
const C_PANEL_BG: Color = Color::from_rgb(
    0x22 as f32 / 255.0,
    0x1f as f32 / 255.0,
    0x30 as f32 / 255.0,
);
const C_BORDER: Color = Color::from_rgb(
    0x2d as f32 / 255.0,
    0x29 as f32 / 255.0,
    0x40 as f32 / 255.0,
);
const C_TEXT_PRIMARY: Color = Color::from_rgb(
    0xf0 as f32 / 255.0,
    0xee as f32 / 255.0,
    0xf8 as f32 / 255.0,
);
const C_ACCENT: Color = Color::from_rgb(
    0xc9 as f32 / 255.0,
    0xa6 as f32 / 255.0,
    0xf0 as f32 / 255.0,
);
const C_MUTED: Color = Color::from_rgb(
    0x8a as f32 / 255.0,
    0x86 as f32 / 255.0,
    0xa3 as f32 / 255.0,
);
const C_DIM: Color = Color::from_rgb(
    0x6b as f32 / 255.0,
    0x68 as f32 / 255.0,
    0x84 as f32 / 255.0,
);
const C_BUILT_WITH: Color = Color::from_rgb(
    0xaa as f32 / 255.0,
    0xa6 as f32 / 255.0,
    0xc0 as f32 / 255.0,
);

static ABOUT_ICON: LazyLock<iced::widget::image::Handle> = LazyLock::new(|| {
    const BYTES: &[u8] = include_bytes!("../../../../assets/icon-256.png");
    iced::widget::image::Handle::from_bytes(BYTES.to_vec())
});

static SVG_GITHUB: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#c9a6f0"><path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57v-2.01c-3.345.72-4.05-1.605-4.05-1.605-.54-1.395-1.32-1.755-1.32-1.755-1.08-.735.09-.72.09-.72 1.2.075 1.83 1.23 1.83 1.23 1.065 1.83 2.805 1.305 3.495.99.105-.78.42-1.305.75-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.32.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.33 3.3 1.23A11.4 11.4 0 0 1 12 5.805c1.02.0045 2.04.135 3 .405 2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.91.435.375.81 1.095.81 2.22v3.285c0 .315.225.69.825.57C20.565 21.795 24 17.31 24 12c0-6.63-5.37-12-12-12z"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

static SVG_ISSUE: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#c9a6f0" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 8v4"/><path d="M12 16h.01"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

static SVG_RELEASES: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#c9a6f0" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 16 12 12 8 16"/><line x1="12" x2="12" y1="12" y2="21"/><path d="M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>"##
            .as_bytes()
            .to_vec(),
    )
});

pub fn about_modal<M: 'static + Clone>(
    dismiss: M,
    open_github: M,
    open_issues: M,
    open_releases: M,
) -> Element<'static, M> {
    let modal = build_card(open_github, open_issues, open_releases);

    let centered = container(modal)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    let backdrop = button(
        container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(dismiss)
    .padding(0)
    .style(|_t: &iced::Theme, _status| button::Style {
        background: Some(Background::Color(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.6,
        })),
        border: Border::default(),
        ..button::Style::default()
    });

    stack![backdrop, centered].into()
}

fn build_card<M: 'static + Clone>(
    open_github: M,
    open_issues: M,
    open_releases: M,
) -> Element<'static, M> {
    let hero = build_hero();
    let divider = container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(|_: &iced::Theme| container::Style {
            background: Some(Background::Color(C_BORDER)),
            ..container::Style::default()
        });
    let actions = build_actions(open_github, open_issues, open_releases);
    let built_with = build_built_with();
    let footer = build_footer();

    let body = column![
        hero,
        container(divider).padding(Padding::default().left(32).right(32)),
        container(actions).padding(Padding::default().left(32).right(32).top(18).bottom(20)),
        container(built_with).padding(Padding::default().left(32).right(32).bottom(14)),
        container(footer).padding(Padding::default().left(32).right(32).bottom(22)),
    ]
    .spacing(0);

    container(body)
        .width(Length::Fixed(MODAL_WIDTH))
        .style(|_: &iced::Theme| container::Style {
            background: Some(Background::Color(C_BG)),
            border: Border {
                color: C_BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..container::Style::default()
        })
        .clip(true)
        .into()
}

fn build_hero<M: 'static>() -> Element<'static, M> {
    let icon = img_widget(ABOUT_ICON.clone())
        .width(Length::Fixed(ICON_SIZE))
        .height(Length::Fixed(ICON_SIZE));

    let icon_with_glow = container(icon)
        .width(Length::Fixed(ICON_SIZE))
        .height(Length::Fixed(ICON_SIZE))
        .style(|_: &iced::Theme| container::Style {
            border: Border {
                radius: 22.0.into(),
                ..Border::default()
            },
            shadow: Shadow {
                color: Color {
                    a: 0.55,
                    ..C_ACCENT
                },
                offset: Vector::new(0.0, 0.0),
                blur_radius: 36.0,
            },
            ..container::Style::default()
        });

    let name = text("SteamLens").size(24).color(C_TEXT_PRIMARY);
    let version = text(concat!("Version ", env!("CARGO_PKG_VERSION")))
        .size(13)
        .color(C_ACCENT);
    let tagline = text("A Steam achievement manager with rarity insights and library statistics.")
        .size(12)
        .color(C_MUTED);

    let content = column![
        container(icon_with_glow)
            .align_x(Alignment::Center)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(18.0)),
        container(name)
            .align_x(Alignment::Center)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(4.0)),
        container(version)
            .align_x(Alignment::Center)
            .width(Length::Fill),
        Space::new().height(Length::Fixed(8.0)),
        container(tagline)
            .align_x(Alignment::Center)
            .width(Length::Fill),
    ]
    .spacing(0)
    .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding(Padding::default().left(32).right(32).top(36).bottom(24))
        .style(|_: &iced::Theme| {
            let gradient = Linear::new(0.0)
                .add_stop(0.0, Color::TRANSPARENT)
                .add_stop(
                    0.55,
                    Color {
                        a: 0.04,
                        ..C_ACCENT
                    },
                )
                .add_stop(
                    1.0,
                    Color {
                        a: 0.18,
                        ..C_ACCENT
                    },
                );
            container::Style {
                background: Some(Background::Gradient(Gradient::Linear(gradient))),
                ..container::Style::default()
            }
        })
        .into()
}

fn build_actions<M: 'static + Clone>(
    open_github: M,
    open_issues: M,
    open_releases: M,
) -> Element<'static, M> {
    let action_btn =
        |icon_handle: svg::Handle, label: &'static str, msg: M| -> Element<'static, M> {
            let svg_icon = svg(icon_handle)
                .width(Length::Fixed(BTN_ICON_SIZE))
                .height(Length::Fixed(BTN_ICON_SIZE));

            let body = row![svg_icon, text(label).size(12).color(C_ACCENT),]
                .spacing(6)
                .align_y(Alignment::Center);

            button(body)
                .on_press(msg)
                .padding(Padding::default().left(14).right(14).top(7).bottom(7))
                .style(|_t: &iced::Theme, status| {
                    let hovered =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    button::Style {
                        background: Some(Background::Color(Color {
                            a: if hovered { 0.18 } else { 0.10 },
                            ..C_ACCENT
                        })),
                        border: Border {
                            color: Color {
                                a: if hovered { 0.45 } else { 0.30 },
                                ..C_ACCENT
                            },
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        text_color: C_ACCENT,
                        ..button::Style::default()
                    }
                })
                .into()
        };

    let buttons = row![
        action_btn(SVG_GITHUB.clone(), "GitHub", open_github),
        action_btn(SVG_ISSUE.clone(), "Report issue", open_issues),
        action_btn(SVG_RELEASES.clone(), "Releases", open_releases),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(buttons)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
}

fn build_built_with<M: 'static>() -> Element<'static, M> {
    let label = text("BUILT WITH").size(10).color(C_MUTED);
    let stack_line = text("Rust  ·  iced  ·  tokio").size(11).color(C_BUILT_WITH);

    let col = column![label, Space::new().height(Length::Fixed(8.0)), stack_line,];

    container(col)
        .width(Length::Fill)
        .padding(Padding::default().left(14).right(14).top(12).bottom(12))
        .style(|_: &iced::Theme| container::Style {
            background: Some(Background::Color(C_PANEL_BG)),
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

fn build_footer<M: 'static>() -> Element<'static, M> {
    let left = text("\u{00A9} 2026 IceSqueez").size(10).color(C_DIM);
    let right = text("MIT License").size(10).color(C_DIM);

    row![left, Space::new().width(Length::Fill), right]
        .align_y(Alignment::Center)
        .into()
}
