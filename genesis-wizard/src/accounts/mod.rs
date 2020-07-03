use super::Identifier;
use crate::AccountMeta;
use dialoguer::{theme::Theme, Input, MultiSelect, Select};
use hexutil::FromHex;
use pinxit::{Identity, PeerId};
use prellblock_client_api::account::{Account, AccountType, Expiry};
use std::{cmp::Reverse, time::SystemTime};

mod reading_rights;

pub(super) fn handle_create_accounts<'a>(theme: &'a dyn Theme, accounts: &mut Vec<AccountMeta>) {
    let create_account_menu = [
        "Create a new account",
        "Show accounts",
        "Delete accounts",
        "Finish",
    ];

    loop {
        let mut create_account_select = Select::with_theme(theme);
        create_account_select
            .with_prompt("Select an option:")
            .items(&create_account_menu)
            .default(0);
        match create_account_select.interact().unwrap() {
            0 => handle_create_account(theme, accounts),
            1 => handle_show_accounts(accounts),
            2 => handle_delete_accounts(theme, accounts),
            3 => break,
            _ => panic!("Invalid Selection."),
        }
    }
}

fn handle_delete_accounts<'a>(theme: &'a dyn Theme, accounts: &mut Vec<AccountMeta>) {
    if accounts.is_empty() {
        println!("No accounts.");
        return;
    }

    let account_names: Vec<String> = accounts
        .iter()
        .map(|meta| meta.account.name.clone())
        .collect();
    let mut delete_account_select = MultiSelect::with_theme(theme);
    delete_account_select
        .with_prompt("Select accounts to delete:")
        .items(&account_names);
    let mut accounts_to_delete = delete_account_select.interact().unwrap();
    accounts_to_delete.sort_by_key(|&a| Reverse(a));
    let _: Vec<_> = accounts_to_delete
        .iter()
        .map(|i| accounts.swap_remove(*i))
        .collect();
}

fn handle_show_accounts(accounts: &mut Vec<AccountMeta>) {
    if accounts.is_empty() {
        println!("No accounts.");
        return;
    }
    let accounts_with_peer_ids = accounts.iter().map(|meta| match &meta.identifier {
        Identifier::WithIdentity(identity) => (&meta.account, identity.id()),
        Identifier::WithPeerId(peer_id) => (&meta.account, peer_id),
    });
    for (account, peer_id) in accounts_with_peer_ids {
        println!("{:?} ({}):\n{:#?}", account.name, peer_id, account);
    }
}

fn handle_create_account<'a>(theme: &'a dyn Theme, accounts: &mut Vec<AccountMeta>) {
    let mut create_accounts_menu = Select::with_theme(theme);
    let mut identifier = Identifier::WithIdentity(Identity::generate());
    let mut account = Account::new("New Account".to_string());
    let create_accounts_items = [
        "Public Key (optional)",
        "Name",
        "Account-Type",
        "Expiry date",
        "Set writing rights",
        "Set reading rights",
        "Show account",
        "Finish",
        "Abort Mission",
    ];
    create_accounts_menu
        .items(&create_accounts_items)
        .default(1);
    loop {
        match create_accounts_menu.interact().unwrap() {
            0 => {
                identifier = handle_set_public_key(theme);
                create_accounts_menu.default(1);
            }
            1 => {
                handle_set_name(theme, &mut account);
                create_accounts_menu.default(2);
            }
            2 => {
                handle_set_account_type(theme, &mut account);
                create_accounts_menu.default(3);
            }
            3 => {
                handle_set_expiry_date(theme, &mut account);
                create_accounts_menu.default(4);
            }
            4 => {
                handle_set_writing_rights(theme, &mut account);
                create_accounts_menu.default(5);
            }
            5 => {
                reading_rights::handle_set_reading_rights(theme, &mut account, accounts);
                create_accounts_menu.default(6);
            }
            6 => {
                println!("{:#?}", account);
            }
            7 => {
                accounts.push(AccountMeta {
                    account,
                    identifier,
                    rpu_cert: None,
                });
                break;
            }
            8 => break,
            _ => panic!("Invalid selection."),
        }
    }
}

