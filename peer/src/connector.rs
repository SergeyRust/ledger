use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;
use queues::{IsQueue, Queue, queue};
use tokio::sync::Mutex;
use network::Data;
use tokio::sync::mpsc::{
    channel,
    Receiver as Rx,
    Sender as Tx
};
use async_trait::async_trait;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::mpsc::error::SendError;
use crate::miner::Miner;
use crate::receiver::Receiver;
use crate::sender::Sender;
use crate::storage::Storage;

/// structure for connecting modules together
#[derive(Debug)]
pub struct Connector {
    pub(crate) receiver_rx: Arc<Mutex<Option<Rx<Data>>>>,
    sender_tx: Arc<Mutex<Option<Tx<Data>>>>,
    pub(crate) miner_rx: Arc<Mutex<Option<Rx<Data>>>>,
    pub(crate) miner_tx: Arc<Mutex<Option<Tx<Data>>>>,
    storage_tx: Arc<Mutex<Option<Tx<Data>>>>,
}

impl Connector {

    pub fn new() -> Self {
        Self {
            receiver_rx: Arc::new(Mutex::new(None)),
            sender_tx: Arc::new(Mutex::new(None)),
            miner_rx: Arc::new(Mutex::new(None)),
            miner_tx: Arc::new(Mutex::new(None)),
            storage_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) {
        let receiver_rx = self.receiver_rx.clone();
        let sender_tx = self.sender_tx.clone();
        let miner_rx = self.miner_rx.clone();
        let miner_tx = self.miner_tx.clone();
        let storage_tx = self.storage_tx.clone();
        tokio::spawn(async {
            Self::process_incoming(
                receiver_rx, sender_tx, miner_rx, miner_tx, storage_tx).await;
        });

    }

    async fn process_incoming(
        receiver_rx: Arc<Mutex<Option<Rx<Data>>>>,
        sender_tx: Arc<Mutex<Option<Tx<Data>>>>,
        miner_rx: Arc<Mutex<Option<Rx<Data>>>>,
        miner_tx: Arc<Mutex<Option<Tx<Data>>>>,
        storage_tx: Arc<Mutex<Option<Tx<Data>>>>,
    )
    {
        tokio::spawn(async move {
            loop {
                let mut miner_rx = miner_rx.lock().await;
                let miner_rx = miner_rx.as_mut();
                if let Some(miner_rx) = miner_rx {
                    while let Some(data) = miner_rx.recv().await {
                        println!("get data from receiver: {}", &data);
                        match data.data_type() {
                            1 => {
                                let storage_tx = storage_tx.clone();
                                Self::send_data(storage_tx, data).await;
                            }
                            _ => { println!("received wrong data type: {}", data) }
                        }
                    }
                }
            }
        });
        loop {
            let mut receiver_rx = receiver_rx.lock().await;
            let receiver_rx = receiver_rx.as_mut();
            if let Some(receiver_rx) = receiver_rx {
                while let Some(data) = receiver_rx.recv().await {
                    println!("get data from receiver: {}", &data);
                    match data.data_type() {
                        1 | 2 => {
                            let miner_tx = miner_tx.clone();
                            Self::send_data(miner_tx, data).await
                        }
                        3 | 4 => {
                            let sender_tx = sender_tx.clone();
                            Self::send_data(sender_tx, data).await;
                        }
                        _ => { unreachable!() }
                    }
                }
            }
            println!("listen_incoming() - there is no receiver") // unreachable ?
        };
    }

    // TODO retry
    async fn send_data(tx: Arc<Mutex<Option<Tx<Data>>>>, data: Data) {
        let mut tx = tx.lock().await;
        let tx = tx.as_mut();
        if let Some(tx) = tx {
            if let Err(e) = tx.send(data).await {
                println!("send_data() error: {}", e)
            } else {
                println!("data sent successfully")
            }
        }
    }
}

#[async_trait]
pub trait Connect {
    async fn connect(&mut self, connector: Arc<Mutex<Connector>>);
}

#[async_trait]
impl Connect for Sender {
    async fn connect(&mut self, connector: Arc<Mutex<Connector>>) {
        let mut connector = connector.lock().await;
        let (tx, rx): (Tx<Data>, Rx<Data>) = channel(10);
        self.connector_rx = Some(rx);
        connector.sender_tx = Arc::new(Mutex::new(Some(tx)));
    }
}

#[async_trait]
impl Connect for Storage {
    async fn connect(&mut self, connector: Arc<Mutex<Connector>>) {
        let mut connector = connector.lock().await;
        let (tx, rx): (Tx<Data>, Rx<Data>) = channel(10);
        self.connector_rx = Some(rx);
        connector.storage_tx = Arc::new(Mutex::new(Some(tx)));
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    use tokio::sync::mpsc::*;
    use tokio::sync::Mutex;
    use network::serialize_data;
    use state::Transaction;
    use utils::LOCAL_HOST;
    use client::Client;
    use crate::connector::{Connect, Connector};

    #[tokio::test]
    async fn test_channel() {
        let address = SocketAddr::from_str((String::from(LOCAL_HOST) + "1234").as_str()).unwrap();
        let mut receiver = crate::receiver::Receiver::new(address).await;
        let mut miner = crate::miner::Miner::new();
        let connector = Arc::new(Mutex::new(Connector::new()));
        let connector1 = connector.clone();
        let connector2 = connector.clone();
        receiver.connect(connector).await;
        tokio::spawn(async move { receiver.run().await });
        miner.connect(connector2.clone()).await; //tokio::spawn(async move {
        connector1.lock().await.start().await;
        send_transaction_to_receiver().await;

    }

    async fn send_transaction_to_receiver() {
        let transaction = create_account_transaction();
        let client = Client::new();
        client.send_transaction_to_network(serialize_data(&transaction)).await;
    }

    fn create_account_transaction() -> Transaction {
        Transaction {
            fee: 333,
            commands: vec![state::Command::CreateAccount {
                public_key: "12345".to_string(),
            }],
        }
    }
}
