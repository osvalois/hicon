// src/sharding/manager.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::Result;

#[derive(Debug, Clone)]
pub struct Shard {
    id: u64,
    node_ids: Vec<String>,
    data_range: (u64, u64),
}

pub struct ShardManager {
    shards: Arc<RwLock<HashMap<u64, Shard>>>,
    total_shards: u64,
    nodes: Arc<RwLock<Vec<String>>>,
}

impl ShardManager {
    pub fn new(total_shards: u64) -> Self {
        ShardManager {
            shards: Arc::new(RwLock::new(HashMap::new())),
            total_shards,
            nodes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn add_node(&self, node_id: String) -> Result<()> {
        let mut nodes = self.nodes.write().map_err(|_| "Failed to acquire write lock")?;
        nodes.push(node_id);
        self.rebalance_shards()?;
        Ok(())
    }

    pub fn remove_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().map_err(|_| "Failed to acquire write lock")?;
        nodes.retain(|id| id != node_id);
        self.rebalance_shards()?;
        Ok(())
    }

    pub fn get_shard_for_data(&self, data_id: u64) -> Result<Shard> {
        let shards = self.shards.read().map_err(|_| "Failed to acquire read lock")?;
        for shard in shards.values() {
            if data_id >= shard.data_range.0 && data_id < shard.data_range.1 {
                return Ok(shard.clone());
            }
        }
        Err("No shard found for the given data ID".into())
    }

    pub fn get_nodes_for_shard(&self, shard_id: u64) -> Result<Vec<String>> {
        let shards = self.shards.read().map_err(|_| "Failed to acquire read lock")?;
        shards.get(&shard_id)
            .map(|shard| shard.node_ids.clone())
            .ok_or_else(|| "Shard not found".into())
    }

    fn rebalance_shards(&self) -> Result<()> {
        let nodes = self.nodes.read().map_err(|_| "Failed to acquire read lock")?;
        let node_count = nodes.len();
        if node_count == 0 {
            return Ok(());
        }

        let mut shards = self.shards.write().map_err(|_| "Failed to acquire write lock")?;
        let shard_size = (u64::MAX / self.total_shards) + 1;

        for i in 0..self.total_shards {
            let shard = Shard {
                id: i,
                node_ids: nodes.iter()
                    .cycle()
                    .take(3.min(node_count))
                    .cloned()
                    .collect(),
                data_range: (i * shard_size, (i + 1) * shard_size),
            };
            shards.insert(i, shard);
        }

        Ok(())
    }

    pub fn get_all_shards(&self) -> Result<Vec<Shard>> {
        let shards = self.shards.read().map_err(|_| "Failed to acquire read lock")?;
        Ok(shards.values().cloned().collect())
    }

    pub fn update_shard(&self, shard: Shard) -> Result<()> {
        let mut shards = self.shards.write().map_err(|_| "Failed to acquire write lock")?;
        shards.insert(shard.id, shard);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_manager() {
        let manager = ShardManager::new(10);
        
        // Add nodes
        manager.add_node("node1".to_string()).unwrap();
        manager.add_node("node2".to_string()).unwrap();
        manager.add_node("node3".to_string()).unwrap();

        // Check shard assignment
        let shard = manager.get_shard_for_data(100).unwrap();
        assert_eq!(shard.node_ids.len(), 3);

        // Remove a node
        manager.remove_node("node2").unwrap();

        // Check shard reassignment
        let shard = manager.get_shard_for_data(100).unwrap();
        assert_eq!(shard.node_ids.len(), 2);

        // Get all shards
        let all_shards = manager.get_all_shards().unwrap();
        assert_eq!(all_shards.len(), 10);
    }
}