use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Opt {
    #[structopt(subcommand)]
    pub cmd: Cmd,
}

#[derive(StructOpt, Debug)]
pub enum Cmd {
    Set(cmd::Set),
    #[structopt(name = "bench")]
    Benchmark(cmd::Benchmark),
    #[structopt(name = "update")]
    UpdateAccount(cmd::UpdateAccount),
}

pub mod cmd {
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
}
