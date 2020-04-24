use balise::server::TlsIdentity;
use pinxit::Identity;
use prellblock::{
    batcher::Batcher,
    block_storage::BlockStorage,
    consensus::Consensus,
    data_broadcaster::Broadcaster,
    data_storage::DataStorage,
    peer::{Calculator, PeerInbox, Receiver},
    permission_checker::PermissionChecker,
    thread_group::ThreadGroup,
    turi::Turi,
    world_state::WorldState,
};
use std::{collections::HashMap, net::TcpListener, sync::Arc};

#[test]
fn test_prellblock() {
    pretty_env_logger::init();
    log::info!("Kitty =^.^=");

    //// TEST-CONFIG
    let turi_address = "127.0.0.1:2480";
    let peer_address = "127.0.0.1:3131";

    // load and parse config
    let mut thread_group = ThreadGroup::new();

    let mut peers = HashMap::new();
    let peer_addresses = Vec::new();

    let identity = Identity::generate();
    peers.insert(identity.id().clone(), turi_address.parse().unwrap());
    // skype_chat.insert(Felix, MArtin)
    // waitinbg for lock on mutex mutx?
    let block_storage = BlockStorage::new("../blocks/test-prellblock").unwrap();

    let consensus = Consensus::new(identity, peers, block_storage);

    let broadcaster = Broadcaster::new(peer_addresses);
    let broadcaster = Arc::new(broadcaster);

    let batcher = Batcher::new(broadcaster);
    let batcher = Arc::new(batcher);

    let world_state = WorldState::default();
    let permission_checker = PermissionChecker::new(world_state);
    let permission_checker = Arc::new(permission_checker);

    let test_identity =
        TlsIdentity::from_pkcs12(include_bytes!("test-identity.pfx"), "prellblock").unwrap();

    // execute the turi in a new thread
    {
        let permission_checker = permission_checker.clone();
        let test_identity = test_identity.clone();
        thread_group.spawn(format!("Turi ({})", turi_address), move || {
            let listener = TcpListener::bind(turi_address)?;
            let turi = Turi::new(test_identity, batcher, permission_checker);
            turi.serve(&listener)
        });
    }

    let data_storage = DataStorage::new("../data/test-prellblock").unwrap();
    let data_storage = Arc::new(data_storage);

    let calculator = Calculator::new();
    let calculator = Arc::new(calculator.into());

    let peer_inbox = PeerInbox::new(calculator, data_storage, consensus, permission_checker);
    let peer_inbox = Arc::new(peer_inbox);

    // execute the receiver in a new thread
    thread_group.spawn(format!("Peer Receiver ({})", peer_address), move || {
        let listener = TcpListener::bind(peer_address)?;
        let receiver = Receiver::new(test_identity, peer_inbox);
        receiver.serve(&listener)
    });

    // wait for all threads -> not gonna do that in the test
    // thread_group.join_and_log();
    log::info!("Going to hunt some mice. I meant *NICE*. Bye.");
}
