#!/bin/bash
set -e
hash=$(./proto-definition/get-hash.sh)
echo -n $hash > ./proto-definition/proto_version