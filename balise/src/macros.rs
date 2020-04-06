//! Macros for building an API.

#[doc(hidden)]
#[macro_export]
macro_rules! request_response_inner {
    ($enum_name:ident, $request_name:ident, $response:ty) => {
        impl $crate::Request<$enum_name> for $request_name {
            type Response = $response;
        }

        impl From<$request_name> for $enum_name {
            fn from(v: $request_name) -> Self {
                Self::$request_name(v)
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! request_and_response {
    (
        $enum_name:ident {
            $(#[$inner:meta])*
            $request_name:ident($($type:ty),*) => $response:ty,
            $($tail:tt)*
        }
    ) => {
        $(#[$inner])*
        #[derive(Debug, Serialize, Deserialize)]
        pub struct $request_name($(pub $type),*);

        $crate::request_response_inner!($enum_name, $request_name, $response);

        $crate::request_and_response!{
            $enum_name {
                $($tail)*
            }
        }
    };
    (
        $enum_name:ident {
            $(#[$inner:meta])*
            $request_name:ident => $response:ty,
            $($tail:tt)*
        }
    ) => {
        #[derive(Debug, Serialize, Deserialize)]
        $(#[$inner])*
        pub struct $request_name;

        $crate::request_response_inner!($enum_name, $request_name, $response);

        $crate::request_and_response!{
            $enum_name {
                $($tail)*
            }
        }
    };
    (
        $enum_name:ident { }
    ) => {}
}

#[macro_export]
#[doc(hidden)]
macro_rules! request_enum {
    (
        $message_module_name:ident
        $(#[$outer:meta])*
        $vis:vis $enum_name:ident {
            $(#[$inner:meta])*
            $request_name:ident($($type:ty),*) => $response:ty,
            $($tail:tt)*
        }
        ( $($enum_variants:tt)* )
    ) => {
        $crate::request_enum! {
            $message_module_name
            $(#[$outer])*
            $vis $enum_name {
                $($tail)*
            }
            (
                $($enum_variants)*
                #[allow(missing_docs)]
                $request_name($message_module_name::$request_name),
            )
        }
    };
    (
        $message_module_name:ident
        $(#[$outer:meta])*
        $vis:vis $enum_name:ident {
            $(#[$inner:meta])*
            $request_name:ident => $response:ty,
            $($tail:tt)*
        }
        ( $($enum_variants:tt)* )
    ) => {
        $crate::request_enum! {
            $message_module_name
            $(#[$outer])*
            $vis $enum_name {
                $($tail)*
            }
            (
                $($enum_variants)*
                #[allow(missing_docs)]
                $request_name($message_module_name::$request_name),
            )
        }
    };
    (
        $message_module_name:ident
        $(#[$outer:meta])*
        $vis:vis $enum_name:ident { }
        ( $($enum_variants:tt)* )
    ) => {
        $(#[$outer])*
        #[derive(Debug, Serialize, Deserialize)]
        $vis enum $enum_name {
            $($enum_variants)*
        }
    };
}

/// Define an API.
///
/// # Example
/// ```
/// use balise::{define_api, Request};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// pub struct Pong;
///
/// define_api! {
///     mod ping_message;
///     pub enum PingAPIRequest {
///         Add(usize, usize) => usize,
///         Ping => Pong,
///     }
/// }
///
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! define_api {
    (
        $(#[$modmeta:meta])*
        mod $message_module_name:ident;

        $(#[$outer:meta])*
        $vis:vis enum $enum_name:ident {
            $($inner:tt)*
        }
    ) => {
        $crate::request_enum! {
            $message_module_name
            $(#[$outer])*
            $vis $enum_name {
                $($inner)*
            }
            ()
        }
        $(#[$modmeta])*
        $vis mod $message_module_name {
            use super::*;
            $crate::request_and_response! {
                $enum_name {
                    $($inner)*
                }
            }
        }
    };
}
