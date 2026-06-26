//! Time-travel and state replay.
//!
//! State is an append-only chain of commits. Every node ever committed lives in
//! a single content-addressed pool shared across all versions, so committing a
//! new revision that changes one node costs one node — not a full copy. This is
//! the structural-sharing dividend of content addressing, and it lets us
//! reconstruct the exact graph at any historical point with bit-level accuracy.

use crate::triple::TripleGraph;
use nexus_core::{Cid, Edge, GraphStore, Node};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Commit {
    pub version: Cid,
    pub parent: Option<Cid>,
    pub timestamp: String,
    pub message: String,
    pub members: Vec<Cid>,
    pub edges: Vec<Edge>,
}

/// An append-only, content-addressed version history.
#[derive(Default)]
pub struct VersionedGraph {
    /// Global dedup pool: every node from every version, stored once.
    pool: HashMap<Cid, Node>,
    commits: Vec<Commit>,
    /// Total node references across all commits (for the sharing report).
    total_member_refs: u64,
}

impl VersionedGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Commit a store snapshot at a logical timestamp (ISO-8601 recommended,
    /// since lexical order then matches chronological order).
    pub fn commit(&mut self, store: &GraphStore, timestamp: &str, message: &str) -> Cid {
        let mut members = Vec::with_capacity(store.len());
        for (cid, node) in store.iter() {
            self.pool.entry(*cid).or_insert_with(|| node.clone());
            members.push(*cid);
        }
        members.sort();
        self.total_member_refs += members.len() as u64;

        let parent = self.commits.last().map(|c| c.version);
        let version = compute_version(parent.as_ref(), &members, timestamp, message);
        self.commits.push(Commit {
            version,
            parent,
            timestamp: timestamp.to_string(),
            message: message.to_string(),
            members,
            edges: store.edges().to_vec(),
        });
        version
    }

    pub fn commits(&self) -> &[Commit] {
        &self.commits
    }

    /// Reconstruct the full graph as of a commit version.
    pub fn at(&self, version: &Cid) -> Option<GraphStore> {
        let commit = self.commits.iter().find(|c| &c.version == version)?;
        Some(self.reconstruct(commit))
    }

    /// Reconstruct the graph as it existed at `timestamp`: the latest commit
    /// whose timestamp is `<= timestamp`.
    pub fn as_of(&self, timestamp: &str) -> Option<GraphStore> {
        let commit = self
            .commits
            .iter()
            .filter(|c| c.timestamp.as_str() <= timestamp)
            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))?;
        Some(self.reconstruct(commit))
    }

    /// The triple-graph partition of a historical version.
    pub fn triple_at(&self, version: &Cid) -> Option<TripleGraph> {
        self.at(version).map(|s| TripleGraph::from_store(&s))
    }

    fn reconstruct(&self, commit: &Commit) -> GraphStore {
        let mut store = GraphStore::new();
        for cid in &commit.members {
            if let Some(node) = self.pool.get(cid) {
                store.insert(node.clone());
            }
        }
        for e in &commit.edges {
            store.add_edge(e.from, e.to, e.label.clone());
        }
        store
    }

    /// Distinct nodes physically stored, across all of history.
    pub fn pool_size(&self) -> usize {
        self.pool.len()
    }

    /// Cross-version structural-sharing ratio in `[0,1)`: fraction of node
    /// references avoided by storing each unique node once.
    pub fn sharing_ratio(&self) -> f64 {
        if self.total_member_refs == 0 {
            return 0.0;
        }
        1.0 - (self.pool.len() as f64 / self.total_member_refs as f64)
    }
}

fn compute_version(parent: Option<&Cid>, members: &[Cid], ts: &str, msg: &str) -> Cid {
    let mut bytes = Vec::new();
    if let Some(p) = parent {
        bytes.extend_from_slice(&p.0);
    }
    for m in members {
        bytes.extend_from_slice(&m.0);
    }
    bytes.extend_from_slice(ts.as_bytes());
    bytes.extend_from_slice(msg.as_bytes());
    Cid::of_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::*;

    fn store(version_fields: &[&str]) -> GraphStore {
        let mut s = GraphStore::new();
        let fields = version_fields
            .iter()
            .map(|n| Field { name: (*n).into(), ty: Ty::String, policy: None })
            .collect();
        s.insert(Node::Thing(Thing { name: "Doc".into(), implements: None, fields }));
        s
    }

    #[test]
    fn as_of_reconstructs_history() {
        let mut h = VersionedGraph::new();
        h.commit(&store(&["a"]), "2026-01-01T00:00:00Z", "v1");
        h.commit(&store(&["a", "b"]), "2026-02-01T00:00:00Z", "v2");
        h.commit(&store(&["a", "b", "c"]), "2026-03-01T00:00:00Z", "v3");

        // Query mid-history.
        let mid = h.as_of("2026-02-15T00:00:00Z").unwrap();
        if let Node::Thing(t) = mid.by_name("thing", "Doc").unwrap() {
            assert_eq!(t.fields.len(), 2);
        } else {
            panic!();
        }
        // Before any commit -> nothing.
        assert!(h.as_of("2025-01-01T00:00:00Z").is_none());
        // After all -> latest.
        let latest = h.as_of("2026-12-01T00:00:00Z").unwrap();
        if let Node::Thing(t) = latest.by_name("thing", "Doc").unwrap() {
            assert_eq!(t.fields.len(), 3);
        }
    }

    #[test]
    fn pool_shares_nodes_across_versions() {
        let mut h = VersionedGraph::new();
        // Same content committed many times -> pool stays at 1.
        for i in 0..10 {
            h.commit(&store(&["a"]), &format!("2026-01-{:02}T00:00:00Z", i + 1), "same");
        }
        assert_eq!(h.pool_size(), 1);
        assert!(h.sharing_ratio() > 0.89); // 1 stored / 10 refs
    }
}
