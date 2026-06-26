//! Semantic Git: version control over the graph, not over text.
//!
//! Changes are recorded as structured transformations of vertices and edges
//! (e.g. *Add field customer_id to Thing Order*) rather than line insertions and
//! deletions. Because every patch is a typed graph operation, merges are
//! resolved structurally and never produce textual conflicts or broken builds —
//! two edits collide only if they touch the *same* structural target.

use nexus_core::{Field, GraphStore, Node, Thing, Ty};
use std::collections::BTreeMap;

/// A structured, semantically-meaningful change to the graph.
#[derive(Clone, PartialEq, Debug)]
pub enum SemanticPatch {
    AddNode { kind: String, name: String, node: Node },
    RemoveNode { kind: String, name: String },
    /// A non-Thing node changed wholesale.
    ReplaceNode { kind: String, name: String, node: Node },
    AddField { thing: String, field: Field },
    RemoveField { thing: String, field: String },
    ChangeFieldType { thing: String, field: String, from: Ty, to: Ty },
    SetFieldPolicy { thing: String, field: String, policy: Option<String> },
}

impl SemanticPatch {
    /// The structural target a patch touches. Two patches conflict only if they
    /// share a target — this is what makes merges deterministic.
    pub fn target(&self) -> String {
        match self {
            SemanticPatch::AddNode { kind, name, .. }
            | SemanticPatch::RemoveNode { kind, name }
            | SemanticPatch::ReplaceNode { kind, name, .. } => format!("{kind}/{name}"),
            SemanticPatch::AddField { thing, field } => format!("thing/{thing}#{}", field.name),
            SemanticPatch::RemoveField { thing, field }
            | SemanticPatch::ChangeFieldType { thing, field, .. }
            | SemanticPatch::SetFieldPolicy { thing, field, .. } => {
                format!("thing/{thing}#{field}")
            }
        }
    }

    /// A human/AI-readable description, the unit of a semantic changelog.
    pub fn describe(&self) -> String {
        match self {
            SemanticPatch::AddNode { kind, name, .. } => format!("Add {kind} {name}"),
            SemanticPatch::RemoveNode { kind, name } => format!("Remove {kind} {name}"),
            SemanticPatch::ReplaceNode { kind, name, .. } => format!("Modify {kind} {name}"),
            SemanticPatch::AddField { thing, field } => {
                format!("Add field {} to Thing {thing}", field.name)
            }
            SemanticPatch::RemoveField { thing, field } => {
                format!("Remove field {field} from Thing {thing}")
            }
            SemanticPatch::ChangeFieldType { thing, field, from, to } => {
                format!("Change Thing {thing}.{field} type {from} -> {to}")
            }
            SemanticPatch::SetFieldPolicy { thing, field, policy } => match policy {
                Some(p) => format!("Govern Thing {thing}.{field} with policy {p}"),
                None => format!("Clear policy on Thing {thing}.{field}"),
            },
        }
    }
}

/// Index a store's named nodes by `(kind, name)` for structural comparison.
fn index(store: &GraphStore) -> BTreeMap<(String, String), Node> {
    let mut m = BTreeMap::new();
    for (_, node) in store.iter() {
        if let Some(name) = node.name() {
            m.insert((node.kind().to_string(), name.to_string()), node.clone());
        }
    }
    m
}

