use crate::models::Transaction;
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info};

pub struct BatchConfig {
    pub max_batch_size: usize,
    pub flush_interval: Duration,
    pub worker_count: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            flush_interval: Duration::from_secs(5),
            worker_count: num_cpus::get(),
        }
    }
}

pub struct BatchManager {
    buffer: TransactionBuffer,
    config: BatchConfig,
    state: Arc<AppState>,
    worker_sender: mpsc::Sender<Vec<Transaction>>,
}

struct TransactionBuffer {
    pending: HashMap<String, Vec<Transaction>>, // Address -> Transactions
    total_size: usize,
    last_flush: Instant,
}

impl BatchManager {
    pub fn new(
        state: Arc<AppState>,
        config: BatchConfig,
        worker_sender: mpsc::Sender<Vec<Transaction>>,
    ) -> Self {
        Self {
            buffer: TransactionBuffer {
                pending: HashMap::new(),
                total_size: 0,
                last_flush: Instant::now(),
            },
            config,
            state,
            worker_sender,
        }
    }

    pub async fn add_transaction(&mut self, transaction: Transaction) {
        let address = transaction.source_address.clone();
        
        self.buffer.pending
            .entry(address)
            .or_insert_with(Vec::new)
            .push(transaction);
        
        self.buffer.total_size += 1;

        if self.should_flush() {
            self.flush().await;
        }
    }

    fn should_flush(&self) -> bool {
        self.buffer.total_size >= self.config.max_batch_size
            || self.buffer.last_flush.elapsed() >= self.config.flush_interval
    }

    async fn flush(&mut self) {
        if self.buffer.total_size == 0 {
            return;
        }

        let mut transactions = Vec::with_capacity(self.buffer.total_size);
        
        // Drain buffer into transactions vec
        for (_, mut addr_txs) in self.buffer.pending.drain() {
            transactions.append(&mut addr_txs);
        }

        // Sort by block time for consistent ordering
        transactions.sort_by_key(|tx| tx.block_time);

        match self.worker_sender.send(transactions).await {
            Ok(_) => {
                debug!("Flushed {} transactions to workers", self.buffer.total_size);
                self.buffer.total_size = 0;
                self.buffer.last_flush = Instant::now();
            }
            Err(e) => {
                error!("Failed to send transactions to workers: {}", e);
            }
        }
    }

    pub async fn start(mut self) {
        let mut interval = interval(self.config.flush_interval);

        loop {
            interval.tick().await;
            self.flush().await;
        }
    }
}
