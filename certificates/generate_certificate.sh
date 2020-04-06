#!/bin/bash

# CONFIGURATION
ca_common_name="prellblock-ca"
ca_country=DE
ca_state=Brandenburg
ca_location="Im Zug"
ca_organization="Deutsche Bahn"
ca_organizational_unit="HPI BPAP1"

days=730 # must be under 800 or so in macOS

password=prellblock # The password for identity files.

# CAN'T TOUCH THIS... DUM DUDUDUMM

folder="$(dirname "$0")"
cd "$folder"

ca_folder="ca"
mkdir -p "$ca_folder"
ca="$ca_folder/ca_$ca_common_name"

server_cn="$2"
server_ip="$3"
server="$server_cn/$1"

# Create server folder folder
mkdir -p "$server_cn"

# Generate CA cert
if [ ! -f "$ca.cert" ]; then
    openssl ecparam -genkey -name secp384r1 -noout -out "$ca.key"
    openssl req -x509 -new -nodes -sha512 -days "$days" -extensions v3_ca -key "$ca.key" -subj "/CN=$ca_common_name/C=$ca_country/ST=$ca_state/L=$ca_location/O=$ca_organization/OU=$ca_organizational_unit" -out "$ca.cert"
fi

# Generate server key
openssl ecparam -genkey -name secp384r1 -noout -out "$server.key"

# Generate configuration file for signing request
cat > "$server.csr.cnf" << EOF
[req]
default_md = sha512
distinguished_name = dn
prompt       = no
utf8         = yes

[dn]
C=$ca_country
ST=$ca_state
L=$ca_location
O=$ca_organization
OU=$ca_organizational_unit RPU
CN=$server_cn
EOF

# Generate signing request
openssl req -new -sha512 -nodes -out "$server.csr" -key "$server.key" -config "$server.csr.cnf"

# Generate signing extension file

cat > "$server.v3.ext" << EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = $server_cn
IP.1 = $server_ip
EOF

# Sign with CA
openssl x509 -req -in "$server.csr" -CA "$ca.cert" -CAkey "$ca.key" -CAcreateserial -out "$server.cert" -days $days -sha512 -extfile "$server.v3.ext"

# Convert to pfx
openssl pkcs12 -export -in "$server.cert" -inkey "$server.key" -out "$server.pfx" -passout pass:$password