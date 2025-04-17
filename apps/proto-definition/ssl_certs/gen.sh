#!/bin/bash

set -e

rm -rf ca
rm -rf server
rm -rf client

mkdir -p ca
mkdir -p server
mkdir -p client

openssl genrsa -out ca/tls.key 2048
openssl req -new -x509 -days 365 -key ca/tls.key -out ca/tls.crt -subj "/CN=US"

openssl genrsa -out server/tls.key 2048
openssl req -new -key server/tls.key -out server/csr.pem -subj "/CN=US"
openssl x509 -req -days 365 -in server/csr.pem -CA ca/tls.crt -CAkey ca/tls.key -CAcreateserial -out server/tls.crt -extfile server.conf

openssl genrsa -out client/tls.key 2048
openssl req -new -key client/tls.key -out client/csr.pem -subj "/CN=US"
openssl x509 -req -days 365 -in client/csr.pem -CA ca/tls.crt -CAkey ca/tls.key -CAcreateserial -out client/tls.crt -extfile server.conf

