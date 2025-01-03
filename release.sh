#!/bin/bash

if [[ -z $(which gh) ]]; then
    echo "gh: GitHub CLI not found (see https://cli.github.com/)"
    exit 1
fi

function build() {
    local __PROG="ped"
    local __ARCH="$1"
    local __TARGET="${__ARCH}-apple-darwin"
    local __BIN_DIR="$(pwd)/target/${__TARGET}/release"
    local __BIN="$__BIN_DIR/$__PROG"

    echo "$__TARGET: building target"
    cargo test --release
    cargo build --release --target=${__TARGET}
    if [ $? -ne 0 ]; then
        echo "$__BIN: build failure"
        exit 1
    fi

    __TAR="$__BIN_DIR/$__PROG-${__VERSION}-${__ARCH}-unix.tar.gz"
    tar -czf "$__TAR" -C "$__BIN_DIR" "$__PROG"
    shasum -a 256 "$__TAR" > "$__TAR.sha256"
}

echo "detecting version"
__VERSION=$(cargo run --release -- --version | cut -d " " -f 2)

__TARGETS=("aarch64" "x86_64")
__FILES=()
for __TARGET in "${__TARGETS[@]}"
do
    build "$__TARGET"
    __FILES+=("$__TAR" "$__TAR.sha256")
done

gh release create --title "$__VERSION" --generate-notes "v$__VERSION" ${__FILES[@]}
if [ $? -eq 0 ]; then
    echo "$__VERSION: release successful"
else
    echo "$__VERSION: release failed"
fi
