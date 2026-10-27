use crate::ids::Seat;

/// Tokens, goods, points and per-player flags (`Player.tactic_tokens` .. `victory_points`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerEcon {
    pub tactic_tokens: i16,
    pub fleet_tokens: i16,
    pub strategic_tokens: i16,
    pub trade_goods: i16,
    pub commodities: i16,
    pub victory_points: i16,
    pub passed: bool,
    pub public_objectives_forbidden: bool,
    pub stability: bool,
    /// `Player.duress_by`.
    pub duress_by: Option<Seat>,
}
