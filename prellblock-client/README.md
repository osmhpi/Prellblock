# Prellblock Client

The client can be used to communicate with the `prelllbock` blockchain.

## Using the FFI for C/C++ Applications

Building `prellblock` also creates a static library `libprellblock_client.a`.
This can be linked against in your C/C++ application.
See [prellblock-client.h](prellblock-client.h) for the API and [the provided example](../c-client/) for usage.

## Regenerating the Header file

The header file is generated via [`cbindgen`](https://github.com/eqrion/cbindgen).
After installing it, execute `cbindgen -c cbindgen.toml --crate prellblock-client -o prellblock-client.h` from *this directory*.
