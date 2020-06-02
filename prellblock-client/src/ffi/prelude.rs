use super::error::CStringError;
use std::{ffi::CStr, fmt, os::raw::c_char};

pub struct NullPointer;

impl fmt::Display for NullPointer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Found null pointer")
    }
}

pub unsafe fn c_char_to_str(s: Option<&c_char>) -> Result<&str, CStringError> {
    Ok(CStr::from_ptr(s.ok_or(NullPointer)?).to_str()?)
}

pub fn create<T>(ptr: Option<&mut *mut T>, value: T) -> Result<(), CStringError> {
    *ptr.ok_or(NullPointer)? = Box::into_raw(Box::new(value));
    Ok(())
}

pub unsafe fn free<T>(ptr: Option<&mut T>) -> Option<T> {
    ptr.map(|ptr| *Box::from_raw(ptr))
}
