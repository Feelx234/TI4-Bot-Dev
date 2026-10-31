//! The fixed words of the mark texts: a literal keyword that is one of a closed set at its place
//! (`yes`, `control`, `space`, `insider:peek`, ...). Declared in the text order of the words; the adapter's
//! table holds the texts and a test checks the order.

/// A keyword of a mark key or value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Word {
    Action = 0,
    ActionCards = 1,
    Breakthrough = 2,
    Choice = 3,
    Control = 4,
    Declined = 5,
    Destroyed = 6,
    Discarded = 7,
    Exempt = 8,
    False = 9,
    Galvanized = 10,
    Granted = 11,
    Ground = 12,
    InsiderPeek = 13,
    Mi = 14,
    Negotiating = 15,
    Open = 16,
    Probe = 17,
    PromissoryNotes = 18,
    Relics = 19,
    Replace = 20,
    SecretObjectives = 21,
    Space = 22,
    Spynet = 23,
    Standing = 24,
    Swap = 25,
    Taken = 26,
    Technologies = 27,
    True = 28,
    Used = 29,
    WinnuGained = 30,
    WinnuImperator = 31,
    WinnuTactical = 32,
    Yes = 33,
    YssarilCommander = 34,
    YssarilHero = 35,
}

impl Word {
    /// The number of words.
    pub const COUNT: usize = 36;
    /// Every word, in integer (= text) order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Action,
        Self::ActionCards,
        Self::Breakthrough,
        Self::Choice,
        Self::Control,
        Self::Declined,
        Self::Destroyed,
        Self::Discarded,
        Self::Exempt,
        Self::False,
        Self::Galvanized,
        Self::Granted,
        Self::Ground,
        Self::InsiderPeek,
        Self::Mi,
        Self::Negotiating,
        Self::Open,
        Self::Probe,
        Self::PromissoryNotes,
        Self::Relics,
        Self::Replace,
        Self::SecretObjectives,
        Self::Space,
        Self::Spynet,
        Self::Standing,
        Self::Swap,
        Self::Taken,
        Self::Technologies,
        Self::True,
        Self::Used,
        Self::WinnuGained,
        Self::WinnuImperator,
        Self::WinnuTactical,
        Self::Yes,
        Self::YssarilCommander,
        Self::YssarilHero,
    ];

    /// The integer value.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}
