//! # nexus-planner
//!
//! Dependency-driven scheduling for Nexus. Builds task DAGs with native
//! auto-parallelism, schedules them onto heterogeneous processors with HEFT,
//! and routes within contract budgets using a multi-objective Pareto front.

pub mod dag;
pub mod plan;
pub mod schedule;

pub use dag::{TaskDag, TaskId};
pub use plan::from_workflow;
pub use schedule::{heft, pareto_front, route_within_budget, Processor, Schedule};
