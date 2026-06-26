//! Resource-aware scheduling: HEFT plus multi-objective Pareto routing.
//!
//! Tasks are mapped to heterogeneous processors to balance latency, cost, and
//! energy. HEFT gives a strong static schedule; a weighted-scalarization list
//! scheduler samples the Pareto front so the planner can pick the schedule that
//! fits a contract's budget — or decide a task should not run at all.

use crate::dag::{TaskDag, TaskId};

/// A heterogeneous compute resource (local core, edge server, cloud node …).
#[derive(Clone, Debug)]
pub struct Processor {
    pub name: String,
    /// Work units processed per second (higher is faster).
    pub speed: f64,
    /// Dollars per second of compute.
    pub cost_per_sec: f64,
    /// Energy (joules) per second of compute.
    pub energy_per_sec: f64,
    /// Peak memory available on this node (bytes) — the hard constraint.
    pub memory: f64,
}

impl Processor {
    pub fn new(name: &str, speed: f64, cost_per_sec: f64, energy_per_sec: f64) -> Self {
        Processor { name: name.into(), speed, cost_per_sec, energy_per_sec, memory: f64::INFINITY }
    }
}

/// A concrete mapping of tasks to processors with timing and cost metrics.
#[derive(Clone, Debug)]
pub struct Schedule {
    pub proc_of: Vec<usize>,
    pub start: Vec<f64>,
    pub finish: Vec<f64>,
    pub makespan: f64,
    pub cost: f64,
    pub energy: f64,
}

/// Global communication bandwidth (data units per second) between processors.
const BANDWIDTH: f64 = 100.0;

fn comp_cost(g: &TaskDag, t: TaskId, p: &Processor) -> f64 {
    g.work[t] / p.speed
}

fn comm_cost(g: &TaskDag, pred: TaskId, succ: TaskId, p_pred: usize, p_succ: usize) -> f64 {
    if p_pred == p_succ {
        0.0
    } else {
        g.data.get(&(pred, succ)).copied().unwrap_or(0.0) / BANDWIDTH
    }
}

/// Upward rank (ranku) used by HEFT to prioritize tasks on the critical path.
fn upward_rank(g: &TaskDag, procs: &[Processor]) -> Vec<f64> {
    let avg_comp: Vec<f64> = (0..g.len())
        .map(|t| procs.iter().map(|p| comp_cost(g, t, p)).sum::<f64>() / procs.len() as f64)
        .collect();
    let order = g.topo_order().expect("DAG required");
    let mut rank = vec![0f64; g.len()];
    for &t in order.iter().rev() {
        let succ_term = g.succs[t]
            .iter()
            .map(|&s| {
                let avg_comm = g.data.get(&(t, s)).copied().unwrap_or(0.0) / BANDWIDTH;
                avg_comm + rank[s]
            })
            .fold(0.0, f64::max);
        rank[t] = avg_comp[t] + succ_term;
    }
    rank
}

/// A processor-selection strategy: given candidate finish times and the cost /
/// energy of placing a task on each processor, return the chosen processor.
type Selector = dyn Fn(&[f64], &[f64], &[f64]) -> usize;

/// Core list scheduler shared by HEFT and the multi-objective sampler.
fn list_schedule(g: &TaskDag, procs: &[Processor], select: &Selector) -> Schedule {
    let n = g.len();
    let rank = upward_rank(g, procs);
    let mut order: Vec<TaskId> = (0..n).collect();
    // HEFT order: descending rank, ties broken by id for determinism.
    order.sort_by(|&a, &b| {
        rank[b].partial_cmp(&rank[a]).unwrap().then(a.cmp(&b))
    });

    let mut avail = vec![0f64; procs.len()];
    let mut proc_of = vec![0usize; n];
    let mut start = vec![0f64; n];
    let mut finish = vec![0f64; n];

    for &t in &order {
        let mut efts = Vec::with_capacity(procs.len());
        let mut costs = Vec::with_capacity(procs.len());
        let mut energies = Vec::with_capacity(procs.len());
        for (pi, p) in procs.iter().enumerate() {
            let ct = comp_cost(g, t, p);
            // Earliest start = max(proc free, all predecessor data ready).
            let mut est = avail[pi];
            for &pred in &g.deps[t] {
                let ready = finish[pred] + comm_cost(g, pred, t, proc_of[pred], pi);
                est = est.max(ready);
            }
            efts.push(est + ct);
            costs.push(ct * p.cost_per_sec);
            energies.push(ct * p.energy_per_sec);
        }
        let chosen = select(&efts, &costs, &energies);
        proc_of[t] = chosen;
        finish[t] = efts[chosen];
        start[t] = efts[chosen] - comp_cost(g, t, &procs[chosen]);
        avail[chosen] = efts[chosen];
    }

    let makespan = finish.iter().copied().fold(0.0, f64::max);
    let cost = (0..n).map(|t| comp_cost(g, t, &procs[proc_of[t]]) * procs[proc_of[t]].cost_per_sec).sum();
    let energy = (0..n).map(|t| comp_cost(g, t, &procs[proc_of[t]]) * procs[proc_of[t]].energy_per_sec).sum();
    Schedule { proc_of, start, finish, makespan, cost, energy }
}