fn handle_set_public_key(theme: &'_ dyn Theme) -> Identifier {
    let public_key_hex = Input::<String>::with_theme(theme)
        .with_prompt("Please enter a public key:")
        .validate_with(|input: &str| -> Result<(), &str> {
            match PeerId::from_hex(input.as_bytes()) {
                Ok(_) => Ok(()),
                Err(_) => {
                    Err("Could not parse public key. Please provide a valid ed25519 key (as hex).")
                }
            }
        })
        .interact()
        .unwrap();
    Identifier::WithPeerId(PeerId::from_hex(public_key_hex.as_bytes()).unwrap())
}

fn handle_set_name<'a>(theme: &'a dyn Theme, account: &mut Account) {
    let name = Input::<String>::with_theme(theme)
        .with_prompt("Please enter the account's name:")
        .default(account.name.clone())
        .interact()
        .unwrap();
    account.name = name;
}

fn handle_set_account_type<'a>(theme: &'a dyn Theme, account: &mut Account) {
    let account_type_options = ["Normal", "Block-Reader", "RPU", "Admin"];
    let mut account_type_select = Select::with_theme(theme);
    account_type_select
        .with_prompt("Please select the Account-Type")
        .items(&account_type_options)
        .default(0);
    match account_type_select.interact().unwrap() {
        0 => account.account_type = AccountType::Normal,
        1 => account.account_type = AccountType::BlockReader,
        2 => handle_set_rpu_addresses(theme, account),
        3 => account.account_type = AccountType::Admin,
        _ => panic!("Invalid Selection."),
    }
}

fn handle_set_rpu_addresses<'a>(theme: &'a dyn Theme, account: &mut Account) {
    let turi_address = Input::<String>::with_theme(theme)
        .with_prompt("Please enter the RPU's Turi IPv4-Address:")
        .default("127.0.0.1:3130".to_string())
        .interact()
        .unwrap()
        .parse()
        .unwrap();
    let peer_address = Input::<String>::with_theme(theme)
        .with_prompt("Please enter the RPU's Peer IPv4-Address:")
        .default("127.0.0.1:2480".to_string())
        .interact()
        .unwrap()
        .parse()
        .unwrap();
    account.account_type = AccountType::RPU {
        turi_address,
        peer_address,
    };
}

fn handle_set_expiry_date<'a>(theme: &'a dyn Theme, account: &mut Account) {
    let expiry_options = ["Never", "At Date"];
    let mut expiry_select = Select::with_theme(theme);
    let default_option = match account.expire_at {
        Expiry::Never => 0,
        Expiry::AtDate(_) => 1,
    };
    expiry_select.items(&expiry_options).default(default_option);
    match expiry_select.interact().unwrap() {
        0 => account.expire_at = Expiry::Never,
        1 => {
            let mut expiry_date_input = Input::<String>::with_theme(theme);
            expiry_date_input.with_prompt(format!(
                "Please enter the expiry date for {} (RFC3339 and UTC):",
                account.name
            ));
            loop {
                let default = if let Expiry::AtDate(expiry) = account.expire_at {
                    humantime::format_rfc3339_millis(SystemTime::from(expiry)).to_string()
                } else {
                    "2020-05-04T13:37:00".to_string()
                };
                let expiry_date_string = expiry_date_input.default(default).interact().unwrap();
                match humantime::parse_rfc3339_weak(&expiry_date_string) {
                    Ok(expiration) => {
                        account.expire_at = Expiry::AtDate(expiration.into());
                        break;
                    }
                    Err(_) => {
                        expiry_date_input
                            .with_prompt(format!(
                            "Invalid Date! Please enter the expiry date for {} (RFC3339 and UTC):"
                            , account.name));
                    }
                }
            }
        }
        _ => panic!("Invalid selection"),
    }
}

fn handle_set_writing_rights<'a>(theme: &'a dyn Theme, account: &mut Account) {
    let writing_rights_options = ["Yes", "No"];
    let mut writing_rights_select = Select::with_theme(theme);
    writing_rights_select
        .items(&writing_rights_options)
        .default(1);
    match writing_rights_select.interact().unwrap() {
        0 => account.writing_rights = true,
        1 => account.writing_rights = false,
        _ => panic!("Invalid selection"),
    }
}
