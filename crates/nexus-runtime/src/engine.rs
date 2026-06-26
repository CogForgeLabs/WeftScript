//! The execution engine.
//!
//! Independent tasks (those sharing a DAG level) run concurrently on real OS
//! threads — concurrency is derived from graph structure, not hand-written
//! async. Each task's output is content-addressed and recorded in a provenance
//! chain. Deterministic and probabilistic nodes are tracked separately so the
//! compiler can keep probabilistic outputs off safety-critical paths.

use crate::provenance::Lineage;
use nexus_core::Cid;
use nexus_planner::TaskDag;
use std::thread;
use std::time::Instant;

/// What a task does: consume predecessor outputs, produce bytes.
pub trait NodeOp: Send + Sync {
    fn run(&self, inputs: &[&[u8]]) -> Vec<u8>;
    fn op_name(&self) -> String;
    /// Deterministic nodes guarantee repeatable, verifiable output.
    fn deterministic(&self) -> bool {
        true
    }
}

/// A closure-based op for ergonomic construction.
pub struct FnOp<F: Fn(&[&[u8]]) -> Vec<u8> + Send + Sync> {
    pub name: String,
    pub det: bool,
    pub f: F,
}

impl<F: Fn(&[&[u8]]) -> Vec<u8> + Send + Sync> NodeOp for FnOp<F> {
    fn run(&self, inputs: &[&[u8]]) -> Vec<u8> {
        (self.f)(inputs)
    }
    fn op_name(&self) -> String {
        self.name.clone()
    }
    fn deterministic(&self) -> bool {
        self.det
    }
}

/// Result of executing a DAG.
pub struct ExecReport {
    pub outputs: Vec<Vec<u8>>,
    pub output_cids: Vec<Cid>,
    pub lineage: Vec<Lineage>,
    pub wall_ms: f64,
    pub level_count: usize,
    pub max_parallelism: usize,
}

impl ExecReport {
    /// Verify every lineage link — a full-chain integrity check.
    pub fn provenance_intact(&self) -> bool {
        self.lineage.iter().all(|l| l.verify())
    }
}

/// Execute the DAG, running each level's tasks concurrently.
pub fn execute_parallel(dag: &TaskDag, ops: &[Box<dyn NodeOp>]) -> ExecReport {
    run(dag, ops, true)
}

/// Execute the DAG strictly serially (baseline / deterministic-replay path).
pub fn execute_serial(dag: &TaskDag, ops: &[Box<dyn NodeOp>]) -> ExecReport {
    run(dag, ops, false)
}

fn run(dag: &TaskDag, ops: &[Box<dyn NodeOp>], parallel: bool) -> ExecReport {
    let n = dag.len();
    assert_eq!(ops.len(), n, "one op per task required");
    let levels = dag.levels();
    let max_parallelism = levels.iter().map(|l| l.len()).max().unwrap_or(0);

    let mut outputs: Vec<Vec<u8>> = vec![Vec::new(); n];
    let mut lineage: Vec<Option<Lineage>> = (0..n).map(|_| None).collect();

    let start = Instant::now();
    for level in &levels {
        // Gather immutable inputs for each task in the level from prior outputs.
        let compute = |&t: &usize| -> (usize, Vec<u8>, Vec<Cid>) {
            let inputs: Vec<&[u8]> = dag.deps[t].iter().map(|p| outputs[*p].as_slice()).collect();
            let input_cids: Vec<Cid> = dag.deps[t].iter().map(|p| Cid::of_bytes(&outputs[*p])).collect();
            let out = ops[t].run(&inputs);
            (t, out, input_cids)
        };

        let results: Vec<(usize, Vec<u8>, Vec<Cid>)> = if parallel && level.len() > 1 {
            thread::scope(|s| {
                let handles: Vec<_> = level.iter().map(|t| s.spawn(|| compute(t))).collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            })
        } else {
            level.iter().map(compute).collect()
        };

        for (t, out, input_cids) in results {
            lineage[t] = Some(Lineage::new(t, &ops[t].op_name(), ops[t].deterministic(), input_cids, &out));
            outputs[t] = out;
        }
    }
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;

    let output_cids = outputs.iter().map(|o| Cid::of_bytes(o)).collect();
    let lineage = lineage.into_iter().map(|l| l.unwrap()).collect();
    ExecReport {
        outputs,
        output_cids,
        lineage,
        wall_ms,
        level_count: levels.len(),
        max_parallelism,
    }
}

/// Count deterministic vs probabilistic nodes in an op set.
pub fn determinism_split(ops: &[Box<dyn NodeOp>]) -> (usize, usize) {
    let det = ops.iter().filter(|o| o.deterministic()).count();
    (det, ops.len() - det)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(name: &str, det: bool) -> Box<dyn NodeOp> {
        let name2 = name.to_string();
        Box::new(FnOp {
            name: name.to_string(),
            det,
            f: move |inputs: &[&[u8]]| {
                let mut out = name2.as_bytes().to_vec();
                for i in inputs {
                    out.extend_from_slice(i);
                }
                out
            },
        })
    }

    fn abcd() -> TaskDag {
        let mut g = TaskDag::new();
        let a = g.add_task("A", 1.0);
        let b = g.add_task("B", 1.0);
        let c = g.add_task("C", 1.0);
        let d = g.add_task("D", 1.0);
        g.add_dep(a, c, 1.0);
        g.add_dep(b, d, 1.0);
        g
    }

    #[test]
    fn parallel_matches_serial_bitwise() {
        let g = abcd();
        let ops1: Vec<Box<dyn NodeOp>> = vec![op("A", true), op("B", true), op("C", true), op("D", true)];
        let ops2: Vec<Box<dyn NodeOp>> = vec![op("A", true), op("B", true), op("C", true), op("D", true)];
        let s = execute_serial(&g, &ops1);
        let p = execute_parallel(&g, &ops2);
        // Auto-parallelism must not change results: identical output CIDs.
        assert_eq!(s.output_cids, p.output_cids);
        assert_eq!(p.max_parallelism, 2);
        assert!(p.provenance_intact());
    }

    #[test]
    fn deterministic_replay_is_stable() {
        let g = abcd();
        let mk = || -> Vec<Box<dyn NodeOp>> {
            vec![op("A", true), op("B", true), op("C", true), op("D", true)]
        };
        let r1 = execute_parallel(&g, &mk());
        let r2 = execute_parallel(&g, &mk());
        assert_eq!(r1.output_cids, r2.output_cids);
    }

    #[test]
    fn provenance_chains_through_deps() {
        let g = abcd();
        let ops: Vec<Box<dyn NodeOp>> = vec![op("A", true), op("B", true), op("C", true), op("D", true)];
        let r = execute_serial(&g, &ops);
        // C consumed A's output, so C's lineage records A's output CID as input.
        let c_lin = &r.lineage[2];
        assert_eq!(c_lin.input_cids, vec![r.output_cids[0]]);
    }

    #[test]
    fn determinism_split_counts() {
        let ops: Vec<Box<dyn NodeOp>> = vec![op("A", true), op("B", false)];
        assert_eq!(determinism_split(&ops), (1, 1));
    }
}
