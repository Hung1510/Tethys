//! Build optimization.
//!
//! The search problem: pick five echoes from an [`Inventory`] respecting the
//! `[4,3,3,1,1]` cost layout (and any set / main-stat constraints) so that an
//! [`Evaluator`] score is maximised, never using the same physical echo twice.
//!
//! Two solvers are provided:
//!
//! * [`optimize_ga`] — a genetic algorithm. This is the general solver; it
//!   handles large inventories where exhaustive search is infeasible.
//! * [`optimize_exhaustive`] — brute force over every valid combination. Only
//!   viable for small candidate pools, but it returns the *provable* optimum,
//!   which makes it the oracle the GA is tested against.

use crate::model::{Build, Echo, EchoSet, Inventory, SlotGroup, Stat};
use crate::score::Evaluator;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

/// Constraints on a build beyond the fixed cost layout.
#[derive(Debug, Clone, Default)]
pub struct BuildSpec {
    /// If set, only echoes of this set are considered (the common
    /// "optimize my five-piece X" request).
    pub required_set: Option<EchoSet>,
    /// Optional whitelist of acceptable main stats for the 4-cost slot.
    pub cost4_main: Option<Vec<Stat>>,
    /// Optional whitelist of acceptable main stats for the 3-cost slots.
    pub cost3_main: Option<Vec<Stat>>,
    /// Optional whitelist of acceptable main stats for the 1-cost slots.
    pub cost1_main: Option<Vec<Stat>>,
}

impl BuildSpec {
    fn main_filter(&self, group: SlotGroup) -> Option<&Vec<Stat>> {
        match group {
            SlotGroup::Cost4 => self.cost4_main.as_ref(),
            SlotGroup::Cost3 => self.cost3_main.as_ref(),
            SlotGroup::Cost1 => self.cost1_main.as_ref(),
        }
    }
}

/// The outcome of an optimization run.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizeResult {
    /// Echo ids for slots in `[4,3,3,1,1]` order.
    pub echo_ids: [u32; 5],
    pub score: f32,
    /// How many builds were scored (a cost/complexity signal for the UI).
    pub evaluations: u64,
    pub method: &'static str,
}

/// Errors that prevent optimization from producing a build.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum OptimizeError {
    #[error("no eligible echo for the {0:?} slot; loosen the spec or scan more echoes")]
    EmptySlot(SlotGroup),
    #[error("need at least two distinct {0:?} echoes")]
    NotEnough(SlotGroup),
    #[error("exhaustive search space of {0} combinations is too large; use the genetic solver")]
    TooLarge(u128),
}

/// Candidate echo indices (into `inventory.echoes`) for each slot group.
struct Pools {
    cost4: Vec<usize>,
    cost3: Vec<usize>,
    cost1: Vec<usize>,
}

impl Pools {
    fn build(inv: &Inventory, spec: &BuildSpec) -> Result<Self, OptimizeError> {
        let pick = |group: SlotGroup| -> Vec<usize> {
            inv.echoes
                .iter()
                .enumerate()
                .filter(|(_, e)| e.slot_group() == Some(group))
                .filter(|(_, e)| match spec.required_set {
                    Some(set) => e.set == set,
                    None => true,
                })
                .filter(|(_, e)| match spec.main_filter(group) {
                    Some(allowed) => allowed.contains(&e.main_stat.stat),
                    None => true,
                })
                .map(|(i, _)| i)
                .collect()
        };

        let cost4 = pick(SlotGroup::Cost4);
        let cost3 = pick(SlotGroup::Cost3);
        let cost1 = pick(SlotGroup::Cost1);

        if cost4.is_empty() {
            return Err(OptimizeError::EmptySlot(SlotGroup::Cost4));
        }
        if cost3.len() < 2 {
            return Err(OptimizeError::NotEnough(SlotGroup::Cost3));
        }
        if cost1.len() < 2 {
            return Err(OptimizeError::NotEnough(SlotGroup::Cost1));
        }
        Ok(Pools {
            cost4,
            cost3,
            cost1,
        })
    }
}

/// Resolve inventory indices into a scoreable [`Build`], or `None` if the same
/// echo would occupy two same-cost slots.
fn resolve<'a>(
    inv: &'a Inventory,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
) -> Option<Build<'a>> {
    if s1 == s2 || s3 == s4 {
        return None;
    }
    let get = |i: usize| -> &Echo { &inv.echoes[i] };
    Some(Build {
        slots: [get(s0), get(s1), get(s2), get(s3), get(s4)],
    })
}

