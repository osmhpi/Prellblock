use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Opt {
    /// Private key file path.
    pub private_key_file: String,
    /// The address of the receiving RPU's address.
    pub turi_address: String,
    #[structopt(subcommand)]
    pub cmd: Cmd,
}

#[derive(StructOpt, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Cmd {
    /// Set a single key value pair.
    Set(cmd::Set),
    /// Run a benchmark.
    #[structopt(name = "bench")]
    Benchmark(cmd::Benchmark),
    /// Update an account.
    #[structopt(name = "update_account")]
    UpdateAccount(cmd::UpdateAccount),
    /// Create an account.
    #[structopt(name = "create_account")]
    CreateAccount(cmd::CreateAccount),
    /// Delete an account.
    #[structopt(name = "delete_account")]
    DeleteAccount(cmd::DeleteAccount),
    /// Get values from the blockchain.
    ///
    /// Specifying only a filter returns the last recorded value.
    #[structopt(name = "get_value")]
    GetValue(cmd::GetValue),
    /// Get accounts from the blockchain.
    #[structopt(name = "get_account")]
    GetAccount(cmd::GetAccount),
    /// Get blocks from the blockchain.
    #[structopt(name = "get_block")]
    GetBlock(cmd::GetBlock),
    /// Get the current block number (that is going to be committed).
    #[structopt(name = "current_block_number")]
    CurrentBlockNumber,
}

pub mod cmd {
    use pinxit::PeerId;
    use prellblock_client::{consensus::BlockNumber, Filter, Span};
    use std::str::FromStr;
    use structopt::StructOpt;

    /// Transaction to set a key to a value.
    #[derive(StructOpt, Debug)]
    pub struct Set {
        /// The key of this transaction.
        pub key: String,
        /// The value of the corresponding key.
        pub value: String,
    }

    /// Benchmark the blockchain.
    #[derive(StructOpt, Debug)]
    pub struct Benchmark {
        /// The key to use for saving benchmark generated data.
        pub key: String,
        /// The number of transactions to send
        pub transactions: u32,
        /// The number of bytes each transaction's payload should have.
        #[structopt(short, long, default_value = "8")]
        pub size: usize,
        /// The number of workers (clients) to use simultaneously.
        #[structopt(short, long, default_value = "1")]
        pub workers: usize,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct UpdateAccount {
        /// The public key of the account to update.
        pub peer_id: String,
        /// The filepath to a yaml-file cotaining the accounts permissions.
        pub permission_file: String,
    }

    /// Create a new account.
    #[derive(StructOpt, Debug)]
    pub struct CreateAccount {
        /// The public key of the account to create.
        pub peer_id: String,
        /// The name of the account to create.
        pub name: String,
        /// The filepath to a yaml-file cotaining the accounts permissions.
        pub permission_file: String,
    }

    /// Delete an account.
    #[derive(StructOpt, Debug)]
    pub struct DeleteAccount {
        /// The public key of the account to delete.
        pub peer_id: String,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetValue {
        /// The `PeerId` to fetch values from.
        pub peer_id: PeerId,
        /// A filter to select keys.
        pub filter: ParseFilter<String>,
        /// The span of values to fetch.
        ///
        /// Valid examples are: 10 (fetch 10 values), 200ms (200ms worth of data), 200s (200s worth of data),
        /// 2020-01-01T00:00:00 (all values after this date).
        #[structopt(default_value = "1")]
        pub span: ParseSpan,
        /// The last value to fetch.
        ///
        /// Valid examples are: 2 (skip the last 2 values), 200s (skip the last 200s),
        /// 2020-01-01T00:00:00 (only values before 2020-01-01).
        #[structopt(default_value = "0")]
        pub end: ParseSpan,
        /// The span of elements to skip between each fetched value.
        ///
        /// Valid examples are: 1 (skip every second value), 200ms (always skip 200ms).
        /// Dates won't be accepted.
        pub skip: Option<ParseSpan>,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetAccount {
        /// The IDs of the accounts to fetch.
        pub peer_ids: Vec<PeerId>,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetBlock {
        /// A filter to select some blocks.
        ///
        /// Valid examples are: 42 (block 42), .. (get all blocks), ..42 (blocks 0 to 41),
        /// 42.. (blocks 42 to current), 200..220 (blocks 200 to 219).
        pub filter: ParseFilter<BlockNumber>,
    }

    #[derive(Debug)]
    pub struct ParseFilter<T>(pub Filter<T>);

    impl FromStr for ParseFilter<String> {
        type Err = std::convert::Infallible;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let filter = if let Some(pos) = s.find("..") {
                let start = s[..pos].to_string();
                let end = s[pos + 2..].to_string();
                if end.is_empty() {
                    (start..).into()
                } else {
                    (start..end).into()
                }
            } else {
                s.to_string().into()
            };
            Ok(Self(filter))
        }
    }

    impl FromStr for ParseFilter<BlockNumber> {
        type Err = std::num::ParseIntError;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let parse_block_number = |s: &str| {
                if s.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(BlockNumber::new(s.parse()?)))
                }
            };
            let filter = if let Some(pos) = s.find("..") {
                let start = parse_block_number(&s[..pos])?;
                let end = parse_block_number(&s[pos + 2..])?;
                match (start, end) {
                    (Some(start), Some(end)) => (start..end).into(),
                    (Some(start), None) => (start..).into(),
                    (None, Some(end)) => (BlockNumber::default()..end).into(),
                    (None, None) => (BlockNumber::default()..).into(),
                }
            } else {
                parse_block_number(s)?.unwrap().into()
            };
            Ok(Self(filter))
        }
    }

    #[derive(Debug)]
    pub struct ParseSpan(pub Span);

    impl FromStr for ParseSpan {
        type Err = Box<dyn std::error::Error>;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let span = if let Ok(count) = s.parse() {
                Span::Count(count)
            } else if let Ok(duration) = humantime::parse_duration(s) {
                Span::Duration(duration)
            } else if let Ok(time) = humantime::parse_rfc3339_weak(s) {
                Span::Time(time)
            } else {
                return Err("Not a number, duration or time".into());
            };
            Ok(Self(span))
        }
    }
}
