#!/bin/bash

key=crypto/key.pem
cert=crypto/cert.pem
pfx=crypto/identity.pfx
cn=127.0.0.1

openssl genrsa 2048 > "$key"

openssl req -x509 -new -key "$key" -out "$cert" -subj "/CN=$cn/C=DE"

openssl pkcs12 -export -in "$cert" -inkey "$key" -out "$pfx" -passout pass:
