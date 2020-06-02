//! Provides a wrapper for using prellblock clients from C.

#![allow(clippy::module_name_repetitions)]

use crate::Client;
use lazy_static::lazy_static;
use std::{
    future::Future,
    os::raw::{c_char, c_int},
};
use tokio::runtime::Runtime;

#[macro_use]
mod error;
mod prelude;

use error::catch_error;
use prelude::*;

lazy_static! {
    static ref ASYNC_RUNTIME: Runtime = Runtime::new().unwrap();
}

fn block_on<T>(f: impl Future<Output = T>) -> T {
    ASYNC_RUNTIME.handle().block_on(f)
}

/// Create a new `Client` for sending requests to RPUs.
#[no_mangle]
pub extern "C" fn pb_client_create(
    address: Option<&c_char>,
    identity_hex: Option<&c_char>,
    client: Option<&mut *mut Client>,
) -> c_int {
    catch_error! {
        // ignore error if logger is already set
        let _ = pretty_env_logger::try_init();

        let address = unsafe { c_char_to_str(address) }?;
        let identity_hex = unsafe { c_char_to_str(identity_hex) }?;

        create(client, {
            let address = address.parse()?;
            let identity = identity_hex.parse()?;

            Client::new(address, identity)
        })?;
    }
}

/// Free the memory for the `Client`.
#[no_mangle]
pub extern "C" fn pb_client_free(client: Option<&mut Client>) {
    unsafe { free(client) };
}

/// Send a Key-Value transaction via the client.
#[no_mangle]
pub extern "C" fn pb_client_send_key_value(
    client: Option<&mut Client>,
    key: Option<&c_char>,
    value: Option<&c_char>,
) -> c_int {
    catch_error! {
        let client = client.ok_or(NullPointer)?;
        let key = unsafe { c_char_to_str(key) }?;
        let value = unsafe { c_char_to_str(value) }?;

        block_on(client.send_key_value(key.to_string(), value.to_string()))?;

        log::info!("Transaction sent successfully.");
    }
}
