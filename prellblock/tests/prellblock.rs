use balise::server::TlsIdentity;
use futures::{select, FutureExt};
use pinxit::Identity;
use prellblock::{
    batcher::Batcher,
    block_storage::BlockStorage,
    consensus::Consensus,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    permission_checker::PermissionChecker,
    turi::Turi,
    world_state::WorldStateService,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_prellblock() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    //// TEST-CONFIG
    let turi_address: SocketAddr = "127.0.0.1:2480".parse().unwrap();
    let peer_address: SocketAddr = "127.0.0.1:3131".parse().unwrap();

    let mut peers = Vec::new();
    let peer_addresses = Vec::new();

    let identity = Identity::generate();
    peers.push((identity.id().clone(), turi_address));

    let block_storage = BlockStorage::new("../blocks/test-prellblock").unwrap();

    let consensus = Consensus::new(identity, peers, block_storage).await;

    let broadcaster = Broadcaster::new(peer_addresses);
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);

    let world_state = WorldStateService::default();
    let permission_checker = PermissionChecker::new(world_state);
    let permission_checker = Arc::new(permission_checker);

    let test_identity =
        TlsIdentity::from_pkcs12(include_bytes!("test-identity.pfx"), "prellblock").unwrap();

    // execute the turi in a new thread
    let turi_task = {
        let permission_checker = permission_checker.clone();
        let test_identity = test_identity.clone();
        tokio::spawn(async move {
            let mut listener = TcpListener::bind(turi_address).await?;
            let turi = Turi::new(test_identity, batcher, permission_checker);
            turi.serve(&mut listener).await
        })
    };

    let data_storage = DataStorage::new("../data/test-prellblock").unwrap();
    let data_storage = Arc::new(data_storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, data_storage, consensus, permission_checker);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    let peer_receiver_task = tokio::spawn(async move {
        let mut listener = TcpListener::bind(peer_address).await?;
        let receiver = Receiver::new(test_identity, peer_inbox);
        receiver.serve(&mut listener).await
    });

    // wait for all tasks -> in tests only wait that there is no error
    // in the first 5 seconds
    select! {
        result = turi_task.fuse() => panic!("Turi ended: {:?}", result),
        result = peer_receiver_task.fuse() => panic!("Peer recceiver ended: {:?}", result),
        _ = tokio::time::delay_for(Duration::from_secs(5)).fuse() => {
            // No error during startup
        },
    };
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}
