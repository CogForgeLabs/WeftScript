//! The task DAG and native auto-parallelism.
//!
//! The planner evaluates the data-dependency vectors of the plan graph and
//! groups independent tasks into levels that run concurrently — replacing
//! hand-written async/await and thread orchestration with structure derived
//! directly from the dependency graph.

use std::collections::HashMap;

pub type TaskId = usize;

/// A directed acyclic graph of computational tasks with work weights and
/// inter-task data volumes.
#[derive(Default, Clone, Debug)]
pub struct TaskDag {
    pub names: Vec<String>,
    pub work: Vec<f64>,
    pub deps: Vec<Vec<TaskId>>,
    pub succs: Vec<Vec<TaskId>>,
    pub data: HashMap<(TaskId, TaskId), f64>,
}

impl TaskDag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_task(&mut self, name: impl Into<String>, work: f64) -> TaskId {
        let id = self.names.len();
        self.names.push(name.into());
        self.work.push(work);
        self.deps.push(Vec::new());
        self.succs.push(Vec::new());
        id
    }

    /// Declare that `succ` consumes `data` units produced by `pred`.
    pub fn add_dep(&mut self, pred: TaskId, succ: TaskId, data: f64) {
        self.deps[succ].push(pred);
        self.succs[pred].push(succ);
        self.data.insert((pred, succ), data);
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Topological order, or `None` if the graph contains a cycle.
    pub fn topo_order(&self) -> Option<Vec<TaskId>> {
        let n = self.len();
        let mut indeg = vec![0usize; n];
        for t in 0..n {
            indeg[t] = self.deps[t].len();
        }
        let mut queue: Vec<TaskId> = (0..n).filter(|t| indeg[*t] == 0).collect();
        queue.sort(); // deterministic
        let mut order = Vec::with_capacity(n);
        let mut head = 0;
        while head < queue.len() {
            let t = queue[head];
            head += 1;
            order.push(t);
            let mut newly = Vec::new();
            for &s in &self.succs[t] {
                indeg[s] -= 1;
                if indeg[s] == 0 {
                    newly.push(s);
                }
            }
            newly.sort();
            queue.extend(newly);
        }
        if order.len() == n {
            Some(order)
        } else {
            None
        }
    }

    pub fn is_dag(&self) -> bool {
        self.topo_order().is_some()
    }

    /// Auto-parallel levels: tasks sharing a level have no dependency between
    /// them and may execute concurrently. Level = longest dependency depth.
    pub fn levels(&self) -> Vec<Vec<TaskId>> {
        let order = self.topo_order().expect("levels() requires a DAG");
        let mut depth = vec![0usize; self.len()];
        for &t in &order {
            let d = self.deps[t].iter().map(|p| depth[*p] + 1).max().unwrap_or(0);
            depth[t] = d;
        }
        let max_depth = depth.iter().copied().max().unwrap_or(0);
        let mut levels = vec![Vec::new(); max_depth + 1];
        for t in 0..self.len() {
            levels[depth[t]].push(t);
        }
        for l in &mut levels {
            l.sort();
        }
        levels
    }

    /// Maximum number of tasks runnable at once (parallel width).
    pub fn max_width(&self) -> usize {
        self.levels().iter().map(|l| l.len()).max().unwrap_or(0)
    }

    /// Serial work: the cost of running every task one after another on a unit
    /// processor. Used as the baseline for speedup measurements.
    pub fn serial_work(&self) -> f64 {
        self.work.iter().sum()
    }

    /// Critical-path length on a unit processor (ignores communication): the
    /// best achievable makespan with unlimited parallelism.
    pub fn critical_path(&self) -> f64 {
        let order = self.topo_order().expect("critical_path requires a DAG");
        let mut finish = vec![0f64; self.len()];
        for &t in &order {
            let ready = self.deps[t].iter().map(|p| finish[*p]).fold(0.0, f64::max);
            finish[t] = ready + self.work[t];
        }
        finish.iter().copied().fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A and B independent; C needs A; D needs B (the spec's example).
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

    #[test]
    fn auto_parallel_levels() {
        let g = abcd();
        let levels = g.levels();
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0], vec![0, 1]); // A, B run together
        assert_eq!(levels[1], vec![2, 3]); // C, D run together
        assert_eq!(g.max_width(), 2);
    }

    #[test]
    fn critical_path_vs_serial() {
        let g = abcd();
        assert_eq!(g.serial_work(), 40.0);
        assert_eq!(g.critical_path(), 20.0); // A->C or B->D
    }

    #[test]
    fn detects_cycle() {
        let mut g = TaskDag::new();
        let a = g.add_task("A", 1.0);
        let b = g.add_task("B", 1.0);
        g.add_dep(a, b, 1.0);
        g.add_dep(b, a, 1.0);
        assert!(!g.is_dag());
    }
}
