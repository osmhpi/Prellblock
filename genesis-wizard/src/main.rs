//! This tool (also known as "Fill Collins") helps to generate the **Genesis** Block
//! (initial configuration for the Prellblock blockchain).

use dialoguer::{
    theme::{ColorfulTheme, Theme},
    Confirm, Password, Select,
};
use hexutil::ToHex;
use newtype_enum::Enum;
use openssl::{
    pkcs12::Pkcs12,
    pkey::{PKey, Private},
    symm::Cipher,
    x509::X509,
};
use pinxit::{Identity, PeerId, Signable};
use prellblock::RpuPrivateConfig;
use prellblock_client_api::{
    account::{Account, AccountType, Permissions},
    consensus::GenesisTransactions,
    transaction, Transaction,
};
use std::{fs, path::{Path, PathBuf}, time::SystemTime};
use structopt::StructOpt;

mod accounts;
mod certificates;
mod util;
mod templates;

#[derive(Clone)]
enum Identifier {
    WithIdentity(Identity),
    WithPeerId(PeerId),
}

struct CA {
    cert: X509,
    pkey: PKey<Private>,
    created: bool,
}

struct AccountMeta {
    account: Account,
    identifier: Identifier,
    rpu_cert: Option<Pkcs12>,
}

impl AccountMeta {
    fn id(&self) -> &PeerId {
        match &self.identifier {
            Identifier::WithIdentity(identity) => identity.id(),
            Identifier::WithPeerId(id) => &id,
        }
    }
}

// https://crates.io/crates/structopt

#[derive(StructOpt, Debug)]
struct Opt {
    /// The path to a configuration template file.
    #[structopt(short, long, parse(from_os_str))]
    template: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();

    // All the variables that are used for writing later.
    let mut accounts: Vec<AccountMeta> = Vec::new();
    let mut ca = None;

    if let Some(template) = opt.template {
        let yaml = fs::read_to_string(template).unwrap();
        let template: Vec<templates::AccountTemplate> = serde_yaml::from_str(&yaml).unwrap();

        for a in template {
            accounts.push(AccountMeta{
                account: Account {
                    name: a.name,
                    account_type: a.permissions.account_type.unwrap(),
                    expire_at: a.permissions.expire_at.unwrap(),
                    writing_rights: a.permissions.has_writing_rights.unwrap(),
                    reading_rights: a.permissions.reading_rights.unwrap()
                },
                identifier: Identifier::WithIdentity(Identity::generate()),
                rpu_cert: None
            })
        }
    }

    let menu_theme = ColorfulTheme::default();
    let main_menu_items = [
        "Create ed25519 key (for signing genesis configuration)",
        "Manage accounts",
        "Manage TLS certificates",
        "Finish and generate configuration files",
        "Cancel",
    ];
    let main_menu_prompt = "This is Fill Collins, I will help you to setup Prellblock. \
    Please select a step below. \
    You need to execute all steps to be able to run Prellblock.";
    let mut main_menu = Select::with_theme(&menu_theme);
    main_menu
        .with_prompt(main_menu_prompt)
        .items(&main_menu_items)
        .default(0);
    loop {
        match main_menu.interact().unwrap() {
            0 => handle_generate_private_key(&menu_theme),
            1 => accounts::handle_create_accounts(&menu_theme, &mut accounts),
            2 => certificates::handle_create_certificates(&menu_theme, &mut accounts, &mut ca),
            3 => {
                if validate(&accounts) == false {
                    continue
                }
                handle_finish(&menu_theme, accounts, ca);
                break;
            }
            4 => {
                let cancel = Confirm::with_theme(&menu_theme)
                    .with_prompt("Do you really want to cancel? This will lose all settings.")
                    .show_default(true)
                    .default(false)
                    .interact()
                    .unwrap();
                if cancel {
                    break;
                }
            }
            _ => panic!("Invalid selection."),
        }
    }
}

fn handle_generate_private_key(theme: &'_ dyn Theme) {
    let key = Identity::generate();
    let path = util::handle_set_path(theme, "CA signing private key", "config/ca/");
    let path = format!("{}/ca.key", path);
    let path = Path::new(&path);
    fs::write(path, key.to_hex()).unwrap();
    println!(
        "Saved private key to {}.",
        path.canonicalize().unwrap().display()
    );
}

