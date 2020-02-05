use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter, Result as FmtResult};

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub variety: String,
    pub content: String,
}

impl Debug for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}({})", self.variety, self.content)
    }
}
