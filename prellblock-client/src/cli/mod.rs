mod config;
mod options;
mod subscriptions;

pub mod prelude {
    pub use super::{
        config::Config,
        options::{cmd, Cmd, Opt},
        subscriptions::SubscriptionConfig,
    };
}
