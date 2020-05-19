# PrellBlock

Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**

[[_TOC_]]

## Overview

`PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

## Running prellblock

### Usage Of TLS

The blockchain by default uses TLS for the connections.
You therefore need certificates for running a prellblock **RPU**.

This can be achieved by creating a custom CA. A [script](./certificates/generate_certificate.sh) automatically creates a CA certificate.
For **every RPU** you need to run:

```sh
certificates/generate_certificate.sh <desired_output_name> <rpu_dns_name> <rpu_ip>
```

Running the script creates a CA-key and -certificate in `./certificates/ca` and some files in `./certificates/<desired_output_name>`.

The most important file is `./certificates/<desired_output_name>/<desired_output_name>.pfx`. It is used by prellblock to load the TLS certificate.
Since it is protected by a password, *prellblock* needs to know the password for reading the file.

**Warning: Do not use the default password in production!**

You can pass another password to *prellblock* via the `TLS_PASSWORD` environment variable.

### RPU Identitiy

Each RPU has to have a identity. They can be generated with the following command:

```sh
cargo run --bin gen-key <rpu_name>
```

The files are placed (and searched in prellblock) in `config/<rpu_name>/`:

- `<rpu_name>.key` is the **private key** of the identity.
- `<rpu_name>.pub` is the **public key** of the identity.

### Configuration

Please have a look at the sections [Usage Of TLS](#Usage-Of-TLS) and [RPU Identity](#rpu-identity) beforehand.
Configuration of the blockchain peers is done via `.toml` files.
The main configuration file is searched at `config/config.toml`.
Each peer has a name and adresses. The corresponding public keys (`peer_id`s) must be *accessible for all peers*.
An example for this **public configuration** could look like this
(all paths are relative from the current working directory):

```toml
[[rpu]]
name = "emily"
peer_id = "./config/emily/emily.pub"
peer_address = "127.0.0.1:2480"
turi_address = "127.0.0.1:3130"

[[rpu]]
name = "james"
peer_id = "./config/james/james.pub"
peer_address = "127.0.0.1:2481"
turi_address = "127.0.0.1:3131"

[[rpu]]
name = "percy"
peer_id = "./config/percy/percy.pub"
peer_address = "127.0.0.1:2482"
turi_address = "127.0.0.1:3132"

[[rpu]]
name = "thomas"
peer_id = "./config/thomas/thomas.pub"
peer_address = "127.0.0.1:2483"
turi_address = "127.0.0.1:3133"
```

Each peer also needs a **private configuration**:

```toml
identity = "./config/emily/emily.key"
tls_id = "./certificates/emily/emily.pfx"
```

The `identity` here is the private key as generated in [RPU Identitiy](#RPU-Identitiy).
The `tls_id` is the `pfx`-file containing the private key and certificate signed by the CA.

### Using `prellblock-client`

The `prellblock-client` binary provides a CLI with predefined Commands for each of the transaction types. Otherwise, you can use the provided library as dependency to build your own clients.

Currently implemented actions are:

- setting a key to a specific Value (using `set <key> <value>` subcommand)
- updating account permissions  (using `update <AccountId> <PermissionFilePath>` subcommand)
- benchmarking (using `bench <rpu_name> <key> <number of transactions>` subcommand)

#### Key-Value Transactions

The keys for this type of transaction needs to be of type `string`, whereas values may be of any type. 

#### Updating Account permissions:

For updating the permissions of a account, the sender account **must be an administrator**
The specified AccountID needs to be a hex-value `PeerId`.

```sh
prellblock-client <target account id as hex> <path to permission file>
```

The permission file is a `yaml`-file containing all necessary permission inforamtions to be applied onto the target account.
Reading rights can be specified as white- or blacklist. They refer to one or more accounts. Futhermore you can define white- or blacklists for specific keys that either can or must not be read.
For its structure, see the example below (all fields can be ommitted resulting in that field being left unchanged):

```yaml
is_admin: true
is_rpu: false
expire_at:
  at_date: 2020-04-15T10:04:59.300878700Z # any date according to RFC3339
# expire_at: never # this can be used for accounts to never expire
writing_rights: true
reading_rights:
  - accounts:
      whitelist:
        - scope: Emily
    namespace:
      blacklist:
        - scope: speed
  - accounts:
      whitelist:
        - scope: Thomas
    namespace:
      blacklist:
        - scope: temperature
```

### Logging

To help setting the correct log-levels, you can use the [`run.sh`](./run.sh) script.
You **need to create** a `run.local.sh` script to configure logging.
Available levels are:

- `trace`
- `debug`
- `info`
- `warn`
- `error`
- `off`

An example would be:

```sh
level info

trace prellblock
off prellblock::
```

This will set the default log level to `info`, show all `trace` logs of `prellblock`
and disable all logs in submodules of `prellblock` (sets `RUST_LOG=info,prellblock=trace,prellblock::=off`).
To use this configuration execute `./run.sh <binary> <options>` instead of `cargo run -- bin <binary> -- <options>`.
If you whish to run `cargo watch` you can also run the script with `./run.sh w(atch) <binary> <options>`.

### Profiling

For testing speed and efficiency of the *prellblock*, there is a tool called [flamegraph-rs/flamegraph](https://github.com/flamegraph-rs/flamegraph).
You can install it via `cargo install flamegraph`. On Linux (Debian) you need to install `linux-perf`, too.
To generate an interactive graph on **Linux**, run:

```sh
sudo sysctl -w kernel.perf_event_paranoid=1
./run.sh f prellblock <options>
```

On **macOS** run:

```sh
./run.sh f prellblock <options>
```

After stopping the program, a graph (`flamegraph.svg`) will be created.

## Cross-Compiling for ARM based machines

For running *prellblock* on a RaspberryPi or similar ARM-based machines, you need to cross compile the blockchain.
Building fully static binaries can be done via [`cross`](https://github.com/rust-embedded/cross) (this needs Docker to be running):

```sh
cargo install cross
cross build --target armv7-unknown-linux-musleabihf --release
```
