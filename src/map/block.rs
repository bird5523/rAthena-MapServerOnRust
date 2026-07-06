use std::collections::HashSet;

pub const BLOCK_SIZE: i16 = 8;

pub struct Block {
    pub entities: HashSet<u32>, // IDs of entities in this block
}

impl Block {
    pub fn new() -> Self {
        Self {
            entities: HashSet::new(),
        }
    }

    pub fn add_entity(&mut self, id: u32) {
        self.entities.insert(id);
    }

    pub fn remove_entity(&mut self, id: u32) {
        self.entities.remove(&id);
    }
}
