mod config;
mod options;

pub mod prelude {
    pub use super::{
        config::Config,
        options::{cmd, Cmd, Opt},
    };
}