fn validate(accounts: &Vec<AccountMeta>) -> bool {
    let mut valid = true;
    for AccountMeta {
        account,
        identifier: _,
        rpu_cert,
    } in accounts
    {
        if let AccountType::RPU { .. } = account.account_type {
            if rpu_cert.is_none() {
                println!("Validation error: No certificate for {}.", account.name);
                valid = false;
            }
        }
    }

    let num_rpus = accounts.iter().filter(|&a| a.account.account_type.is_rpu()).count();
    if num_rpus < 4 {
        println!("Validation error: Your configuration must include at least four accounts of type RPU.");
        valid = false;
    }

    if !valid {
        println!("Please fix your configuration.");
    }

    return valid;
}

fn handle_finish(theme: &'_ dyn Theme, accounts: Vec<AccountMeta>, ca: Option<CA>) {
    let signing_identity = loop {
        let identity_data = Password::with_theme(theme)
            .with_prompt(
                "Please enter a ed25519 private key as genesis configuration signing identity:",
            )
            .interact()
            .unwrap();
        match identity_data.parse::<Identity>() {
            Ok(identity) => break identity,
            Err(err) => {
                println!("Could not parse genesis configuration private key: {}", err);
                continue;
            }
        }
    };
    let path = util::handle_set_path(
        theme,
        "account private and public keys for the accounts",
        "config",
    );
    let mut transactions = Vec::new();
    for AccountMeta {
        account,
        identifier,
        rpu_cert,
    } in accounts
    {
        let account_directory = format!("{}/{}", path, account.name);
        let private_key_path = format!("{}/{1}/{1}.key", path, account.name);

        let id = match identifier {
            Identifier::WithIdentity(identity) => {
                let peer_id = identity.id();
                let priv_key = identity.to_hex();
                if let Err(err) = fs::create_dir(account_directory.clone()) {
                    println!("Could not create directory {}: {}", account_directory, err);
                }
                fs::write(
                    format!("{}/{1}/{1}.pub", path, account.name),
                    peer_id.to_hex().clone(),
                )
                .unwrap();
                fs::write(private_key_path.clone(), priv_key).unwrap();
                peer_id.clone()
            }
            Identifier::WithPeerId(peer_id) => peer_id,
        };

        let name = account.name;
        if let AccountType::RPU { .. } = account.account_type {
            let mut pfx_path = "<path to .pfx file>".to_string();
            if let Some(rpu_cert) = rpu_cert {
                pfx_path = format!("{}/{}.pfx", account_directory, name);
                fs::write(pfx_path.clone(), &rpu_cert.to_der().unwrap()).unwrap();
            } else {
                println!("No certificate for {}, cannot set tls_id.", name);
            }

            let rpu_config = RpuPrivateConfig {
                identity: private_key_path,
                tls_id: pfx_path,
                block_path: format!("blocks/{}", name),
                data_path: format!("data/{}", name),
            };
            let rpu_config = toml::to_string(&rpu_config).unwrap();
            fs::write(format!("{}/{}.toml", account_directory, name), rpu_config).unwrap();
        }
        let br_account_transaction = Transaction::from_variant(transaction::CreateAccount {
            id,
            name,
            permissions: Permissions {
                account_type: Some(account.account_type),
                expire_at: Some(account.expire_at),
                has_writing_rights: Some(account.writing_rights),
                reading_rights: Some(account.reading_rights),
            },
            timestamp: SystemTime::now(),
        });
        transactions.push(br_account_transaction.sign(&signing_identity).unwrap());
    }

    // Write certificates
    if let Some(ca) = ca {
        if ca.created {
            let path = util::handle_set_path(theme, "CA certificate and private key", "config/ca");
            fs::write(
                format!("{}/ca-certificate.pem", path),
                &ca.cert.to_pem().unwrap(),
            )
            .unwrap();
            let password = Password::with_theme(theme)
                .with_prompt(
                    "Please enter a password for the CA private key (needed to reopen later):",
                )
                .with_confirmation("Please confirm the password", "Passwords mismatching")
                .interact()
                .unwrap();
            fs::write(
                format!("{}/ca-private-key.pem", path),
                &ca.pkey
                    .private_key_to_pem_pkcs8_passphrase(Cipher::aes_256_cbc(), password.as_bytes())
                    .unwrap(),
            )
            .unwrap();
        }
    }

    let genesis = GenesisTransactions {
        transactions,
        timestamp: SystemTime::now(),
    };
    let path = util::handle_set_path(theme, "genesis-configuration", "config/genesis");
    fs::write(
        format!("{}/genesis.yaml", path),
        serde_yaml::to_string(&genesis).unwrap(),
    )
    .unwrap();
}
