use async_broadcast::{Receiver, Sender};
use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};

#[derive(Debug)]
pub struct MessageBoard<K, R> {
    inner: HashMap<K, Sender<R>>,
}

impl<K, R> Default for MessageBoard<K, R> {
    fn default() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

impl<K, R> MessageBoard<K, R>
where
    K: Eq + Hash,
    R: Clone,
{
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn listen(&mut self, k: K) -> Receiver<R> {
        match self.inner.entry(k) {
            Entry::Occupied(occupied) => occupied.get().new_receiver(),
            Entry::Vacant(vacant) => {
                let (s, r) = async_broadcast::broadcast(5);
                vacant.insert(s);
                r
            }
        }
    }

    pub async fn post(&mut self, k: &K, value: R) -> Result<(), R> {
        if let Some((_, sender)) = self.inner.remove_entry(k) {
            sender.broadcast(value).await.unwrap();
            Ok(())
        } else {
            Err(value)
        }
    }
}
