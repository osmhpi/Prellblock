use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Opt {
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
    #[structopt(name = "update")]
    UpdateAccount(cmd::UpdateAccount),
    /// Get values from the blockchain.
    #[structopt(name = "get_value")]
    GetValue(cmd::GetValue),
    /// Get accounts from the blockchain.
    #[structopt(name = "get_account")]
    GetAccount(cmd::GetAccount),
    /// Get blocks from the blockchain.
    #[structopt(name = "get_block")]
    GetBlock(cmd::GetBlock),
    /// Get the current block number.
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
        /// The name of the RPU to benchmark.
        pub rpu_name: String,
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
        /// The id of the account to update.
        pub id: String,
        /// The filepath to a yaml-file cotaining the accounts permissions.
        pub permission_file: String,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetValue {
        /// The `PeerId` to fetch values from.
        pub peer_id: PeerId,
        /// A filter to select keys.
        pub filter: ParseFilter<String>,
        /// The span of values to fetch.
        #[structopt(default_value = "1")]
        pub span: ParseSpan,
        /// The last value to fatch.
        #[structopt(default_value = "0")]
        pub end: ParseSpan,
        /// The span of elements to skip between each fetched value.
        pub skip: Option<ParseSpan>,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetAccount {
        /// The ids of the accounts to fetch.
        pub peer_ids: Vec<PeerId>,
    }

    /// Update the permissions for a given account.
    #[derive(StructOpt, Debug)]
    pub struct GetBlock {
        /// A filter to select some blocks.
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
