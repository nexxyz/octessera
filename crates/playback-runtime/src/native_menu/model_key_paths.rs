use std::cell::RefCell;

use super::model_search::find_item_path_by_key;
use super::NativeMenuItem;

const REMEMBERED_KEYS: usize = 8;

/// Where recently looked-up keys sit in the menu tree, so repeated edits of one row (an Aux
/// sweep, say) skip the full-tree search. A remembered path counts only while the row it leads
/// to still carries the key, so rebuilt or edited trees just cost one fresh search.
#[derive(Clone, Debug, Default)]
pub struct MenuKeyPaths {
    paths: RefCell<Vec<(String, Vec<usize>)>>,
    #[cfg(test)]
    tree_searches: std::cell::Cell<usize>,
}

impl PartialEq for MenuKeyPaths {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for MenuKeyPaths {}

impl MenuKeyPaths {
    pub(super) fn find<'a>(
        &self,
        root: &'a NativeMenuItem,
        key: &str,
    ) -> Option<&'a NativeMenuItem> {
        item_at(root, &self.path_for(root, key)?)
    }

    pub(super) fn find_mut<'a>(
        &self,
        root: &'a mut NativeMenuItem,
        key: &str,
    ) -> Option<&'a mut NativeMenuItem> {
        let path = self.path_for(root, key)?;
        path.iter()
            .try_fold(root, |item, index| item.children.get_mut(*index))
    }

    #[cfg(test)]
    pub(crate) fn tree_searches(&self) -> usize {
        self.tree_searches.get()
    }

    fn path_for(&self, root: &NativeMenuItem, key: &str) -> Option<Vec<usize>> {
        let mut paths = self.paths.borrow_mut();
        if let Some(index) = paths.iter().position(|(remembered, _)| remembered == key) {
            let entry = paths.remove(index);
            if item_at(root, &entry.1).is_some_and(|item| item.key.as_deref() == Some(key)) {
                let path = entry.1.clone();
                paths.push(entry);
                return Some(path);
            }
        }
        #[cfg(test)]
        self.tree_searches.set(self.tree_searches.get() + 1);
        let mut path = Vec::new();
        if !find_item_path_by_key(root, key, &mut path) {
            return None;
        }
        if paths.len() == REMEMBERED_KEYS {
            paths.remove(0);
        }
        paths.push((key.to_owned(), path.clone()));
        Some(path)
    }
}

fn item_at<'a>(root: &'a NativeMenuItem, path: &[usize]) -> Option<&'a NativeMenuItem> {
    path.iter()
        .try_fold(root, |item, index| item.children.get(*index))
}
