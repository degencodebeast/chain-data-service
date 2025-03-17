use crate::models::Transaction;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
use crate::db::transaction; 

pub struct WorkerPool {
    workers: Vec<WorkerHandle>,
    sender: mpsc::Sender<Vec<Transaction>>,
}

struct WorkerHandle {
    id: usize,
    handle: tokio::task::JoinHandle<()>,
}

impl WorkerPool {
    pub fn new(state: Arc<AppState>, worker_count: usize) -> Self {
        let (sender, receiver) = mpsc::channel(1000);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        
        let mut workers = Vec::with_capacity(worker_count);
        
        for id in 0..worker_count {
            let worker = Worker::new(
                id,
                state.clone(),
                receiver.clone(),
            );
            
            let handle = tokio::spawn(async move {
                worker.run().await;
            });
            
            workers.push(WorkerHandle { id, handle });
        }

        Self {
            workers,
            sender,
        }
    }

    pub fn get_sender(&self) -> mpsc::Sender<Vec<Transaction>> {
        self.sender.clone()
    }
}

struct Worker {
    id: usize,
    state: Arc<AppState>,
    receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Vec<Transaction>>>>,
}

impl Worker {
    fn new(
        id: usize,
        state: Arc<AppState>,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<Vec<Transaction>>>>,
    ) -> Self {
        Self {
            id,
            state,
            receiver,
        }
    }

    async fn run(&self) {
        info!("Worker {} started", self.id);

        loop {
            let transactions = {
                let mut receiver = self.receiver.lock().await;
                match receiver.recv().await {
                    Some(txs) => txs,
                    None => break,
                }
            };

            if let Err(e) = self.process_batch(transactions).await {
                error!("Worker {} failed to process batch: {}", self.id, e);
            }
        }
    }

    async fn process_batch(&self, transactions: Vec<Transaction>) -> Result<(), sqlx::Error> {
        // First try to update cache
        {
            let cache = self.state.cache.lock().await;
            for tx in &transactions {
                let cache_key = format!("{}:{}:{}", 
                    tx.source_address, 
                    tx.block_time, 
                    tx.block_time + 3600 // 1 hour window for example
                );
                if let Some(mut cached_txs) = cache.get(&cache_key).await {
                    cached_txs.push(tx.clone());
                    cache.insert(cache_key, cached_txs).await;
                }
            }
        }

        // Use existing add_transactions function for database operations
        transaction::add_transactions(&self.state.db_pool, &transactions).await?;
        
        info!("Worker {} processed {} transactions", self.id, transactions.len());

        Ok(())
    }
}