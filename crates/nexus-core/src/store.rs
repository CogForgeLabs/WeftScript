//! The content-addressed graph store.
//!
//! Inserting a node returns its CID. Because identity is content, inserting the
//! same logical node twice stores it once — structural deduplication is free
//! and automatic. The store also maintains a name index and typed edges so the
//! graph is queryable by humans, AI agents, and verification engines alike.

use crate::cid::Cid;
use crate::node::Node;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A typed directed edge between two content-addressed nodes.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: Cid,
    pub to: Cid,
    pub label: String,
}

/// An append-only, content-addressed store with automatic deduplication.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct GraphStore {
    nodes: HashMap<Cid, Node>,
    /// Insertion order, for deterministic iteration and stable views.
    order: Vec<Cid>,
    /// (kind, name) -> CID for fast symbolic lookup.
    name_index: HashMap<(String, String), Cid>,
    edges: Vec<Edge>,
    /// Total number of insert calls, including duplicates — lets us report the
    /// real deduplication ratio achieved by content addressing.
    insert_count: u64,
}

impl GraphStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a node, returning its CID. Idempotent under content equality.
    pub fn insert(&mut self, node: Node) -> Cid {
        self.insert_count += 1;
        let cid = node.cid();
        if !self.nodes.contains_key(&cid) {
            if let Some(name) = node.name() {
                self.name_index
                    .insert((node.kind().to_string(), name.to_string()), cid);
            }
            self.nodes.insert(cid, node);
            self.order.push(cid);
        }
        cid
    }

    pub fn add_edge(&mut self, from: Cid, to: Cid, label: impl Into<String>) {
        self.edges.push(Edge { from, to, label: label.into() });
    }

    pub fn get(&self, cid: &Cid) -> Option<&Node> {
        self.nodes.get(cid)
    }

    pub fn contains(&self, cid: &Cid) -> bool {
        self.nodes.contains_key(cid)
    }

    /// Resolve a node by its kind and declared name.
    pub fn by_name(&self, kind: &str, name: &str) -> Option<&Node> {
        let cid = self.name_index.get(&(kind.to_string(), name.to_string()))?;
        self.nodes.get(cid)
    }

    pub fn cid_of(&self, kind: &str, name: &str) -> Option<Cid> {
        self.name_index.get(&(kind.to_string(), name.to_string())).copied()
    }

    /// Number of distinct stored nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Iterate nodes in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&Cid, &Node)> {
        self.order.iter().map(move |c| (c, &self.nodes[c]))
    }

    /// All nodes of a given kind, in insertion order.
    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = (&'a Cid, &'a Node)> {
        self.iter().filter(move |(_, n)| n.kind() == kind)
    }

    /// How many insert calls were issued (including duplicates).
    pub fn insert_count(&self) -> u64 {
        self.insert_count
    }

    /// Deduplication ratio in `[0,1]`: fraction of inserts that were redundant.
    pub fn dedup_ratio(&self) -> f64 {
        if self.insert_count == 0 {
            return 0.0;
        }
        let unique = self.nodes.len() as f64;
        1.0 - (unique / self.insert_count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::*;
    use crate::value::Ty;

    fn thing(name: &str) -> Node {
        Node::Thing(Thing {
            name: name.into(),
            implements: None,
            fields: vec![Field { name: "id".into(), ty: Ty::Uuid, policy: None }],
        })
    }

    #[test]
    fn dedup_is_automatic() {
        let mut s = GraphStore::new();
        let a = s.insert(thing("Account"));
        let b = s.insert(thing("Account")); // identical content
        assert_eq!(a, b);
        assert_eq!(s.len(), 1);
        assert_eq!(s.insert_count(), 2);
        assert!((s.dedup_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn name_lookup() {
        let mut s = GraphStore::new();
        s.insert(thing("Customer"));
        assert!(s.by_name("thing", "Customer").is_some());
        assert!(s.by_name("thing", "Missing").is_none());
    }

    #[test]
    fn edges_recorded() {
        let mut s = GraphStore::new();
        let a = s.insert(thing("A"));
        let b = s.insert(thing("B"));
        s.add_edge(a, b, "references");
        assert_eq!(s.edges().len(), 1);
        assert_eq!(s.edges()[0].label, "references");
    }
}
