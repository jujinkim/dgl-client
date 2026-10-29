//! Device-local ordering, independent of shared host metadata and connection completion.
use dgl_core::model::HostProfile;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Read, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostOrder {
    ids: Vec<String>,
    /// Before a manual drop, this prefix is in first-connection-start order.
    started: usize,
    manual: bool,
}

impl HostOrder {
    pub fn load(path: &Path, hosts: &[HostProfile]) -> Self {
        let mut bytes = Vec::new();
        let saved = std::fs::File::open(path)
            .ok()
            .and_then(|f| f.take(1_048_577).read_to_end(&mut bytes).ok())
            .filter(|_| bytes.len() <= 1_048_576)
            .and_then(|_| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|o| {
                o.started <= o.ids.len()
                    && o.ids.iter().all(|id| !id.is_empty())
                    && o.ids.iter().collect::<HashSet<_>>().len() == o.ids.len()
            });
        let mut order = saved.unwrap_or(Self {
            ids: Vec::new(),
            started: 0,
            manual: false,
        });
        order.reconcile(hosts);
        order
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn reconcile(&mut self, hosts: &[HostProfile]) -> bool {
        let old = self.clone();
        let live: HashSet<_> = hosts.iter().map(|h| &h.id).collect();
        self.started = self.ids[..self.started]
            .iter()
            .filter(|id| live.contains(id))
            .count();
        self.ids.retain(|id| live.contains(id));
        for host in hosts {
            if !self.ids.contains(&host.id) {
                self.ids.push(host.id.clone());
            }
        }
        *self != old
    }

    pub fn connection_started(&mut self, id: &str) -> bool {
        if self.manual {
            return false;
        }
        let Some(index) = self.ids.iter().position(|i| i == id) else {
            return false;
        };
        if index < self.started {
            return false;
        }
        let id = self.ids.remove(index);
        self.ids.insert(self.started, id);
        self.started += 1;
        true
    }

    /// Commit the full displayed order only on a valid drop. `None` means the end.
    pub fn drop_before(&mut self, id: &str, before: Option<&str>) -> bool {
        if before == Some(id)
            || !self.ids.iter().any(|i| i == id)
            || before.is_some_and(|b| !self.ids.iter().any(|i| i == b))
        {
            return false;
        }
        self.ids.retain(|i| i != id);
        let index = before
            .and_then(|b| self.ids.iter().position(|i| i == b))
            .unwrap_or(self.ids.len());
        self.ids.insert(index, id.into());
        self.manual = true;
        true
    }

    pub fn save(&self, path: &Path) -> dgl_core::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| dgl_core::Error::Storage)?;
        dgl_core::sync::save_private(path, &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hosts(ids: &[&str]) -> Vec<HostProfile> {
        ids.iter().map(|id| serde_json::from_value(serde_json::json!({"id":id,"alias":id,"hostname":"localhost","username":"test"})).unwrap()).collect()
    }
    #[test]
    fn starts_are_stable_and_manual_order_survives_metadata_reload_and_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("host-order.json");
        let mut list = hosts(&["c", "b", "a", "d"]);
        let mut order = HostOrder::load(&path, &list);
        assert!(order.connection_started("a"));
        assert!(order.connection_started("b"));
        for _ in 0..4 {
            assert!(!order.connection_started("b"));
            assert!(!order.connection_started("a"));
        }
        assert_eq!(order.ids(), ["a", "b", "c", "d"]);
        order.save(&path).unwrap();
        assert_eq!(HostOrder::load(&path, &list), order);
        assert!(!order.drop_before("a", Some("a")));
        assert!(!order.drop_before("b", Some("deleted")));
        assert!(order.drop_before("d", Some("a")));
        list.reverse();
        list[0].favorite = true;
        list[0].last_connected = i64::MAX;
        assert!(!order.reconcile(&list));
        assert!(!order.connection_started("c"));
        order.save(&path).unwrap();
        order = HostOrder::load(&path, &list);
        assert_eq!(order.ids(), ["d", "a", "b", "c"]);
        assert!(order.reconcile(&hosts(&["c", "a", "e", "d"])));
        assert_eq!(order.ids(), ["d", "a", "c", "e"]);
        assert!(!order.connection_started("e"));
        // The atomic private write leaves no temporary file behind.
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["host-order.json"]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
    #[test]
    fn invalid_files_fall_back_and_removed_prefix_ids_do_not_promote_unstarted_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("host-order.json");
        for invalid in [
            "{}",
            "broken",
            r#"{"ids":["a","a"],"started":0,"manual":false}"#,
            r#"{"ids":["a"],"started":2,"manual":false}"#,
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert_eq!(
                HostOrder::load(&path, &hosts(&["c", "b", "a"])).ids(),
                ["c", "b", "a"]
            );
        }
        std::fs::remove_file(&path).unwrap();
        let mut order = HostOrder::load(&path, &hosts(&["c", "b", "a"]));
        order.connection_started("a");
        order.reconcile(&hosts(&["c", "b"]));
        order.connection_started("b");
        assert_eq!(order.ids(), ["b", "c"]);
    }
}
