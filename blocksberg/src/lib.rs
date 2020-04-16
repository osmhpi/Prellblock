mod serde;
pub use crate::serde::ByteArrayHelper;

#[macro_export]
macro_rules! write_hex {
    ($f:ident, $data:expr, $len:expr) => {{
        let data: &[u8; $len] = $data;
        let data: &[u8] = data;
        let out = &mut [0; $len * 2];
        $crate::private::encode_to_slice(data, out).unwrap();
        $f.write_str(::std::str::from_utf8(out).unwrap())
    }};
}

#[doc(hidden)]
pub mod private {
    pub use hex::encode_to_slice;
}
