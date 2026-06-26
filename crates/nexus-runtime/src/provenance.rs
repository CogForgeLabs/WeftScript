//! Native provenance: every produced item carries a cryptographic lineage
//! chain back to its origins.
//!
//! Each task's output CID, together with its operation name and the CIDs of its
//! inputs, hashes into a `lineage` CID. Because the chain is content-addressed,
//! any tampering anywhere in an output's history changes its lineage CID, and
//! auditors can trace any result back to the exact inputs that produced it.

use nexus_core::Cid;

/// One link in a provenance chain: what produced a node's output.
#[derive(Clone, Debug, PartialEq)]
pub struct Lineage {
    pub task: usize,
    pub op: String,
    pub deterministic: bool,
    pub input_cids: Vec<Cid>,
    pub output_cid: Cid,
    /// Hash of (op, inputs, output) — the tamper-evident lineage identifier.
    pub lineage_cid: Cid,
}

impl Lineage {
    pub fn new(task: usize, op: &str, deterministic: bool, input_cids: Vec<Cid>, output: &[u8]) -> Lineage {
        let output_cid = Cid::of_bytes(output);
        let lineage_cid = compute(op, &input_cids, &output_cid);
        Lineage { task, op: op.to_string(), deterministic, input_cids, output_cid, lineage_cid }
    }

    /// Recompute and verify the lineage CID — detects tampering.
    pub fn verify(&self) -> bool {
        compute(&self.op, &self.input_cids, &self.output_cid) == self.lineage_cid
    }
}

fn compute(op: &str, inputs: &[Cid], output: &Cid) -> Cid {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(op.as_bytes());
    for c in inputs {
        bytes.extend_from_slice(&c.0);
    }
    bytes.extend_from_slice(&output.0);
    Cid::of_bytes(&bytes)
}

/// Trace an output's full ancestry by walking input CIDs back through a set of
/// lineage links (indexed by their output CID).
pub fn trace(target: &Cid, links: &[Lineage]) -> Vec<Cid> {
    let mut chain = Vec::new();
    let mut frontier = vec![*target];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(cid) = frontier.pop() {
        if !seen.insert(cid.to_hex()) {
            continue;
        }
        chain.push(cid);
        if let Some(link) = links.iter().find(|l| l.output_cid == cid) {
            for inp in &link.input_cids {
                frontier.push(*inp);
            }
        }
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lineage_verifies() {
        let l = Lineage::new(0, "settle", true, vec![Cid::of_bytes(b"in")], b"out");
        assert!(l.verify());
    }

    #[test]
    fn tampering_detected() {
        let mut l = Lineage::new(0, "settle", true, vec![Cid::of_bytes(b"in")], b"out");
        // Tamper with the recorded output CID.
        l.output_cid = Cid::of_bytes(b"forged");
        assert!(!l.verify());
    }

    #[test]
    fn traces_ancestry() {
        let in_cid = Cid::of_bytes(b"raw");
        let l1 = Lineage::new(0, "a", true, vec![in_cid], b"mid");
        let l2 = Lineage::new(1, "b", true, vec![l1.output_cid], b"final");
        let chain = trace(&l2.output_cid, &[l1.clone(), l2.clone()]);
        assert!(chain.contains(&l2.output_cid));
        assert!(chain.contains(&l1.output_cid));
        assert!(chain.contains(&in_cid));
    }
}
