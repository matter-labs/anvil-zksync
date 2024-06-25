#!/usr/bin/env bash

set -e

ERA_TEST_NODE_REPO="https://github.com/matter-labs/era-test-node"

function script_usage() {
    cat << EOF
Era Test Node Installer v0.1.0

USAGE:
    -h | --help              Display help information
    -v | --version           Downloads a specific version of Era Test Node              (default: latest release)
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

    echo "Running install script for Era Test Node..."

    get_os_info
    download_binary
    prepare_binary

    echo "Era Test Node has been successfully installed!"
}

function prepare_binary() {
    echo "Preparing binary..."

    tar xz -f "$file_name"
    rm "$file_name"
    mv era_test_node "$destination/era_test_node"
    chmod +x "$destination/era_test_node"

    echo "Succesfully prepared binary!"
}

function download_binary() {
    file_name="era_test_node-$version-$architecture-$os.tar.gz"
    url="$ERA_TEST_NODE_REPO/releases/download/$version/$file_name"

    echo "Downloading Era Test Node binary from: $url..."
    wget $url

    echo "Successfully downloaded Era Test Node Binary!"
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
            ;;
        *)
            echo "ERROR: Era Test Node only supports Linux and MacOS! Detected OS: $unamestr"
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
            echo "ERROR: Unsupported architecture detected!"
            exit 1
            ;;
    esac

    echo "Operating system: $os"
    echo "Architecture: $architecture"
}

function get_latest_version() {
    echo v\
        $(curl --proto '=https' -sSf https://raw.githubusercontent.com/matter-labs/era-test-node/main/Cargo.toml | \
        grep "version" -m 1 Cargo.toml | \
        awk '{print $3}' | \
        sed 's/"//g')"
}

main "$@"
