//! Layout persistence deliberately contains no hosts, tabs or SSH credentials.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

pub type TabId = u64;
pub type GroupId = u64;
pub const MIN_WIDTH: f32 = 280.;
pub const MIN_HEIGHT: f32 = 180.;
pub const DIVIDER: f32 = 5.;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Layout {
    Group {
        id: GroupId,
    },
    Split {
        axis: Axis,
        ratio: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
impl Default for Layout {
    fn default() -> Self {
        Self::Group { id: 1 }
    }
}
impl Layout {
    pub fn groups(&self) -> Vec<GroupId> {
        match self {
            Self::Group { id } => vec![*id],
            Self::Split { first, second, .. } => [first.groups(), second.groups()].concat(),
        }
    }
    pub fn minimum(&self) -> (f32, f32) {
        match self {
            Self::Group { .. } => (MIN_WIDTH, MIN_HEIGHT),
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let (aw, ah) = first.minimum();
                let (bw, bh) = second.minimum();
                match axis {
                    Axis::Horizontal => (aw + bw + DIVIDER, ah.max(bh)),
                    Axis::Vertical => (aw.max(bw), ah + bh + DIVIDER),
                }
            }
        }
    }
    pub fn at_mut(&mut self, path: &[bool]) -> Option<&mut Self> {
        if path.is_empty() {
            return Some(self);
        }
        match self {
            Self::Split { first, second, .. } => {
                if path[0] { second } else { first }.at_mut(&path[1..])
            }
            _ => None,
        }
    }
    fn replace_group(&mut self, id: GroupId, replacement: Layout) -> bool {
        match self {
            Self::Group { id: here } if *here == id => {
                *self = replacement;
                true
            }
            Self::Split { first, second, .. } => {
                first.replace_group(id, replacement.clone())
                    || second.replace_group(id, replacement)
            }
            _ => false,
        }
    }
    fn remove(&mut self, id: GroupId) -> bool {
        if let Self::Split { first, second, .. } = self {
            if matches!(**first, Self::Group { id: here } if here == id) {
                *self = *second.clone();
                return true;
            }
            if matches!(**second, Self::Group { id: here } if here == id) {
                *self = *first.clone();
                return true;
            }
            return first.remove(id) || second.remove(id);
        }
        false
    }
    fn neighbor(&self, id: GroupId) -> Option<GroupId> {
        match self {
            Self::Split { first, second, .. } => {
                if matches!(**first, Self::Group { id: here } if here == id) {
                    return second.groups().first().copied();
                }
                if matches!(**second, Self::Group { id: here } if here == id) {
                    return first.groups().last().copied();
                }
                first.neighbor(id).or_else(|| second.neighbor(id))
            }
            _ => None,
        }
    }
    fn valid(&self, depth: usize, ids: &mut HashSet<GroupId>) -> bool {
        if depth > 16 {
            return false;
        }
        match self {
            Self::Group { id } => *id > 0 && *id < u64::MAX && ids.insert(*id) && ids.len() <= 16,
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                ratio.is_finite()
                    && (0.05..=0.95).contains(ratio)
                    && first.valid(depth + 1, ids)
                    && second.valid(depth + 1, ids)
            }
        }
    }
    pub fn load(path: &Path) -> Self {
        use std::io::Read;
        let mut bytes = Vec::new();
        let layout = std::fs::File::open(path)
            .ok()
            .and_then(|f| f.take(65537).read_to_end(&mut bytes).ok())
            .filter(|_| bytes.len() <= 65536)
            .and_then(|_| serde_json::from_slice::<Self>(&bytes).ok());
        layout
            .filter(|l| l.valid(0, &mut HashSet::new()))
            .unwrap_or_default()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        std::fs::create_dir_all(
            path.parent()
                .ok_or_else(|| std::io::Error::other("layout directory"))?,
        )?;
        let temp = path.with_extension("json.tmp");
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.sync_all()?;
        std::fs::rename(temp, path)
    }
}
#[derive(Default, Debug)]
pub struct Group {
    pub tabs: Vec<TabId>,
    pub selected: Option<TabId>,
}
pub struct Workspace {
    pub layout: Layout,
    pub groups: BTreeMap<GroupId, Group>,
    pub active: GroupId,
    next_group: GroupId,
    next_tab: TabId,
}
impl Workspace {
    pub fn new(layout: Layout) -> Self {
        let ids = layout.groups();
        Self {
            active: ids[0],
            next_group: ids.iter().max().unwrap() + 1,
            next_tab: 1,
            groups: ids.into_iter().map(|id| (id, Group::default())).collect(),
            layout,
        }
    }
    pub fn selected(&self) -> Option<TabId> {
        self.groups.get(&self.active)?.selected
    }
    pub fn add_tab(&mut self) -> TabId {
        let id = self.next_tab;
        self.next_tab += 1;
        let group = self.groups.get_mut(&self.active).unwrap();
        group.tabs.push(id);
        group.selected = Some(id);
        id
    }
    pub fn select(&mut self, group: GroupId, tab: TabId) {
        if let Some(g) = self.groups.get_mut(&group)
            && g.tabs.contains(&tab)
        {
            g.selected = Some(tab);
            self.active = group;
        }
    }
    pub fn close(&mut self, id: TabId) {
        for g in self.groups.values_mut() {
            if let Some(index) = g.tabs.iter().position(|t| *t == id) {
                g.tabs.remove(index);
                if g.selected == Some(id) {
                    g.selected = g
                        .tabs
                        .get(index.min(g.tabs.len().saturating_sub(1)))
                        .copied();
                }
                return;
            }
        }
    }
    pub fn move_tab(&mut self, id: TabId, destination: GroupId, before: Option<TabId>) {
        if !self.groups.contains_key(&destination)
            || !self.groups.values().any(|g| g.tabs.contains(&id))
            || before == Some(id)
        {
            return;
        }
        self.close(id);
        let group = self.groups.get_mut(&destination).unwrap();
        let index = before
            .and_then(|t| group.tabs.iter().position(|v| *v == t))
            .unwrap_or(group.tabs.len());
        group.tabs.insert(index, id);
        group.selected = Some(id);
        self.active = destination;
    }
    pub fn split(&mut self, group: GroupId, axis: Axis, width: f32, height: f32) -> bool {
        if !self.can_split(group, axis, width, height) {
            return false;
        }
        let id = self.next_group;
        self.next_group += 1;
        let new = Layout::Split {
            axis,
            ratio: 0.5,
            first: Box::new(Layout::Group { id: group }),
            second: Box::new(Layout::Group { id }),
        };
        if !self.layout.replace_group(group, new) {
            return false;
        }
        self.groups.insert(id, Group::default());
        self.active = id;
        true
    }
    pub fn can_split(&self, group: GroupId, axis: Axis, width: f32, height: f32) -> bool {
        self.groups.contains_key(&group)
            && self.groups.len() < 16
            && width >= MIN_WIDTH
            && height >= MIN_HEIGHT
            && match axis {
                Axis::Horizontal => width >= MIN_WIDTH * 2. + DIVIDER,
                Axis::Vertical => height >= MIN_HEIGHT * 2. + DIVIDER,
            }
    }
    pub fn merge(&mut self, group: GroupId) -> bool {
        let Some(neighbor) = self.layout.neighbor(group) else {
            return false;
        };
        let source = self.groups.remove(&group).unwrap();
        let dest = self.groups.get_mut(&neighbor).unwrap();
        dest.tabs.extend(source.tabs);
        if self.active == group {
            self.active = neighbor;
            dest.selected = source.selected.or(dest.selected);
        } else if dest.selected.is_none() {
            dest.selected = source.selected;
        }
        self.layout.remove(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_split_move_close_merge_preserves_ids_and_selection() {
        let mut w = Workspace::new(Layout::default());
        let a = w.add_tab();
        let b = w.add_tab();
        assert!(w.split(1, Axis::Horizontal, 1200., 800.));
        let right = w.active;
        assert!(w.selected().is_none());
        assert!(w.split(right, Axis::Vertical, 600., 800.));
        let bottom = w.active;
        w.move_tab(a, bottom, None);
        assert_eq!(w.selected(), Some(a));
        assert_eq!(w.groups[&1].selected, Some(b));
        w.move_tab(b, bottom, Some(a));
        assert_eq!(w.groups[&bottom].tabs, vec![b, a]);
        w.close(b);
        assert_eq!(w.selected(), Some(a));
        assert!(w.merge(bottom));
        assert_eq!(w.active, right);
        assert_eq!(w.selected(), Some(a));
        assert!(w.merge(1));
        assert_eq!(w.selected(), Some(a));
        assert!(!w.merge(right));
        w.close(a);
        assert!(w.groups[&right].tabs.is_empty());
        assert!(!w.split(right, Axis::Horizontal, 500., 500.));
    }
    #[test]
    fn persistence_restores_only_empty_groups_and_rejects_invalid_files() {
        let path = std::env::temp_dir().join(format!("dgl-layout-{}.json", std::process::id()));
        let mut w = Workspace::new(Layout::default());
        w.add_tab();
        w.split(1, Axis::Vertical, 800., 800.);
        w.layout.save(&path).unwrap();
        let restored = Workspace::new(Layout::load(&path));
        assert_eq!(restored.layout, w.layout);
        assert!(
            restored
                .groups
                .values()
                .all(|g| g.tabs.is_empty() && g.selected.is_none())
        );
        for invalid in [
            "{}",
            "{\"type\":\"Group\",\"id\":0}",
            "{\"type\":\"Split\",\"axis\":\"Vertical\",\"ratio\":1,\"first\":{\"type\":\"Group\",\"id\":1},\"second\":{\"type\":\"Group\",\"id\":1}}",
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert_eq!(Layout::load(&path), Layout::default());
        }
        std::fs::remove_file(path).unwrap();
    }
}
