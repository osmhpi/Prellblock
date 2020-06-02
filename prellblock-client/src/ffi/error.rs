use std::{
    cell::RefCell,
    ffi::CString,
    os::raw::{c_char, c_int},
    str,
};

pub const SUCCESS: c_int = 0;
pub const ERROR: c_int = -1;

pub struct CStringError(CString);

fn to_c_string(s: String) -> CString {
    let s = match CString::new(s) {
        Ok(s) => s,
        Err(err) => {
            let s = format!("{:?}", str::from_utf8(&err.into_vec()).unwrap());
            match CString::new(s) {
                Ok(s) => s,
                Err(_) => CString::new("Invalid error message").unwrap(),
            }
        }
    };
    if s.as_bytes().is_empty() {
        CString::new("Empty error message").unwrap()
    } else {
        s
    }
}

impl<T> From<T> for CStringError
where
    T: ToString,
{
    fn from(v: T) -> Self {
        Self(to_c_string(v.to_string()))
    }
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// Get the last error message of the prellblock client.
///
/// Returns a thread local reference to the last error message.
#[no_mangle]
pub extern "C" fn pb_last_error() -> *const c_char {
    LAST_ERROR.with(|last_error| last_error.borrow().as_ptr())
}

pub fn catch_error(f: impl FnOnce() -> Result<(), CStringError>) -> c_int {
    match f() {
        Ok(()) => SUCCESS,
        Err(err) => {
            LAST_ERROR.with(|last_error| *last_error.borrow_mut() = err.0);
            ERROR
        }
    }
}

macro_rules! catch_error {
    ($($code:tt)*) => {
        catch_error(|| Ok({
            $($code)*
        }))
    };
}
