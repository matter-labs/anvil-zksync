#!/usr/bin/env bash

set -e

ANVIL_ZKSYNC_REPO="https://github.com/matter-labs/anvil-zksync"

function script_usage() {
    cat << EOF
anvil-zksync Installer v0.1.0

USAGE:
    -h | --help              Display help information
    -v | --version           Downloads a specific version of anvil-zksync             (default: latest release)
    -d | --destination       The path to the folder where the binary will be installed  (default: /usr/local/bin)
EOF
}

function parse_args() {
    version=$(get_latest_version)
    destination="/usr/local/bin"

    while [[ $# -gt 0 ]]; do
        arg="$1"
        shift
        case $arg in
            -h | --help)
                script_usage
                exit 0
                ;;
            -v | --version)
                version="$1"
                shift
                ;;
            -d | --destination)
                destination="$1"
                shift
                ;;
            *)
                echo "Invalid argument was provided: $arg"
                exit 1
                ;;
        esac
    done
}

function main() {
    parse_args "$@"

    echo "Running install script for anvil-zksync..."

    get_os_info
    download_binary
    prepare_binary

    echo "anvil-zksync has been successfully installed!"
}

function prepare_binary() {
    echo "Preparing binary..."

    tar xz -f "$file_name"
    rm "$file_name"
    mv anvil-zksync "$destination/anvil-zksync"
    chmod +x "$destination/anvil-zksync"

    echo "Successfully prepared binary!"
}

function download_binary() {
    file_name="anvil-zksync-$version-$architecture-$os.tar.gz"
    url="$ANVIL_ZKSYNC_REPO/releases/download/$version/$file_name"

    echo "Downloading anvil-zksync binary from: $url..."
    wget $url

    echo "Successfully downloaded anvil-zksync Binary!"
}

function get_os_info() {
    unamestr="$(uname)"
    case "$unamestr" in
        "Linux")
            os="unknown-linux-gnu"
            arch=$(lscpu | awk '/Architecture:/{print $2}')
            ;;
        "Darwin")
            os="apple-darwin"
            arch=$(arch)
            if [[ "$arch" == "i386" ]]; then
                # Rosetta reports as i386, but we treat it as x86_64
                arch="x86_64"
            fi
            ;;
        *)
            echo "ERROR: anvil-zksync only supports Linux and MacOS! Detected OS: $unamestr"
            exit 1
            ;;
    esac

    case "$arch" in
        "x86_64")
            architecture="x86_64"
            ;;
        "arm64")
            architecture="aarch64"
            ;;
        *)
            echo "Operating system: $os"
            echo "Architecture: $arch"
            echo "ERROR: Unsupported architecture detected!"
            exit 1
            ;;
    esac

    echo "Operating system: $os"
    echo "Architecture: $architecture"
}

function get_latest_version() {
    # TODO: update repo name when it's created
    echo v$(curl --proto '=https' -sSf https://raw.githubusercontent.com/matter-labs/anvil-zksync/main/Cargo.toml | \
        grep "version" -m 1 | \
        awk '{print $3}' | \
        sed 's/"//g')
}

main "$@"
