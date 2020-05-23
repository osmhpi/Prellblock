//! Provides a wrapper for using prellblock clients from C.

use super::Client;
use lazy_static::lazy_static;
// use prellblock_client_api::account_permissions::Permissions;
use std::{ffi::CStr, os::raw::c_char, sync::Mutex};
use tokio::runtime;

lazy_static! {
    static ref ASYNC_RUNTIME: Mutex<tokio::runtime::Runtime> = {
        let rt = runtime::Runtime::new().unwrap();
        rt.into()
    };
}

/// Create a new `Client` for sending requests to RPUs.
#[no_mangle]
pub extern "C" fn create_client_instance(
    address: *const c_char,
    identity_hex: *const c_char,
) -> *mut Client {
    pretty_env_logger::init();
    let address = unsafe {
        assert!(!address.is_null());
        CStr::from_ptr(address)
    };
    let address = address.to_str().unwrap().parse().unwrap();

    let identity_hex = unsafe {
        assert!(!identity_hex.is_null());
        CStr::from_ptr(identity_hex)
    };
    let identity = identity_hex.to_str().unwrap().parse().unwrap();

    Box::into_raw(Box::new(Client::new(address, identity)))
}

/// Free the memory for the `Client`.
#[no_mangle]
pub extern "C" fn destroy_client_instance(client: *mut Client) {
    if client.is_null() {
        return;
    }
    unsafe {
        Box::from_raw(client);
    }
}

/// Send a Key-Value transaction via the client.
#[no_mangle]
pub extern "C" fn send_key_value(client: *mut Client, key: *const c_char, value: *const c_char) {
    let client = unsafe {
        assert!(!client.is_null());
        &mut *client
    };

    let key = unsafe {
        assert!(!key.is_null());
        CStr::from_ptr(key)
    };
    let key = key.to_str().unwrap();

    let value = unsafe {
        assert!(!value.is_null());
        CStr::from_ptr(value)
    };
    let value = value.to_str().unwrap();

    let result = ASYNC_RUNTIME.lock().unwrap().block_on(async {
        client
            .send_key_value(key.to_string(), value.to_string())
            .await
    });

    // Execute the future, blocking the current thread until completion
    match result {
        Ok(()) => log::info!("Transaction sent successfully."),
        Err(err) => log::error!("Error: {}", err),
    }
}

// /// Update a account.
// #[no_mangle]
// pub extern "C" fn send_update_account(
//     client: *mut Client,
//     id_hex: *const c_char,
//     permissions: FfiPermissions,
// ) {
//     let client = unsafe {
//         assert!(!client.is_null());
//         &mut *client
//     };

//     let id_hex = unsafe {
//         assert!(!id_hex.is_null());
//         CStr::from_ptr(id_hex)
//     };
//     let account = id_hex.to_str().unwrap().parse().unwrap();

//     println!("{:?}", permissions);

//     let permissions = Permissions {
//         is_admin: Some(true),
//         is_rpu: None,
//         expire_at: None,
//         has_writing_rights: None,
//         reading_rights: None,
//     };

//     let result = ASYNC_RUNTIME
//         .lock()
//         .unwrap()
//         .block_on(async { client.update_account(account, permissions).await });

//     // Execute the future, blocking the current thread until completion
//     match result {
//         Ok(()) => log::info!("Transaction sent successfully."),
//         Err(err) => log::error!("Error: {}", err),
//     }
// }

// #[repr(C)]
// #[derive(Debug)]
// pub struct FfiPermissions {
//     pub is_admin: PermissionOption<bool>,
// }

// /// An alternative for Rust's Option.
// #[repr(C)]
// #[derive(Debug)]
// pub enum PermissionOption<T> {
//     None,
//     Some(T),
// }
