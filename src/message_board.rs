use async_std::sync::{channel, Receiver, Sender};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Default)]
pub struct MessageBoard<K, R> {
    inner: HashMap<K, Sender<R>>,
}

impl<K, R> MessageBoard<K, R>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn listen(&mut self, k: K) -> Receiver<R> {
        let (s, r) = channel::<R>(5);
        self.inner.insert(k, s);
        r
    }

    pub async fn post(&mut self, k: &K, value: R) -> Result<(), R> {
        if let Some((_, sender)) = self.inner.remove_entry(k) {
            sender.send(value).await;
            Ok(())
        } else {
            Err(value)
        }
    }
}
