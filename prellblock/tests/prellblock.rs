use balise::{server::TlsIdentity, Address};
use futures::{select, FutureExt};
use im::Vector;
use pinxit::Identity;
use prellblock::{
    batcher::Batcher,
    block_storage::BlockStorage,
    consensus::Consensus,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    reader::Reader,
    transaction_checker::TransactionChecker,
    turi::Turi,
    world_state::WorldStateService,
};
use prellblock_client_api::consensus::GenesisTransactions;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_prellblock() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    //// TEST-CONFIG
    let turi_address: Address = "127.0.0.1:2480".parse().unwrap();
    let peer_address: Address = "127.0.0.1:3131".parse().unwrap();

    let mut peers = Vector::new();

    let identity = Identity::generate();
    peers.push_back((identity.id().clone(), peer_address.clone()));

    let fake_genesis = GenesisTransactions {
        transactions: vec![],
        timestamp: SystemTime::now(),
    };

    let block_storage = BlockStorage::new("../blocks/test-prellblock", Some(fake_genesis)).unwrap();
    let world_state = WorldStateService::default();
    {
        let mut world_state = world_state.get_writable().await;
        world_state.peers = peers;
        world_state.save();
    }

    let consensus = Consensus::new(identity, block_storage.clone(), world_state.clone()).await;

    let broadcaster = Broadcaster::new(world_state.clone());
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);

    let reader = Reader::new(block_storage, world_state.clone());

    let transaction_checker = TransactionChecker::new(world_state);

    let test_identity =
        TlsIdentity::from_pkcs12(include_bytes!("test-identity.pfx"), "prellblock").unwrap();

    // execute the turi in a new thread
    let turi_task = {
        let address = turi_address.clone();
        let transaction_checker = transaction_checker.clone();
        let test_identity = test_identity.clone();
        tokio::spawn(async move {
            let mut listener = TcpListener::bind(address.to_string().parse::<SocketAddr>().unwrap()).await?;
            let turi = Turi::new(test_identity, batcher, reader, transaction_checker);
            turi.serve(&mut listener).await
        })
    };

    let data_storage = DataStorage::new("../data/test-prellblock").unwrap();
    let data_storage = Arc::new(data_storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, data_storage, consensus, transaction_checker);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    let address = peer_address.clone();
    let peer_receiver_task = tokio::spawn(async move {
        let mut listener = TcpListener::bind(address.to_string().parse::<SocketAddr>().unwrap()).await?;
        let receiver = Receiver::new(test_identity, peer_inbox);
        receiver.serve(&mut listener).await
    });

    // wait for all tasks -> in tests only wait that there is no error
    // in the first 5 seconds
    select! {
        result = turi_task.fuse() => panic!("Turi ended: {:?}", result),
        result = peer_receiver_task.fuse() => panic!("Peer recceiver ended: {:?}", result),
        _ = tokio::time::sleep(Duration::from_secs(5)).fuse() => {
            // No error during startup
        },
    };
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}
