use super::Identifier;
use crate::AccountMeta;
use dialoguer::{theme::Theme, Input, MultiSelect, Select};
use hexutil::ToHex;
use prellblock_client_api::account::{Account, Permission, ReadingPermission, ReadingRight};

pub(super) fn handle_set_reading_rights<'a>(
    theme: &'a dyn Theme,
    account: &mut Account,
    accounts: &[AccountMeta],
) {
    // TODO: option to remove a Black-/Whitelist
    let reading_rights_options = ["Add", "Show", "Done"];
    let mut reading_rights_select = Select::with_theme(theme);
    reading_rights_select
        .with_prompt("Actions for reading rights (these are first fit):\n")
        .items(&reading_rights_options)
        .default(0);

    loop {
        match reading_rights_select.interact().unwrap() {
            0 => handle_add_reading_right(theme, &mut account.reading_rights, accounts),
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
        .with_prompt("Select whether the right shall be allowed (Whitelist) or denied (Blacklist):")
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

    let account_options = [
        "Select permitted accounts",
        "Add namespaces",
        "Show",
        "Done",
        "Cancel",
    ];
    let mut account_options_select = Select::with_theme(theme);
    let permission_type = match &reading_permission {
        ReadingPermission::Blacklist(_) => "Blacklist",
        ReadingPermission::Whitelist(_) => "Whitelist",
    };
    account_options_select
        .with_prompt(format!("Edit the permissions of the {}:", permission_type))
        .items(&account_options)
        .default(0);
    loop {
        match account_options_select.interact().unwrap() {
            0 => {
                handle_select_permitted_accounts(theme, &mut reading_right, accounts);
                account_options_select.default(1);
            }
            1 => {
                handle_select_permitted_namespaces(theme, &mut reading_right);
                account_options_select.default(3);
            }
            2 => {
                handle_list_permission_item(&mut reading_right);
                account_options_select.default(3);
            }
            3 => {
                match reading_permission {
                    ReadingPermission::Blacklist(ref mut permission_list)
                    | ReadingPermission::Whitelist(ref mut permission_list) => {
                        *permission_list = reading_right;
                    }
                }
                reading_rights.push(reading_permission);
                break;
            }
            4 => break,
            _ => panic!("Invalid selection"),
        };
    }
}

// TODO: include this
// fn handle_remove_permission_item<'a>(theme: &'a dyn Theme, reading_right: &mut ReadingRight) {
//     if reading_right.accounts.is_empty() {
//         println!("No permission rules.");
//         return;
//     }

//     let items: Vec<_> = reading_right
//         .accounts
//         .iter()
//         .zip(reading_right.namespace.iter())
//         .map(|(peer_id, namespace)| format!("{}-{}", peer_id, namespace.scope))
//         .collect();
//     let mut delete_rules_select = MultiSelect::with_theme(theme);
//     delete_rules_select
//         .with_prompt("Select permission rules to delete:")
//         .items(&items);
//     let mut permission_rules_to_delete = delete_rules_select.interact().unwrap();
//     permission_rules_to_delete.sort_by_key(|&a| Reverse(a));
//     let _: Vec<_> = permission_rules_to_delete
//         .iter()
//         .map(|i| {
//             reading_right.accounts.swap_remove(*i);
//             reading_right.namespace.swap_remove(*i)
//         })
//         .collect();
// }

fn handle_list_permission_item(reading_right: &mut ReadingRight) {
    if reading_right.accounts.is_empty() {
        println!("No permission rules.");
        return;
    }

    println!("Accounts:");
    reading_right
        .accounts
        .iter()
        .for_each(|account| println!("  - {}", account));
    println!("Timeseries:");
    reading_right
        .namespace
        .iter()
        .for_each(|namespace| println!("  - {}", namespace.scope));
}

fn handle_select_permitted_accounts<'a>(
    theme: &'a dyn Theme,
    reading_right: &mut ReadingRight,
    accounts: &[AccountMeta],
) {
    // get user input and try to parse it into a `PeerId`
    let account_items: Vec<(String, String, bool)> = accounts
        .iter()
        .map(|meta| {
            (
                meta.account.name.clone(),
                meta.id().to_hex(),
                reading_right.accounts.contains(meta.id()),
            )
        })
        .collect();
    if account_items.is_empty() {
        println!("No Account registered yet!");
    } else {
        let mut select_accounts = MultiSelect::with_theme(theme);
        let account_items: Vec<_> = account_items
            .iter()
            .map(|account| (format!("{} ({})", account.0, account.1), account.2))
            .collect();
        select_accounts
            .items_checked(&account_items)
            .with_prompt("Please select one or more accounts from the list below.");

        let selection = select_accounts.interact().unwrap();
        selection
            .into_iter()
            .map(|idx| match &accounts[idx].identifier {
                Identifier::WithIdentity(identity) => identity.id(),
                Identifier::WithPeerId(peer_id) => &peer_id,
            })
            .for_each(|peer_id| reading_right.accounts.push(peer_id.clone()));
    }
}

fn handle_select_permitted_namespaces<'a>(theme: &'a dyn Theme, reading_right: &mut ReadingRight) {
    let backup = reading_right.clone();
    // state a dialog on how to go on
    let options = ["Add a namespace", "Done", "Cancel"];
    let mut select = Select::with_theme(theme);
    select.items(&options).default(0);
    loop {
        match select.interact().unwrap() {
            0 => {
                let name = Input::<String>::new()
                    .with_prompt("Enter name")
                    .interact()
                    .unwrap();
                reading_right.namespace.push(Permission { scope: name });
            }
            1 => {
                return;
            }
            2 => {
                *reading_right = backup;
                return;
            }
            _ => panic!("Invalid selection"),
        }
    }
}
