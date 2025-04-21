#! /bin/bash
set -e

backend_path=./backend/
if [ ! -d "$backend_path" ]; then
    echo "backend_path does not exist, cloning backend"
    git clone git@github.com:tzafon/backend.git
fi
pushd $backend_path
git checkout main
git pull
popd


apps_folder=$backend_path/apps/v2/
echo "apps_folder: $apps_folder"
# delete apps folder
if [ -d "apps" ]; then
    rm -rf apps
fi
mkdir apps

# list of files to exclude
exclude_list=("*.crt" "*.key" "*.srl" "*.pem" "internal*" "*.env")
exclude_list_str=""
for exclude in ${exclude_list[@]}; do
    exclude_list_str="$exclude_list_str --exclude $exclude"
done

paths_to_sync=("proto-definition" "Cargo.*" "Dockerfile.rust-builder" \
                "rust-instance-container" "rust-shared" "rust-instance-manager" \
                "tzafonwright" ".dockerignore")
for path in ${paths_to_sync[@]}; do
    rsync -av $exclude_list_str $apps_folder/$path apps/
done

pushd apps/proto-definition/ssl_certs
bash ./gen.sh
popd