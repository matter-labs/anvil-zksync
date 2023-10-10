#!/bin/bash
# Created with passion by @iwantanode
# Check if the 'rustc' command is available
if command -v rustc &> /dev/null; then
  echo "Rust is already installed."
  rustc --version
else
  echo "Rust is not installed. Installing Rust..."

  # Download and run the official Rust installation script
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

  if [ $? -eq 0 ]; then
    echo "Rust has been successfully installed."
    rustc --version
  else
    echo "Failed to install Rust."
    exit 1
  fi
fi
sudo apt-get install -y cmake pkg-config libssl-dev clang

curl -LO 
https://github.com/matter-labs/era-test-node/releases/download/v0.1.0-alpha.6/era_test_node-v0.1.0-alpha.6-x86_64-unknown-linux-gnu.tar.gz
tar xz -f era_test_node-v0.1.0-alpha.6-x86_64-unknown-linux-gnu.tar.gz
chmod +x era_test_node
./era_test_node run
