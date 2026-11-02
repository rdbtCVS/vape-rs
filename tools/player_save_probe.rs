// Copy into the staged horizon-client/examples/ directory, then run with a fresh
// HORIZON_HOME: cargo run -p horizon-client --example player_save_probe
use horizon_client::{lan::playerdata, save};
use horizon_world::nbt::Tag;

fn main() {
    let root = std::path::PathBuf::from(std::env::var_os("HORIZON_HOME").expect("isolated home"));
    for generation in 0..2 {
        for uuid in 1..=150 {
            let file = playerdata::path(&root, uuid);
            playerdata::vape_player_saves::submit(file, Tag::Int(generation));
        }
        playerdata::vape_player_saves::flush().unwrap();
        for uuid in 1..=150 {
            let file = playerdata::path(&root, uuid);
            assert_eq!(save::read_dat(&file), Some(Tag::Int(generation)));
            if generation == 1 {
                assert_eq!(
                    save::read_dat(&file.with_extension("dat_old")),
                    Some(Tag::Int(0))
                );
            }
        }
    }
    // exists() must wait for the new snapshot, even without an explicit flush.
    playerdata::vape_player_saves::submit(playerdata::path(&root, 151), Tag::Int(42));
    assert!(playerdata::exists(&root, 151));
    assert_eq!(
        save::read_dat(&playerdata::path(&root, 151)),
        Some(Tag::Int(42))
    );
    let (tx, _rx) = std::sync::mpsc::channel();
    let joined = horizon_client::lan::Joined {
        conn: 1,
        uuid: 152,
        name: "SaveProbe".into(),
        properties: Vec::new(),
        info: horizon_protocol::packets::configuration::ClientInformation::new(2),
        brand: String::new(),
        sender: horizon_client::lan::PlayerSender::new(tx, Default::default()),
        latency: Default::default(),
        address: "127.0.0.1:0".parse().unwrap(),
    };
    let mut guest = horizon_client::lan::guest::Guest::new(joined, 1, [4.0, 5.0, 6.0], 2);
    playerdata::save(&guest, &root, None);
    guest.position = [9.0, 10.0, 11.0];
    playerdata::save(&guest, &root, None);
    guest.position = [0.0; 3];
    assert!(playerdata::load(&mut guest, &root));
    assert_eq!(guest.position, [9.0, 10.0, 11.0]);
    println!(
        "150 player files: two generations, gzip NBT, backups, read barriers and guest save/load passed"
    );
}
