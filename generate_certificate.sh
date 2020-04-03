#!/bin/bash

# CONFIGURATION
crypto_folder="crypto"
ca_common_name="localhost"
country=DE
days=3562
password=prellblock

# CAN'T TOUCH THIS... DUM DUDUDUMM

ca="$crypto_folder/ca_$ca_common_name"

server="$crypto_folder/$1"
cn="$2"

# Create artefact folder
mkdir -p "$crypto_folder"


# Generate CA cert
if [ ! -f "$ca.cert" ]; then
    openssl ecparam -genkey -name secp384r1 -noout -out "$ca.key"
    # openssl genrsa 4096 > "$ca.key"
    openssl req -x509 -new -sha512 -days "$days" -extensions v3_ca -key "$ca.key" -subj "/CN=$ca_common_name/C=$country" -out "$ca.cert"
fi

# Generate server key
# openssl ecparam -genkey -name secp384r1 -noout -out "$server.key"
openssl genrsa 4096 > "$server.pem"
# Generate signing request
#openssl req -new -sha512 -nodes -key "$server.key" -subj "/CN=$cn/C=$country" -out "$server.csr"
# Sign with CA
# openssl x509 -req -sha512 -days "$days" -in "$server.csr" -CA "$ca.cert" -CAkey "$ca.key" -CAcreateserial -out "$server.cert"
openssl req -x509 -new -key "$server.pem" -out "$server.cert.pem" -subj "/CN=127.0.0.1/C=DE"
# Convert to pfx
# openssl pkcs12 -export -in "$server.cert" -inkey "$server.key" -out "$server.pfx" -certfile "$ca.cert" -passout "pass:$password"
openssl pkcs12 -export -in "$server.cert.pem" -inkey "$server.pem" -out "$server.pfx" -passout pass:$password