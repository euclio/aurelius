#[derive(Debug)]
pub(crate) struct IdMap<T> {
    storage: Vec<Option<T>>,
}

impl<T> IdMap<T> {
    pub fn insert(&mut self, item: T) -> usize {
        if let Some(idx) = self.storage.iter().position(|i| i.is_none()) {
            self.storage[idx] = Some(item);
            idx
        } else {
            let idx = self.storage.len();
            self.storage.push(Some(item));
            idx
        }
    }

    pub fn remove(&mut self, idx: usize) -> Option<T> {
        if let Some(elt) = self.storage.get_mut(idx) {
            elt.take()
        } else {
            None
        }
    }

    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.storage.iter().flatten()
    }
}

impl<T> Default for IdMap<T> {
    fn default() -> Self {
        IdMap { storage: vec![] }
    }
}
