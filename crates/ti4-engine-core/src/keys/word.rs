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
    Granted = 10,
    Ground = 11,
    InsiderPeek = 12,
    Mi = 13,
    Negotiating = 14,
    Open = 15,
    Probe = 16,
    PromissoryNotes = 17,
    Relics = 18,
    Replace = 19,
    SecretObjectives = 20,
    Space = 21,
    Spynet = 22,
    Standing = 23,
    Swap = 24,
    Taken = 25,
    Technologies = 26,
    True = 27,
    Used = 28,
    WinnuGained = 29,
    WinnuImperator = 30,
    WinnuTactical = 31,
    Yes = 32,
    YssarilCommander = 33,
    YssarilHero = 34,
}

impl Word {
    /// The number of words.
    pub const COUNT: usize = 35;
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
