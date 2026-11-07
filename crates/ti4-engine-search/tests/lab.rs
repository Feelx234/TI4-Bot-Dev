//! The rule bot with its typed view at every seat, through the search API, is the MCTS lab's game (`ti4_mcts_lab::play`: the product's
//! native step without the wrapper, the question put synchronously): the same answers, the same steps, the same final core.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use ti4_engine_search::agent::{Answer, RuleBotAgent, SeatAgent};
use ti4_engine_search::driver::{self, Limits};

struct Recording {
    ids: Rc<RefCell<Vec<String>>>,
}

impl ti4_mcts_lab::Decide for Recording {
    fn decide(&mut self, t: &ti4_mcts_lab::Table, a: &ti4_mcts_lab::Asked<'_>) -> Result<usize, String> {
        let picked = ti4_mcts_lab::rule_bot(t, a).0;
        self.ids.borrow_mut().push(a.rendered.options[picked].id.clone());
        Ok(picked)
    }
}

fn hash_debug(core: &ti4_engine_core::state::CoreState) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{core:?}").hash(&mut h);
    h.finish()
}

#[test]
fn the_rule_bot_with_its_view_plays_the_labs_game() {
    for seed in [1u64, 2] {
        let (table, mut lab) = ti4_mcts_lab::start(&ti4_mcts_lab::spec(seed)).unwrap();
        let ids = Rc::new(RefCell::new(Vec::new()));
        let mut dec = Recording { ids: Rc::clone(&ids) };
        let ended = ti4_mcts_lab::play(&table, &mut lab, &mut dec, 20_000, 30, |_, _| {});
        assert_eq!(ended.error, None);

        let mut pos = ti4_engine_search::start(&ti4_engine_search::mcts_spec(seed)).unwrap();
        let mut agents: Vec<Box<dyn SeatAgent>> = (0..4).map(|_| Box::new(RuleBotAgent::seeing()) as Box<dyn SeatAgent>).collect();
        let mut mine = Vec::new();
        let got = driver::play(&mut pos, &mut agents, Limits { max_steps: 20_000, max_rounds: 30 }, false, |_, r| {
            if let Answer::Id(id) = &r.answer {
                mine.push(id.clone());
            }
        });
        assert_eq!(got.error, None);
        assert_eq!(mine, *ids.borrow(), "seed {seed}: the answers");
        assert_eq!(got.steps, ended.steps, "seed {seed}: steps");
        assert_eq!(got.decisions, ended.decisions, "seed {seed}: decisions");
        assert_eq!(got.outcome.vp, ended.vp);
        assert_eq!(pos.core_hash(), hash_debug(&lab.core), "seed {seed}: the final core");
        eprintln!("seed {seed}: {} steps, {} decisions, {:?}", got.steps, got.decisions, got.outcome);
    }
}
