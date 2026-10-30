//! Self-Assembly Routines (`sar`), first ability (package W12; string `technology::self_assembly_after_production`): "After 1 or more
//! of your units use PRODUCTION, you may exhaust this card to place 1 mech from your reinforcements on a planet you control in that
//! system." Asked once at the end of a use of PRODUCTION that produced units, from rules version `RULES_SELF_ASSEMBLY`, while the
//! card is ready, a mech is left in the reinforcements and the seat controls a planet in the system. A single planet is still a
//! question (the exhaust is a "may"). The mech is the form the seat places (its own faction mech); as in the string, the placement
//! does not announce a Naaz mech (a copied quirk: the other placements go through `place_for` and do).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::stage::{Illegal, Sx, Window};
use ti4_engine_core::kernel::{fx, fx_strategy, Kernel, Next};
use ti4_engine_core::query::{board, fleet};
use ti4_engine_core::state::CoreState;

/// `ti4_model::state::RULES_SELF_ASSEMBLY`.
const RULES_SELF_ASSEMBLY: u32 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

/// The question at the end of a use of PRODUCTION in `system`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfAssembly {
    pub who: Seat,
    pub system: System,
    stage: Stage,
    mech: Option<Unit>,
}

/// The mech the seat places, when the card applies: ready, a mech in the reinforcements, the placement not forbidden.
fn mech_to_place(s: &CoreState, k: &Kernel, who: Seat) -> Option<Unit> {
    let p = s.players().by_seat(who)?;
    if !s.meta().rules_at_least(RULES_SELF_ASSEMBLY) || !p.tech.owned.contains(Technology::SelfAssemblyRoutines) || p.tech.exhausted.contains(Technology::SelfAssemblyRoutines) {
        return None;
    }
    let scope = crate::common::scope(k);
    let roster = crate::cd_registry::yin::timing::roster(scope);
    let mech = fx_strategy::placed_unit(s, k, scope, &roster, who, UnitBaseType::Mech)?;
    (!k.hooks.economy.effect_placement_forbidden(s, k, who, mech) && fleet::remaining_all(s, scope, who, mech) > 0).then_some(mech)
}

fn planets(s: &CoreState, who: Seat, system: System) -> Vec<Planet> {
    board::controlled_planets(s, who).as_slice().iter().filter(|(here, _)| *here == system).map(|(_, p)| *p).collect()
}

impl SelfAssembly {
    #[must_use]
    pub const fn new(who: Seat, system: System) -> Self {
        Self { who, system, stage: Stage::Start, mech: None }
    }
}

impl Window for SelfAssembly {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.mech = mech_to_place(sx.state, sx.kernel, self.who);
            self.stage = if self.mech.is_some() && !planets(sx.state, self.who, self.system).is_empty() { Stage::Asking } else { Stage::Done };
        }
        if self.stage == Stage::Asking { Next::Continue(Ask::Technology(TAsk::SelfAssembly)) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::SelfAssembly),
            ctx: Ctx {
                origin: Origin::Content(Content::Technology(Technology::SelfAssemblyRoutines)),
                subtype: Subtype::Technology(TSub::SelfAssemblyMech),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: planets(s, self.who, self.system)
                .into_iter()
                .map(|p| Opt::Technology(TOpt::Planet(p, ti4_engine_core::choice::agenda::Shown::default())))
                .chain(std::iter::once(Opt::Decline))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Technology(TOpt::Planet(planet, _))) if planets(sx.state, self.who, self.system).contains(&planet) => {
                let mech = self.mech.ok_or(Illegal)?;
                fx_strategy::exhaust_technology(sx.state, self.who, Technology::SelfAssemblyRoutines);
                fx::place_unit(sx.state, self.who, self.system, Some(planet), mech);
            }
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}
