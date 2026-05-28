#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementFilter {
    All,
    Unlocked,
    Locked,
}

impl AchievementFilter {
    pub fn label(self) -> &'static str {
        match self {
            AchievementFilter::All => "All",
            AchievementFilter::Unlocked => "Unlocked",
            AchievementFilter::Locked => "Locked",
        }
    }
}

impl std::fmt::Display for AchievementFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementSort {
    UnlockChance,
    RarityAndName,
    Name,
}

impl AchievementSort {
    pub fn label(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "Unlock Chance",
            AchievementSort::RarityAndName => "Rarity & Name",
            AchievementSort::Name => "Name",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "Unlock chance",
            AchievementSort::RarityAndName => "Rarity & name",
            AchievementSort::Name => "A \u{2014} Z",
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            AchievementSort::UnlockChance => "Sort by unlock chance (rarest first)",
            AchievementSort::RarityAndName => "Sort by rarity tier, then name",
            AchievementSort::Name => "Sort by name (A to Z)",
        }
    }

    pub const ALL: &'static [AchievementSort] = &[
        AchievementSort::Name,
        AchievementSort::UnlockChance,
        AchievementSort::RarityAndName,
    ];
}

impl std::fmt::Display for AchievementSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
