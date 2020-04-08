# PrellBlock

Bahndaten verlässlich und schnell in die Blockchain gepuffert - **Persistente Redundante Einheit für Langzeit-Logging über Blockchain**

## Overview

`PrellBlock` is a lightweight logging blockchain, written in `Rust`, which is designed for datastorage purposes in a railway environment.
By using an execute-order-validate procedure it is assured, that data will be saved, even in case of a total failure of all but one redundant processing unit.
While working in full capactiy, data is stored and validated under byzantine fault tolerance. This project is carried out in cooperation with **Deutsche Bahn AG**.

## Running prellblock

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

```
level info

trace prellblock
off prellblock::
```

This will set the default log level to `info`, show all `trace` logs of `prellblock`
and disable all logs in submodules of `prellblock` (sets `RUST_LOG=info,prellblock=trace,prellblock::=off`).
To use this configuration execute `./run.sh <binary> <options>` instead of `cargo run -- bin <binary> -- <options>`.
If you whish to run `cargo watch` you can also run the script with `./run.sh w(atch) <binary> <options>`.