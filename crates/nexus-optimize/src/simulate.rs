//! Digital Twin Simulation Mode.
//!
//! Before deploying to production, a plan is executed in a simulated sandbox
//! using historical telemetry (the knowledge-loop profiles). The simulator
//! produces a predictive impact report — latency, cost, success probability —
//! and flags budget or reliability violations *before* any real action is taken.

use nexus_planner::{Processor, Schedule, TaskDag};
use nexus_runtime::KnowledgeLoop;

/// A predictive impact report for a planned workflow.
#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub predicted_latency_ms: f64,
    pub predicted_cost: f64,
    pub success_probability: f64,
    pub makespan_ms: f64,
    pub violations: Vec<String>,
}

impl ImpactReport {
    pub fn is_safe_to_deploy(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Run a digital-twin simulation of `dag`, whose tasks map to `task_names`,
/// using learned `profiles`, scheduled by `schedule` on `procs`, against a
/// contract `(max_cost, max_time_ms)` budget.
pub fn simulate(
    dag: &TaskDag,
    task_names: &[String],
    profiles: &KnowledgeLoop,
    schedule: &Schedule,
    _procs: &[Processor],
    budget: (f64, f64),
) -> ImpactReport {
    let (max_cost, max_time_ms) = budget;

    // Predicted per-task latency and cost come from historical telemetry,
    // falling back to the structural estimate where no history exists.
    let mut predicted_latency_ms = 0.0;
    let mut predicted_cost = 0.0;
    let mut success_probability = 1.0;
    for t in 0..dag.len() {
        let name = &task_names[t];
        let default_latency = (dag.work[t]) * 100.0; // 100ms per work unit baseline
        predicted_latency_ms += profiles.predict_latency(name, default_latency);
        predicted_cost += profiles.predict_cost(name, 0.01);
        let sr = profiles
            .profile(name)
            .map(|p| if p.samples > 0 { p.success_rate } else { 1.0 })
            .unwrap_or(1.0);
        success_probability *= sr;
    }

    let mut violations = Vec::new();
    if predicted_cost > max_cost + 1e-9 {
        violations.push(format!(
            "predicted cost ${:.4} exceeds budget ${:.4}",
            predicted_cost, max_cost
        ));
    }
    if schedule.makespan * 1000.0 > max_time_ms + 1e-9 {
        violations.push(format!(
            "makespan {:.0}ms exceeds timeout {:.0}ms",
            schedule.makespan * 1000.0,
            max_time_ms
        ));
    }
    if success_probability < 0.95 {
        violations.push(format!(
            "success probability {:.3} below 0.95 reliability bar",
            success_probability
        ));
    }

    ImpactReport {
        predicted_latency_ms,
        predicted_cost,
        success_probability,
        makespan_ms: schedule.makespan * 1000.0,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_planner::{heft, Processor};

    fn dag() -> (TaskDag, Vec<String>) {
        let mut g = TaskDag::new();
        g.add_task("Validate", 1.0);
        g.add_task("Charge", 2.0);
        g.add_task("Receipt", 5.0);
        g.add_dep(0, 1, 1.0);
        g.add_dep(1, 2, 1.0);
        let names = vec!["Validate".into(), "Charge".into(), "Receipt".into()];
        (g, names)
    }

    fn procs() -> Vec<Processor> {
        vec![Processor::new("local", 1.0, 0.001, 1.0)]
    }

    #[test]
    fn clean_simulation_is_deployable() {
        let (g, names) = dag();
        let p = procs();
        let sched = heft(&g, &p);
        let k = KnowledgeLoop::new();
        // Generous budget: cost ceiling high, time ceiling high.
        let report = simulate(&g, &names, &k, &sched, &p, (1.0, 100_000.0));
        assert!(report.is_safe_to_deploy(), "violations: {:?}", report.violations);
        assert!(report.predicted_latency_ms > 0.0);
    }

    #[test]
    fn catches_cost_spike_before_deploy() {
        let (g, names) = dag();
        let p = procs();
        let sched = heft(&g, &p);
        let mut k = KnowledgeLoop::new();
        // History shows Charge is expensive.
        for _ in 0..5 {
            k.record("Charge", 300.0, 0.50, true);
        }
        let report = simulate(&g, &names, &k, &sched, &p, (0.10, 100_000.0));
        assert!(!report.is_safe_to_deploy());
        assert!(report.violations.iter().any(|v| v.contains("cost")));
    }

    #[test]
    fn catches_reliability_risk() {
        let (g, names) = dag();
        let p = procs();
        let sched = heft(&g, &p);
        let mut k = KnowledgeLoop::new();
        // Charge fails often.
        for _ in 0..10 {
            k.record("Charge", 100.0, 0.01, false);
        }
        let report = simulate(&g, &names, &k, &sched, &p, (10.0, 100_000.0));
        assert!(report.success_probability < 0.95);
        assert!(!report.is_safe_to_deploy());
    }
}