/// Compute the semantic diff transforming `old` into `new`.
pub fn diff(old: &GraphStore, new: &GraphStore) -> Vec<SemanticPatch> {
    let a = index(old);
    let b = index(new);
    let mut patches = Vec::new();

    // Removals and modifications.
    for (key, old_node) in &a {
        match b.get(key) {
            None => patches.push(SemanticPatch::RemoveNode {
                kind: key.0.clone(),
                name: key.1.clone(),
            }),
            Some(new_node) if new_node != old_node => {
                if let (Node::Thing(ot), Node::Thing(nt)) = (old_node, new_node) {
                    patches.extend(diff_thing(ot, nt));
                } else {
                    patches.push(SemanticPatch::ReplaceNode {
                        kind: key.0.clone(),
                        name: key.1.clone(),
                        node: new_node.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    // Additions.
    for (key, new_node) in &b {
        if !a.contains_key(key) {
            patches.push(SemanticPatch::AddNode {
                kind: key.0.clone(),
                name: key.1.clone(),
                node: new_node.clone(),
            });
        }
    }
    patches
}

fn diff_thing(old: &Thing, new: &Thing) -> Vec<SemanticPatch> {
    let mut patches = Vec::new();
    let of: BTreeMap<_, _> = old.fields.iter().map(|f| (f.name.clone(), f)).collect();
    let nf: BTreeMap<_, _> = new.fields.iter().map(|f| (f.name.clone(), f)).collect();
    for (name, f) in &of {
        match nf.get(name) {
            None => patches.push(SemanticPatch::RemoveField {
                thing: old.name.clone(),
                field: name.clone(),
            }),
            Some(g) => {
                if f.ty != g.ty {
                    patches.push(SemanticPatch::ChangeFieldType {
                        thing: old.name.clone(),
                        field: name.clone(),
                        from: f.ty.clone(),
                        to: g.ty.clone(),
                    });
                }
                if f.policy != g.policy {
                    patches.push(SemanticPatch::SetFieldPolicy {
                        thing: old.name.clone(),
                        field: name.clone(),
                        policy: g.policy.clone(),
                    });
                }
            }
        }
    }
    for (name, g) in &nf {
        if !of.contains_key(name) {
            patches.push(SemanticPatch::AddField {
                thing: new.name.clone(),
                field: (*g).clone(),
            });
        }
    }
    patches
}

/// The outcome of a three-way merge.
#[derive(Debug)]
pub struct MergeResult {
    /// Patches that apply cleanly (union of non-conflicting changes).
    pub merged: Vec<SemanticPatch>,
    /// Targets touched incompatibly by both branches.
    pub conflicts: Vec<MergeConflict>,
}

#[derive(Debug)]
pub struct MergeConflict {
    pub target: String,
    pub ours: SemanticPatch,
    pub theirs: SemanticPatch,
}

/// Three-way merge of two branches against a common base. Non-overlapping
/// changes always combine; identical changes coalesce; only genuinely
/// divergent edits to the same target are reported as conflicts.
pub fn merge(base: &GraphStore, ours: &GraphStore, theirs: &GraphStore) -> MergeResult {
    let p_ours = diff(base, ours);
    let p_theirs = diff(base, theirs);
    let mut by_target: BTreeMap<String, &SemanticPatch> = BTreeMap::new();
    for p in &p_ours {
        by_target.insert(p.target(), p);
    }

    let mut merged = p_ours.clone();
    let mut conflicts = Vec::new();
    for tp in &p_theirs {
        match by_target.get(&tp.target()) {
            None => merged.push(tp.clone()),
            Some(op) if **op == *tp => { /* identical edit, already present */ }
            Some(op) => conflicts.push(MergeConflict {
                target: tp.target(),
                ours: (*op).clone(),
                theirs: tp.clone(),
            }),
        }
    }
    MergeResult { merged, conflicts }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(fields: Vec<Field>) -> GraphStore {
        let mut s = GraphStore::new();
        s.insert(Node::Thing(Thing { name: "Order".into(), implements: None, fields }));
        s
    }

    fn f(name: &str, ty: Ty) -> Field {
        Field { name: name.into(), ty, policy: None }
    }

    #[test]
    fn detects_field_addition() {
        let a = store_with(vec![f("id", Ty::Uuid)]);
        let b = store_with(vec![f("id", Ty::Uuid), f("customer_id", Ty::Uuid)]);
        let d = diff(&a, &b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].describe(), "Add field customer_id to Thing Order");
    }

    #[test]
    fn detects_type_change() {
        let a = store_with(vec![f("amount", Ty::Decimal)]);
        let b = store_with(vec![f("amount", Ty::String)]);
        let d = diff(&a, &b);
        assert!(matches!(d[0], SemanticPatch::ChangeFieldType { .. }));
    }

    #[test]
    fn non_overlapping_merges_clean() {
        let base = store_with(vec![f("id", Ty::Uuid)]);
        let ours = store_with(vec![f("id", Ty::Uuid), f("a", Ty::String)]);
        let theirs = store_with(vec![f("id", Ty::Uuid), f("b", Ty::String)]);
        let r = merge(&base, &ours, &theirs);
        assert!(r.conflicts.is_empty());
        assert_eq!(r.merged.len(), 2); // add a + add b
    }

    #[test]
    fn divergent_same_target_conflicts() {
        let base = store_with(vec![f("amount", Ty::Decimal)]);
        let ours = store_with(vec![f("amount", Ty::String)]);
        let theirs = store_with(vec![f("amount", Ty::Uuid)]);
        let r = merge(&base, &ours, &theirs);
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].target, "thing/Order#amount");
    }

    #[test]
    fn identical_edits_coalesce() {
        let base = store_with(vec![f("id", Ty::Uuid)]);
        let ours = store_with(vec![f("id", Ty::Uuid), f("x", Ty::String)]);
        let theirs = store_with(vec![f("id", Ty::Uuid), f("x", Ty::String)]);
        let r = merge(&base, &ours, &theirs);
        assert!(r.conflicts.is_empty());
        assert_eq!(r.merged.len(), 1);
    }
}
