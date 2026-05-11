use iced::Color;

const fn rgb(rgb: u32) -> Color {
    Color {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

const FALLBACK: Color = rgb(0x8a8a96);

pub fn genre_color(name: &str) -> Color {
    match name {
        "Action" => rgb(0xe87c5a),
        "Adventure" => rgb(0xe8af5a),
        "Casual" => rgb(0xaac878),
        "Early Access" => rgb(0x9682c8),
        "Free to Play" => rgb(0x6ec3c3),
        "Indie" => rgb(0xb4dc6e),
        "Massively Multiplayer" => rgb(0x5f91d2),
        "RPG" => rgb(0xcd78b4),
        "Racing" => rgb(0xdc6464),
        "Simulation" => rgb(0x78aadc),
        "Sports" => rgb(0x8cc88c),
        "Strategy" => rgb(0xbe91dc),

        "Accounting" => rgb(0xa8c89a),
        "Animation & Modeling" => rgb(0xe89682),
        "Audio Production" => rgb(0xb48cdc),
        "Design & Illustration" => rgb(0xf0c878),
        "Education" => rgb(0x82c8c8),
        "Photo Editing" => rgb(0xe8b478),
        "Software Training" => rgb(0x82a0c8),
        "Utilities" => rgb(0xb4b4b4),
        "Video Production" => rgb(0xc89578),
        "Web Publishing" => rgb(0x78dc96),
        "Game Development" => rgb(0xdc96c8),

        "Sexual Content" => rgb(0xdc6e9a),
        "Nudity" => rgb(0xbe7882),
        "Violent" => rgb(0xc85a5a),
        "Gore" => rgb(0x963c3c),

        "Documentary" => rgb(0xaab4c8),
        "Tutorial" => rgb(0xf0dc78),

        _ => FALLBACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_genre_returns_specific_color() {
        assert_eq!(genre_color("Action"), rgb(0xe87c5a));
        assert_eq!(genre_color("Strategy"), rgb(0xbe91dc));
        assert_eq!(genre_color("Gore"), rgb(0x963c3c));
    }

    #[test]
    fn unknown_genre_returns_fallback() {
        assert_eq!(genre_color("Nonexistent"), FALLBACK);
        assert_eq!(genre_color(""), FALLBACK);
    }

    #[test]
    fn rgb_decoder_matches_expected() {
        let c = rgb(0xff0000);
        assert!((c.r - 1.0).abs() < 1e-6);
        assert!(c.g.abs() < 1e-6);
        assert!(c.b.abs() < 1e-6);
        assert!((c.a - 1.0).abs() < 1e-6);
    }
}
