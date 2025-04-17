#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# Define source and destination paths
SOURCE_DIR="$(dirname "$0")/ssl_certs"
DEST_DIR="/etc/ssl_certs"

# Check if source directory exists
if [ ! -d "$SOURCE_DIR" ]; then
  echo "Error: Source directory '$SOURCE_DIR' not found."
  exit 1
fi

# Check if required source files exist
if [ ! -f "$SOURCE_DIR/server/tls.crt" ] || \
   [ ! -f "$SOURCE_DIR/server/tls.key" ] || \
   [ ! -f "$SOURCE_DIR/ca/tls.crt" ] || \
   [ ! -f "$SOURCE_DIR/client/tls.crt" ] || \
   [ ! -f "$SOURCE_DIR/client/tls.key" ]; then
  echo "Error: One or more required certificate files are missing in '$SOURCE_DIR'."
  exit 1
fi

echo "Creating destination directories with sudo..."
sudo mkdir -p "$DEST_DIR/server"
sudo mkdir -p "$DEST_DIR/ca"
sudo mkdir -p "$DEST_DIR/client"

echo "Copying certificates with sudo..."
sudo cp "$SOURCE_DIR/server/tls.crt" "$DEST_DIR/server/tls.crt"
sudo cp "$SOURCE_DIR/server/tls.key" "$DEST_DIR/server/tls.key"
sudo cp "$SOURCE_DIR/ca/tls.crt" "$DEST_DIR/ca/tls.crt"
sudo cp "$SOURCE_DIR/client/tls.crt" "$DEST_DIR/client/tls.crt"
sudo cp "$SOURCE_DIR/client/tls.key" "$DEST_DIR/client/tls.key"

echo "Setting appropriate permissions with sudo..."
# Restrict access to server key
sudo chmod 600 "$DEST_DIR/server/tls.key"
# Make other certificates/keys readable (adjust if needed for stricter security)
sudo chmod 644 "$DEST_DIR/server/tls.crt"
sudo chmod 644 "$DEST_DIR/ca/tls.crt"
sudo chmod 644 "$DEST_DIR/client/tls.crt"
sudo chmod 644 "$DEST_DIR/client/tls.key"

echo "Certificate setup complete."
echo "Files copied to:"
echo "  - $DEST_DIR/server/tls.crt"
echo "  - $DEST_DIR/server/tls.key"
echo "  - $DEST_DIR/ca/tls.crt"
echo "  - $DEST_DIR/client/tls.crt"
echo "  - $DEST_DIR/client/tls.key"

echo "Remember to make this script executable: chmod +x proto-definition/local-setup.sh"
