use balise::server::TlsIdentity;
use pinxit::Identity;
use prellblock::{
    batcher::Batcher,
    consensus::Consensus,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    permission_checker::PermissionChecker,
    turi::Turi,
    world_state::WorldState,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_prellblock() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    //// TEST-CONFIG
    let turi_address: SocketAddr = "127.0.0.1:2480".parse().unwrap();
    let peer_address: SocketAddr = "127.0.0.1:3131".parse().unwrap();

    let mut peers = HashMap::new();
    let peer_addresses = Vec::new();

    let identity = Identity::generate();
    peers.insert(identity.id().clone(), turi_address);

    let consensus = Consensus::new(identity, peers).await;

    let broadcaster = Broadcaster::new(peer_addresses);
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);

    let world_state = WorldState::default();
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

    let storage = DataStorage::new("../data/test-prellblock").unwrap();
    let storage = Arc::new(storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, storage, consensus, permission_checker);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    let peer_receiver_task = tokio::spawn(async move {
        let mut listener = TcpListener::bind(peer_address).await?;
        let receiver = Receiver::new(test_identity, peer_inbox);
        receiver.serve(&mut listener).await
    });

    // TODO: wait for all tasks -> in tests only wait that there is no error
    // in the first 5 seconds
    let _ = (turi_task, peer_receiver_task);
    // future::select_all(&[turi_task, peer_receiver_task, async {
    //     tokio::time::delay_for(Duration::from_secs(5)).await;
    //     Ok(())
    // }])
    // .await
    // .0
    // .unwrap();
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}