/// Heterogeneous Earliest Finish Time: minimize each task's earliest finish.
pub fn heft(g: &TaskDag, procs: &[Processor]) -> Schedule {
    list_schedule(g, procs, &|efts, _costs, _energy| argmin(efts))
}

/// Sample the Pareto front over (makespan, cost, energy) by scalarizing with a
/// grid of weight vectors and keeping the non-dominated schedules.
pub fn pareto_front(g: &TaskDag, procs: &[Processor]) -> Vec<Schedule> {
    let mut schedules = Vec::new();
    let steps = [0.0, 0.25, 0.5, 0.75, 1.0];
    for &wt in &steps {
        for &wc in &steps {
            let we: f64 = (1.0_f64 - wt - wc).max(0.0);
            if wt + wc > 1.0 {
                continue;
            }
            // Normalize the three objectives roughly before weighting.
            let sched = list_schedule(g, procs, &move |efts, costs, energies| {
                let nt = normalize(efts);
                let nc = normalize(costs);
                let ne = normalize(energies);
                let scored: Vec<f64> = (0..efts.len())
                    .map(|i| wt * nt[i] + wc * nc[i] + we * ne[i])
                    .collect();
                argmin(&scored)
            });
            schedules.push(sched);
        }
    }
    non_dominated(schedules)
}

/// Route within a contract budget: the cheapest non-dominated schedule whose
/// cost and makespan both fit. `None` means the task should not run as-is and
/// the planner must reroute or fall back — the spec's budget-aware decision.
pub fn route_within_budget(
    g: &TaskDag,
    procs: &[Processor],
    max_cost: f64,
    max_time: f64,
) -> Option<Schedule> {
    pareto_front(g, procs)
        .into_iter()
        .filter(|s| s.cost <= max_cost + 1e-9 && s.makespan <= max_time + 1e-9)
        .min_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap())
}

fn argmin(v: &[f64]) -> usize {
    let mut best = 0;
    for i in 1..v.len() {
        if v[i] < v[best] {
            best = i;
        }
    }
    best
}

fn normalize(v: &[f64]) -> Vec<f64> {
    let max = v.iter().copied().fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return vec![0.0; v.len()];
    }
    v.iter().map(|x| x / max).collect()
}

fn dominates(a: &Schedule, b: &Schedule) -> bool {
    let le = a.makespan <= b.makespan + 1e-9 && a.cost <= b.cost + 1e-9 && a.energy <= b.energy + 1e-9;
    let lt = a.makespan < b.makespan - 1e-9 || a.cost < b.cost - 1e-9 || a.energy < b.energy - 1e-9;
    le && lt
}

fn non_dominated(schedules: Vec<Schedule>) -> Vec<Schedule> {
    let mut front: Vec<Schedule> = Vec::new();
    for s in schedules {
        if front.iter().any(|f| dominates(f, &s)) {
            continue;
        }
        front.retain(|f| !dominates(&s, f));
        // Avoid near-duplicates.
        if !front.iter().any(|f| {
            (f.makespan - s.makespan).abs() < 1e-9
                && (f.cost - s.cost).abs() < 1e-9
                && (f.energy - s.energy).abs() < 1e-9
        }) {
            front.push(s);
        }
    }
    front
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abcd() -> TaskDag {
        let mut g = TaskDag::new();
        let a = g.add_task("A", 10.0);
        let b = g.add_task("B", 10.0);
        let c = g.add_task("C", 10.0);
        let d = g.add_task("D", 10.0);
        g.add_dep(a, c, 1.0);
        g.add_dep(b, d, 1.0);
        g
    }

    fn two_procs() -> Vec<Processor> {
        vec![
            Processor::new("local", 1.0, 0.0, 5.0),
            Processor::new("edge", 1.0, 0.01, 8.0),
        ]
    }

    #[test]
    fn heft_exploits_parallelism() {
        let g = abcd();
        let s = heft(&g, &two_procs());
        // Serial would be 40s; with 2 procs the makespan must beat that.
        assert!(s.makespan < 40.0);
        assert!(s.makespan <= 21.0); // ~two tasks per proc + tiny comm
    }

    #[test]
    fn single_proc_is_serial() {
        let g = abcd();
        let s = heft(&g, &[Processor::new("only", 1.0, 0.0, 1.0)]);
        assert!((s.makespan - 40.0).abs() < 1e-9);
    }

    #[test]
    fn pareto_front_is_nondominated() {
        let g = abcd();
        let front = pareto_front(&g, &two_procs());
        assert!(!front.is_empty());
        for i in 0..front.len() {
            for j in 0..front.len() {
                if i != j {
                    assert!(!dominates(&front[i], &front[j]));
                }
            }
        }
    }

    #[test]
    fn budget_routing_picks_feasible() {
        let g = abcd();
        let procs = two_procs();
        // Generous budget -> some schedule fits.
        let s = route_within_budget(&g, &procs, 1.0, 100.0).unwrap();
        assert!(s.cost <= 1.0);
        // Impossible time budget -> nothing fits, planner must reroute/fallback.
        assert!(route_within_budget(&g, &procs, 1.0, 1.0).is_none());
    }
}
