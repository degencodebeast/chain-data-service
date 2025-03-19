use crate::models::Transaction;
use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, debug};
use crate::db::transaction; 
use crate::blockchain::processor;
use std::collections::HashSet;
use crate::db::address;
use crate::blockchain::client::SolanaClient;

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
            let worker_state = state.clone();
            let worker_receiver = receiver.clone();
            
            let handle = tokio::spawn(async move {
                let worker = Worker::new(id, worker_state, worker_receiver);
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
    
    pub async fn start(&self, shutdown_token: CancellationToken) {
        info!("Worker pool running with {} workers", self.workers.len());
        
        // Wait for shutdown signal
        shutdown_token.cancelled().await;
        
        // When shutdown is triggered, dropping sender will cause all workers to exit
        info!("Worker pool received shutdown signal");
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
                    None => {
                        info!("Worker {} channel closed, shutting down", self.id);
                        break;
                    }
                }
            };

            if let Err(e) = self.process_batch(transactions).await {
                error!("Worker {} failed to process batch: {}", self.id, e);
            }
        }

        info!("Worker {} shutting down", self.id);
    }

    async fn process_batch(&self, transactions: Vec<Transaction>) -> Result<(), sqlx::Error> {
        if transactions.is_empty() {
            return Ok(());
        }
        
        debug!("Worker processing {} transactions", transactions.len());
        
        // Create tracked addresses set (needed for process_transaction_batch)
        let addresses = match address::get_all_tracked_addresses(&self.state.db_pool).await {
            Ok(addrs) => addrs,
            Err(e) => {
                error!("Failed to get tracked addresses: {}", e);
                return Ok(());
            }
        };
        let tracked_addresses: HashSet<_> = addresses.into_iter().collect();
        
        // Create client
        let client = SolanaClient::new(&self.state.config);
        
        // Extract signatures - this is necessary since process_transaction_batch works with signatures
        let signatures: Vec<String> = transactions.iter()
            .map(|tx| tx.signature.clone())
            .collect();
        
        // Now use process_transaction_batch
        match processor::process_transaction_batch(
            &client,
            &self.state.db_pool,
            &signatures,
            &tracked_addresses
        ).await {
            Ok(count) => debug!("Successfully processed {} transactions", count),
            Err(e) => error!("Error processing transaction batch: {}", e)
        }

        Ok(())
    }
}