use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, MutexGuard};
//use std::sync::Mutex;
use std::thread;

use queues::{Queue, queue};
use tokio::sync::Mutex;
use tracing::{event, Level, span};

use network::Data;

use crate::connector::{Connect, Connector};
use crate::miner::Miner;
use crate::receiver::Receiver;
use crate::sender::Sender;

const LOCAL_HOST: &str = "127.0.0.1:";

pub struct Node {}

impl Node {

    pub async fn start(local_port: &str) {
        let address = SocketAddr::from_str((String::from(LOCAL_HOST) + local_port).as_str()).unwrap();
        let receiver = Arc::new(Mutex::new(Receiver::new(address).await));
        let sender = Arc::new(Mutex::new(Sender::new(address)));
        let miner = Arc::new(Mutex::new(Miner::new(local_port.parse().unwrap()))); // TODO FIX

        let receiver1 = receiver.clone();
        let sender1 = sender.clone();
        let miner1 = miner.clone();
        let receiver2 = receiver.clone();
        let sender2 = sender.clone();
        let miner2 = miner.clone();

        let connector = Arc::new(Mutex::new(Connector::new()));
        let c1 = connector.clone();
        let c2 = connector.clone();
        let c3 = connector.clone();

        miner1.lock().await.connect(c1).await;
        sender1.lock().await.connect(c2).await;
        receiver1.lock().await.connect(c3).await;
        connector.lock().await.start().await;

        tokio::spawn(async move {
            let mut receiver = receiver2.lock().await;
            receiver.run().await
        });
        tokio::spawn(async move {
            let mut sender = sender2.lock().await;
            sender.run().await
        });
        tokio::spawn(async move {
            let miner = miner2.lock().await;
            miner.run().await;
        });

        event!(Level::INFO, "node started on 127.0.0.1:{}", local_port);
    }
}