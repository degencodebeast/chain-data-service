use crate::models::Transaction;
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

#[derive(Clone)]
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
    buffer: Arc<Mutex<TransactionBuffer>>,
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
            buffer: Arc::new(Mutex::new(TransactionBuffer {
                pending: HashMap::new(),
                total_size: 0,
                last_flush: Instant::now(),
            })),
            config,
            state,
            worker_sender,
        }
    }

    pub async fn add_transaction(&self, transaction: Transaction) {
        // First collect necessary info without holding the lock too long
        let should_flush;
        let transactions_to_flush;
        
        {
            // Scope for lock to ensure it's released promptly
            let mut buffer = self.buffer.lock().await;
            
            // Add transaction to buffer
            let address = transaction.source_address.clone();
            buffer.pending
                .entry(address)
                .or_insert_with(Vec::new)
                .push(transaction);
            
            buffer.total_size += 1;
            
            // Check if we should flush
            should_flush = buffer.total_size >= self.config.max_batch_size ||
                           buffer.last_flush.elapsed() >= self.config.flush_interval;
            
            // If we need to flush, extract the transactions
            if should_flush {
                // Get all transactions from buffer
                transactions_to_flush = self.collect_transactions_for_flush(&mut buffer);
            } else {
                transactions_to_flush = Vec::new();
            }
        }
        
        // Now flush outside the lock if needed
        if should_flush && !transactions_to_flush.is_empty() {
            debug!("Flushing {} transactions from add_transaction", transactions_to_flush.len());
            self.send_transactions_to_worker(transactions_to_flush).await;
        }
    }

    // Helper to collect transactions for flushing
    fn collect_transactions_for_flush(&self, buffer: &mut TransactionBuffer) -> Vec<Transaction> {
        let mut transactions = Vec::with_capacity(buffer.total_size);
        
        // Extract all transactions from the pending map
        for (_, txs) in buffer.pending.drain() {
            transactions.extend(txs);
        }
        
        // Reset the buffer state
        buffer.total_size = 0;
        buffer.last_flush = Instant::now();
        
        transactions
    }

    // Helper to send transactions to worker
    async fn send_transactions_to_worker(&self, transactions: Vec<Transaction>) {
        if transactions.is_empty() {
            return;
        }
        
        match self.worker_sender.send(transactions.clone()).await {
            Ok(_) => {
                debug!("Successfully sent {} transactions to worker pool", transactions.len());
            },
            Err(e) => {
                error!("Failed to send transactions to worker pool: {}", e);
            }
        }
    }

    pub async fn start(&self, shutdown: CancellationToken) {
        let mut interval_timer = interval(self.config.flush_interval);

        // Perform an initial check immediately on startup
        self.perform_flush_check().await;
        
        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    // Check and flush on every timer tick
                    self.perform_flush_check().await;
                }
                _ = shutdown.cancelled() => {
                    info!("BatchManager shutting down, performing final flush");
                    self.perform_flush_check().await;
                    break;
                }
            }
        }
    }
    
    // Helper method to check and flush if needed
    async fn perform_flush_check(&self) {
        // Extract transactions that need flushing
        let transactions_to_flush = {
            let mut buffer = self.buffer.lock().await;
            
            // Skip if nothing to flush
            if buffer.total_size == 0 {
                return;
            }
            
            // Check if it's time to flush
            let should_flush = buffer.total_size >= self.config.max_batch_size ||
                             buffer.last_flush.elapsed() >= self.config.flush_interval;
            
            // If it's time to flush, collect transactions
            if should_flush {
                let txs = self.collect_transactions_for_flush(&mut buffer);
                txs
            } else {
                Vec::new()
            }
        };
        
        // Send transactions to worker without holding the lock
        if !transactions_to_flush.is_empty() {
            debug!("Timer-based flush of {} transactions", transactions_to_flush.len());
            self.send_transactions_to_worker(transactions_to_flush).await;
        }
    }

    // Add a public method to force a flush
    pub async fn force_flush(&self) {
        let transactions_to_flush = {
            let mut buffer = self.buffer.lock().await;
            self.collect_transactions_for_flush(&mut buffer)
        };
        
        if !transactions_to_flush.is_empty() {
            self.send_transactions_to_worker(transactions_to_flush).await;
        }
    }
}