/// Tuning knobs for the genetic algorithm.
#[derive(Debug, Clone)]
pub struct GaConfig {
    pub population: usize,
    pub generations: usize,
    pub tournament_size: usize,
    pub mutation_rate: f64,
    pub elitism: usize,
    /// Seed for reproducibility. Fix it in tests; randomise in production.
    pub seed: u64,
}

impl Default for GaConfig {
    fn default() -> Self {
        Self {
            population: 200,
            generations: 150,
            tournament_size: 4,
            mutation_rate: 0.15,
            elitism: 4,
            seed: 0xC0FFEE,
        }
    }
}

/// A genome is a choice of index *within each slot's pool*.
type Genome = [usize; 5];

/// Optimize with a genetic algorithm. General-purpose; scales to large
/// inventories where [`optimize_exhaustive`] is infeasible.
pub fn optimize_ga(
    inv: &Inventory,
    spec: &BuildSpec,
    eval: &dyn Evaluator,
    cfg: &GaConfig,
) -> Result<OptimizeResult, OptimizeError> {
    let pools = Pools::build(inv, spec)?;
    let mut rng = StdRng::seed_from_u64(cfg.seed);
    let mut evaluations: u64 = 0;

    // Map a pool-relative genome to inventory indices.
    let to_inv = |g: &Genome| -> (usize, usize, usize, usize, usize) {
        (
            pools.cost4[g[0]],
            pools.cost3[g[1]],
            pools.cost3[g[2]],
            pools.cost1[g[3]],
            pools.cost1[g[4]],
        )
    };

    let fitness = |g: &Genome, evaluations: &mut u64| -> f32 {
        let (a, b, c, d, e) = to_inv(g);
        match resolve(inv, a, b, c, d, e) {
            Some(build) => {
                *evaluations += 1;
                eval.score(&build)
            }
            // Invalid (duplicate echo): heavily penalised so selection avoids it.
            None => f32::NEG_INFINITY,
        }
    };

    let random_genome = |rng: &mut StdRng| -> Genome {
        // Distinct picks for the paired cost-3 and cost-1 slots.
        let (c3a, c3b) = distinct_pair(pools.cost3.len(), rng);
        let (c1a, c1b) = distinct_pair(pools.cost1.len(), rng);
        [rng.gen_range(0..pools.cost4.len()), c3a, c3b, c1a, c1b]
    };

    // Initial population.
    let mut pop: Vec<Genome> = (0..cfg.population)
        .map(|_| random_genome(&mut rng))
        .collect();

    let mut best: Option<(Genome, f32)> = None;

    for _ in 0..cfg.generations {
        // Evaluate.
        let mut scored: Vec<(Genome, f32)> = pop
            .iter()
            .map(|g| (*g, fitness(g, &mut evaluations)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((g, s)) = scored.first() {
            if best.as_ref().map(|(_, bs)| *s > *bs).unwrap_or(true) {
                best = Some((*g, *s));
            }
        }

        // Next generation: elitism + offspring.
        let mut next: Vec<Genome> = scored.iter().take(cfg.elitism).map(|(g, _)| *g).collect();
        while next.len() < cfg.population {
            let p1 = tournament(&scored, cfg.tournament_size, &mut rng);
            let p2 = tournament(&scored, cfg.tournament_size, &mut rng);
            let mut child = crossover(&p1, &p2, &mut rng);
            mutate(&mut child, &pools, cfg.mutation_rate, &mut rng);
            repair(&mut child, &pools, &mut rng);
            next.push(child);
        }
        pop = next;
    }

    let (genome, score) = best.ok_or(OptimizeError::EmptySlot(SlotGroup::Cost4))?;
    let (a, b, c, d, e) = to_inv(&genome);
    Ok(OptimizeResult {
        echo_ids: [
            inv.echoes[a].id,
            inv.echoes[b].id,
            inv.echoes[c].id,
            inv.echoes[d].id,
            inv.echoes[e].id,
        ],
        score,
        evaluations,
        method: "genetic",
    })
}

/// Exhaustively evaluate every valid build. Returns the provable optimum.
/// Refuses (with [`OptimizeError::TooLarge`]) above `max_combinations` so it is
/// never accidentally called on a whale's inventory.
pub fn optimize_exhaustive(
    inv: &Inventory,
    spec: &BuildSpec,
    eval: &dyn Evaluator,
    max_combinations: u128,
) -> Result<OptimizeResult, OptimizeError> {
    let pools = Pools::build(inv, spec)?;
    let n4 = pools.cost4.len() as u128;
    let n3 = pools.cost3.len() as u128;
    let n1 = pools.cost1.len() as u128;
    // C(n,2) distinct unordered pairs for the paired slots.
    let combos = n4 * (n3 * (n3 - 1) / 2) * (n1 * (n1 - 1) / 2);
    if combos > max_combinations {
        return Err(OptimizeError::TooLarge(combos));
    }

    let mut best: Option<(usize, usize, usize, usize, usize, f32)> = None;
    let mut evaluations: u64 = 0;

    for &s0 in &pools.cost4 {
        for i in 0..pools.cost3.len() {
            for j in (i + 1)..pools.cost3.len() {
                for k in 0..pools.cost1.len() {
                    for l in (k + 1)..pools.cost1.len() {
                        let (s1, s2) = (pools.cost3[i], pools.cost3[j]);
                        let (s3, s4) = (pools.cost1[k], pools.cost1[l]);
                        if let Some(build) = resolve(inv, s0, s1, s2, s3, s4) {
                            evaluations += 1;
                            let sc = eval.score(&build);
                            if best.as_ref().map(|b| sc > b.5).unwrap_or(true) {
                                best = Some((s0, s1, s2, s3, s4, sc));
                            }
                        }
                    }
                }
            }
        }
    }

    let (s0, s1, s2, s3, s4, score) = best.ok_or(OptimizeError::EmptySlot(SlotGroup::Cost4))?;
    Ok(OptimizeResult {
        echo_ids: [
            inv.echoes[s0].id,
            inv.echoes[s1].id,
            inv.echoes[s2].id,
            inv.echoes[s3].id,
            inv.echoes[s4].id,
        ],
        score,
        evaluations,
        method: "exhaustive",
    })
}

// --- GA operators -----------------------------------------------------------

fn distinct_pair(len: usize, rng: &mut StdRng) -> (usize, usize) {
    debug_assert!(len >= 2);
    let a = rng.gen_range(0..len);
    let mut b = rng.gen_range(0..len - 1);
    if b >= a {
        b += 1;
    }
    (a, b)
}

fn tournament(scored: &[(Genome, f32)], k: usize, rng: &mut StdRng) -> Genome {
    let mut best: Option<(Genome, f32)> = None;
    for _ in 0..k.max(1) {
        let pick = scored.choose(rng).copied().unwrap();
        if best.as_ref().map(|b| pick.1 > b.1).unwrap_or(true) {
            best = Some(pick);
        }
    }
    best.unwrap().0
}

fn crossover(p1: &Genome, p2: &Genome, rng: &mut StdRng) -> Genome {
    let mut child = *p1;
    for slot in 0..5 {
        if rng.gen_bool(0.5) {
            child[slot] = p2[slot];
        }
    }
    child
}

fn mutate(g: &mut Genome, pools: &Pools, rate: f64, rng: &mut StdRng) {
    let lens = [
        pools.cost4.len(),
        pools.cost3.len(),
        pools.cost3.len(),
        pools.cost1.len(),
        pools.cost1.len(),
    ];
    for slot in 0..5 {
        if rng.gen_bool(rate) {
            g[slot] = rng.gen_range(0..lens[slot]);
        }
    }
}

/// Fix genomes that ended up with duplicate picks in a paired slot group,
/// keeping the population feasible so selection pressure is not wasted.
fn repair(g: &mut Genome, pools: &Pools, rng: &mut StdRng) {
    if g[1] == g[2] && pools.cost3.len() >= 2 {
        let (a, b) = distinct_pair(pools.cost3.len(), rng);
        g[1] = a;
        g[2] = b;
    }
    if g[3] == g[4] && pools.cost1.len() >= 2 {
        let (a, b) = distinct_pair(pools.cost1.len(), rng);
        g[3] = a;
        g[4] = b;
    }
}
