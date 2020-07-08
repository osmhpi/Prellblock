# Fill Collins - the genesis wizard

The setup wizard *"Fill Collins"* will help you create the genesis transactions for the Prellblock blockchain.

To run an instance of the Prellblock Blockchain, it is necessary to create at least 4 RPU Accounts. It is also recommended to create an Admin Account in order to be able to manage accounts, change permissions and manage RPUs after the initial start of Prellblock.

## Running the genesis wizard

To start *Fill Collins* run :

```sh
cargo run --bin genesis-wizard
```

The wizard will offer you the following options which will be covered in the sections below:
```
❯ Create ed25519 key (for signing genesis configuration)
❯ Manage accounts
❯ Manage TLS certificates
❯ Finish and generate configuration files
❯ Cancel
```

It is recommended to complete these steps in the given order (will reduce frustration significantly) from top to bottom. The [`Create ed25519 key`](#creating-an-ed25519-key) option is purely optional, but be aware that you **will definitely** need a vaild ed25519 key later in the process for signing the genesis transactions. So if you do not have a key yet, be sure to use this option.

## Creating an ed25519 key

In order to be accepted as the genesis configuration, all transactions need to be signed by a valid signing identity. Throughout the entirety of the Prellblock blockchain, ed25519 key-pairs are used to identify accounts and sign messages.

The `Create ed25519 key` option will create an ed25519 private key, which you can use later to sign the genesis transactions. 
The key will be stored in a location of your desire (`<selected-path>/ca.key`).

## Managing Accounts

This option will enable you to create new accounts, to list or remove existing accounts.
There are several options to specify an account which you should set.  

```
❯ Public Key (*optional*)  
❯ Name  
❯ Account-Type  
❯ Expiry date  
❯ Set writing rights  
❯ Set reading rights  
❯ Show account
❯ Finish
❯ Abort mission
```
0. The Public Key is an optional parameter. If not specified, a unique key will be automatically generated on `Finish`.
1. Every account requires a `Name`, which can be used to furter identify an account.
2. Next an `Account-Type` is expected. In Prellblock there are four types of accounts:
   - **Normal**: a normal account with no special privileges
   - **BlockReader**: an account that can read whole blocks and therefore read all values
   - **Admin**: an RPU that can participate in the consensus
   - **RPU**: an admin that can manage and edit all other accounts
3. There need to be at least four accounts of type `RPU` to form a working Prellblock blockchain. An RPU needs to be setup with a `Turi IPv4 address` and a `Peer IPv4 address`.The first of which is used to connect from a Client to an RPU, the second for internal communication between RPUs.
4. For security reasons an account may have an expiry date. If you choose to set an expiry date you may do so in [rfc3999](https://tools.ietf.org/html/rfc3339) format.
5. An account may have a right to `write` into his own `keyspace`. Therefore `writing rights` is either `true` or `false` (default is `false`).
6. `Reading rights` can be extensively defined. You can choose to define a `Blacklist` or `Whitelist` to deny or allow certain accesses respectively. They consist of a list of `accounts` and corresponding `namespaces` (provides as strings). 
> If you are creating the account *bob* and want *bob* to be able to read *alice's* keys *temperature* and *speed*, you would create a Whitelist, selecting *alice* and the namespaces. You may define multiple black- or whitelists to specify the reading rights.
8. You can view the results with the `Show account` option.
9. When you are sure, that this account is setup correctly, the option `Finish` stores this account. Otherwise, you can `Abort mission` and cancel the account's creation.

## Managing TLS certificates

The Prellblock blockchain uses `Transport Layer Security` to ensure the privacy and security of all communication amongst the clients and RPUs. Thus it is necessary to create valid [x509 TLS-certificates](https://www.ssl.com/faqs/what-is-an-x-509-certificate/) for all the RPUs partaking in the system.

When selecting the `Manage TLS certificates` option, you will be prompted to select one of the following actions:
```
❯ Create CA certificate
❯ Load CA certificate
❯ Create keys and certificates for all RPUs
❯ Go back
```

1. If you alerady have a fitting cerificate for a Certificate Authority ready, you should select the `Load CA certificate` option and then specify the path to the private key and the certificate (in PEM format). Otherwise select the `Create CA certificate` option. This option will prompt you for several pieces of information. In case you have any trouble with these prompts either check out the [openSSL x509 documentation](https://www.openssl.org/docs/man1.0.2/man1/x509.html) or just roll with the default options (not recommended in production).
2. After you got the CA certificate ready, select the `Create keys and certificates for all RPUs` option. This will automatically create all the necessary TLS certificates as well as the TLS private keys using a hardcoded expiry date of 1 year into the future from time of creation by default.
3. Once the above 2 steps are done, use the `Go back` option to return to the main menu.

## Finishing and generating configuration files

The `Finish and generate configuration files` option is going to be the last step before being ready to get Prellblock up and running.

First you will be prompted for a ed25519 private key for signing the transactions. Here you will need to enter the key itself (note that you cannot see any of what you are typing or pasting here - because security -, so don't worry if the field stays blank upon entry of he key). You can use the key from the `Create e25519 key` option from the main menu if you used that, otherwise just paste an ed25519 private key of your own. 

Next you need to specify multiple paths for storing all the generated keys, certificates and configuration files. 

The first of these prompts will be about the accounts' ed25519 key-pair directory. In this directory (e.g. `config`) the wizard will create a subdirectory for each account using the account's name as the directory name. These subdirectorys will then contain the private key (`<name>.key`), the public key (`<name>.pub`) if no public key was provided in the [Managing Accounts](#managing-accounts) step, the TLS identity file (`<name>.pfx`) and the prvate RPU configuration (`<name>.toml`) for each RPU. For non-RPU accounts, there will only be the private and public key files.

The second prompt will ask you to specify the path for the TLS CA certificate and once given a path, will ask for a password for the CA private key. (The password will be prompted twice to confirm you did not make any spelling mistakes). This will create `<selected-path>/ca-certificate.pem` and `<selected-path>/ca-private-key.pem`.

Lastly you will be asked for a path in which you want to store the genesis configuration file (`genesis.yaml`). Select one and the process will automatically finish writing all necessary files.

Congratulations, you just completed your Prellblock blockchain setup!
With four RPUs (emily, james, percy, thomas) and default paths the directory structure should look something like this:

```
config
├── ca
│   ├── ca-certificate.pem
│   ├── ca.key
│   └── ca-private-key.pem
├── emily
│   ├── emily.key
│   ├── emily.pfx
│   ├── emily.pub
│   └── emily.toml
├── genesis
│   └── genesis.yaml
├── james
│   ├── james.key
│   ├── james.pfx
│   ├── james.pub
│   └── james.toml
├── percy
│   ├── percy.key
│   ├── percy.pfx
│   ├── percy.pub
│   └── percy.toml
└── thomas
    ├── thomas.key
    ├── thomas.pfx
    ├── thomas.pub
    └── thomas.toml
```