#! /bin/bash

set -e

pushd apps
./proto-definition/build.sh
cargo build --all
popd


