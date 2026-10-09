//! Preserve accepted resource paths across asynchronous engine rejection.

use std::collections::HashMap;

pub(super) type ResourceKey = (u128, usize);
pub(super) type ResourcePaths = HashMap<ResourceKey, String>;

#[derive(Debug, Clone, Default)]
pub(super) struct ResourceEdits {
    originals: ResourcePaths,
}

impl ResourceEdits {
    pub(super) fn record(&mut self, key: ResourceKey, previous: String) {
        self.originals.entry(key).or_insert(previous);
    }

    /// Return rejected paths to restore without replacing newer selections.
    pub(super) fn settle(
        &mut self,
        submitted: &ResourcePaths,
        current: &ResourcePaths,
        accepted: bool,
    ) -> ResourcePaths {
        let mut restored = ResourcePaths::new();
        self.originals.retain(|key, original| {
            let Some(desired) = current.get(key) else {
                return false;
            };
            let Some(sent) = submitted.get(key) else {
                return true;
            };
            if accepted {
                if desired == sent {
                    return false;
                }
                // A newer selection is pending; its fallback is the path the
                // engine just accepted, not the path before that submission.
                original.clone_from(sent);
                return true;
            }
            if desired == sent {
                restored.insert(*key, original.clone());
                return false;
            }
            true
        });
        restored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(value: &str) -> ResourcePaths {
        HashMap::from([((5, 0), value.to_owned())])
    }

    #[test]
    fn rejection_restores_original_across_coalesced_edits() {
        let mut edits = ResourceEdits::default();
        edits.record((5, 0), "accepted".into());
        edits.record((5, 0), "invalid-first".into());
        assert!(
            edits
                .settle(&paths("invalid-first"), &paths("invalid-second"), false)
                .is_empty()
        );
        assert_eq!(
            edits.settle(&paths("invalid-second"), &paths("invalid-second"), false),
            paths("accepted")
        );
    }

    #[test]
    fn older_success_becomes_newer_edits_fallback() {
        let mut edits = ResourceEdits::default();
        edits.record((5, 0), "original".into());
        assert!(
            edits
                .settle(&paths("accepted-new"), &paths("invalid-newer"), true)
                .is_empty()
        );
        assert_eq!(
            edits.settle(&paths("invalid-newer"), &paths("invalid-newer"), false),
            paths("accepted-new")
        );
    }

    #[test]
    fn accepted_or_removed_resources_leave_no_rollback() {
        let mut edits = ResourceEdits::default();
        edits.record((5, 0), "original".into());
        assert!(edits.settle(&paths("new"), &paths("new"), true).is_empty());
        assert!(edits.settle(&paths("new"), &paths("new"), false).is_empty());
        edits.record((5, 0), "original".into());
        assert!(
            edits
                .settle(&paths("new"), &HashMap::new(), false)
                .is_empty()
        );
        assert!(edits.settle(&paths("new"), &paths("new"), false).is_empty());
    }
}
