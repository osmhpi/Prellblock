use super::Identifier;
use crate::AccountMeta;
use dialoguer::{theme::Theme, Input, MultiSelect, Select};
use hexutil::ToHex;
use prellblock_client_api::account::{Account, Permission, ReadingPermission, ReadingRight};
use std::cmp::Reverse;

pub(super) fn handle_set_reading_rights<'a>(
    theme: &'a dyn Theme,
    account: &mut Account,
    accounts: &[AccountMeta],
) {
    // TODO: option to remove a Black-/Whitelist
    let reading_rights_options = ["Add", "Show", "Done"];
    let mut reading_rights_select = Select::with_theme(theme);
    reading_rights_select
        .with_prompt("Actions for reading rights:\n(These are first fit)")
        .items(&reading_rights_options)
        .default(0);

    loop {
        match reading_rights_select.interact().unwrap() {
            0 => {
                handle_add_reading_right(theme, &mut account.reading_rights, accounts);
            }
            1 => handle_show_reading_rights(&account.reading_rights),
            2 => break,
            _ => panic!("Invalid selection"),
        }
    }
}

fn handle_show_reading_rights(reading_rights: &[ReadingPermission]) {
    print!("Reading rights:\n{:#?}", reading_rights);
}

fn handle_add_reading_right<'a>(
    theme: &'a dyn Theme,
    reading_rights: &mut Vec<ReadingPermission>,
    accounts: &[AccountMeta],
) {
    let add_reading_rights_options = ["Blacklist", "Whitelist"];
    let mut add_reading_rights_select = Select::with_theme(theme);
    add_reading_rights_select
        .items(&add_reading_rights_options)
        .default(1);
    match add_reading_rights_select.interact().unwrap() {
        0 => handle_add_list(
            theme,
            reading_rights,
            ReadingPermission::Blacklist(ReadingRight {
                accounts: vec![],
                namespace: vec![],
            }),
            accounts,
        ),
        1 => handle_add_list(
            theme,
            reading_rights,
            ReadingPermission::Whitelist(ReadingRight {
                accounts: vec![],
                namespace: vec![],
            }),
            accounts,
        ),
        _ => panic!("Invalid selection"),
    }
}

fn handle_add_list<'a>(
    theme: &'a dyn Theme,
    reading_rights: &mut Vec<ReadingPermission>,
    mut reading_permission: ReadingPermission,
    accounts: &[AccountMeta],
) {
    let mut reading_right = ReadingRight::default();

    let account_options = ["Show", "Add", "Remove", "Cancel"];
    let mut account_options_select = Select::with_theme(theme);
    account_options_select
        .with_prompt("Edit, add or remove a permission to the reading right:")
        .items(&account_options)
        .default(1);
    match account_options_select.interact().unwrap() {
        0 => handle_list_permission_item(&mut reading_right),
        1 => {
            handle_add_permission_item(theme, &mut reading_right, accounts);
            match reading_permission {
                ReadingPermission::Blacklist(ref mut permission_list)
                | ReadingPermission::Whitelist(ref mut permission_list) => {
                    *permission_list = reading_right;
                }
            }
            reading_rights.push(reading_permission);
        }
        2 => handle_remove_permission_item(theme, &mut reading_right),
        3 => {}
        _ => panic!("Invalid selection"),
    };
}

fn handle_remove_permission_item<'a>(theme: &'a dyn Theme, reading_right: &mut ReadingRight) {
    if reading_right.accounts.is_empty() {
        println!("No permission rules.");
        return;
    }

    let items: Vec<_> = reading_right
        .accounts
        .iter()
        .zip(reading_right.namespace.iter())
        .map(|(peer_id, namespace)| format!("{}-{}", peer_id, namespace.scope))
        .collect();
    let mut delete_rules_select = MultiSelect::with_theme(theme);
    delete_rules_select
        .with_prompt("Select permission rules to delete:")
        .items(&items);
    let mut permission_rules_to_delete = delete_rules_select.interact().unwrap();
    permission_rules_to_delete.sort_by_key(|&a| Reverse(a));
    let _: Vec<_> = permission_rules_to_delete
        .iter()
        .map(|i| {
            reading_right.accounts.swap_remove(*i);
            reading_right.namespace.swap_remove(*i)
        })
        .collect();
}

fn handle_list_permission_item(reading_right: &mut ReadingRight) {
    if reading_right.accounts.is_empty() {
        println!("No permission rules.");
        return;
    }

    reading_right
        .accounts
        .iter()
        .zip(reading_right.namespace.iter())
        .for_each(|(peer_id, namespace)| {
            println!("Account: {}\ntimeseries: {}", peer_id, namespace.scope)
        });
}

fn handle_add_permission_item<'a>(
    theme: &'a dyn Theme,
    reading_right: &mut ReadingRight,
    accounts: &[AccountMeta],
) {
    // make a copy of the reading right to restore it on cancelation
    let backup = reading_right.clone();

    // outer loop for adding `PeerId`s.
    loop {
        // get user input and try to parse it into a `PeerId`
        let fitting_accounts: Vec<(String, String)> = accounts
            .iter()
            .map(|account| {
                let peer_id = match &account.identifier {
                    Identifier::WithIdentity(identity) => identity.id(),
                    Identifier::WithPeerId(peer_id) => &peer_id,
                };
                (account.account.name.clone(), peer_id.to_hex())
            })
            .collect();
        if fitting_accounts.is_empty() {
            println!("No Account registered yet!");
            break;
        } else {
            let mut select_accounts = MultiSelect::with_theme(theme);
            let fitting_accounts_items: Vec<String> = fitting_accounts
                .iter()
                .map(|account| format!("{} ({})", account.0, account.1))
                .collect();
            select_accounts
                .items(&fitting_accounts_items)
                .with_prompt("Please select one or more accounts from the list below.");

            let selection = select_accounts.interact().unwrap();
            selection
                .iter()
                .map(|idx| match &accounts[*idx].identifier {
                    Identifier::WithIdentity(identity) => identity.id(),
                    Identifier::WithPeerId(peer_id) => &peer_id,
                })
                .for_each(|peer_id| reading_right.accounts.push(peer_id.clone()));
        }

        // state a dialog on how to go on
        let options = ["Undo", "Add a namespace", "Cancel"];
        let mut select = Select::with_theme(theme);
        select.items(&options).default(1);

        // outer loop for adding namespaces.
        loop {
            match select.interact().unwrap() {
                // undo `PeerId`
                0 => {
                    reading_right.accounts = backup.accounts.clone();
                    continue;
                }
                // add a namespace
                1 => {
                    let namespace = Input::<String>::new()
                        .with_prompt("Add a namespace")
                        .interact()
                        .unwrap();
                    reading_right
                        .namespace
                        .push(Permission { scope: namespace });
                }
                // total cancellation
                2 => {
                    reading_right.accounts.pop();
                    break;
                }
                _ => panic!("Invalid selection"),
            }

            // state a dialog on how to go on
            let options = ["Undo", "Add another namespace", "Done", "Cancel"];
            let mut select = Select::with_theme(theme);
            select.items(&options).default(2);

            match select.interact().unwrap() {
                0 => {
                    reading_right.namespace.pop();
                    continue;
                }
                1 => {
                    continue;
                }
                2 => {
                    return;
                }
                3 => {
                    *reading_right = backup;
                    return;
                }
                _ => panic!("Invalid selection"),
            }
        }
    }
}
