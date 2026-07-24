use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct TestSuite {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<TestLimit>,
    pub cases: Vec<TestCase>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, serde_json::Value>,
}

impl TestSuite {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn merge(&mut self, other: Self) {
        self.cases.extend(other.cases);
        self.meta.extend(other.meta);
        if let Some(limit) = other.limit {
            self.merge_limit(limit);
        }
    }

    pub fn merge_limit(&mut self, limit: TestLimit) {
        let mut current = self.limit.unwrap_or_default();
        if limit.time.is_some() {
            current.time = limit.time;
        }
        if limit.memory.is_some() {
            current.memory = limit.memory;
        }
        self.limit = current.into_option();
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TestCase {
    pub input: String,
    pub answer: String,

    #[serde(default, skip_serializing)]
    pub id: u32,
}

#[derive(Default, Serialize, Deserialize, Clone, Copy)]
pub struct TestLimit {
    pub memory: Option<u32>,
    pub time: Option<u64>,
}

impl TestLimit {
    pub fn into_option(self) -> Option<Self> {
        (self.memory.is_some() || self.time.is_some()).then_some(self)
    }
}
