#!/bin/bash

if [[ -z $(which gh) ]]; then
    echo "gh: GitHub CLI not found (see https://cli.github.com/)"
    exit 1
fi

function build() {
    local __ARCH="$1"
    local __TARGET="${__ARCH}-apple-darwin"
    local __BIN_DIR="$(pwd)/target/${__TARGET}/release"
    local __BIN="$__BIN_DIR/$__PROG"

    echo "$__TARGET: building target"
    cargo build --release --target=${__TARGET}
    if [ $? -ne 0 ]; then
        echo "$__BIN: build failed"
        exit 1
    fi
    cargo test --release --target=${__TARGET}
    if [ $? -ne 0 ]; then
        echo "$__BIN: test failed"
        exit 1
    fi

    __TAR="$__BIN_DIR/$__PROG-${__VERSION}-${__ARCH}-unix.tar.gz"
    tar -czf "$__TAR" -C "$__BIN_DIR" "$__PROG"
    shasum -a 256 "$__TAR" > "$__TAR.sha256"
}

__VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r ".packages[0].version")
echo "${__VERSION}: version detected"

gh release list | grep "${__VERSION}" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "${__VERSION}: release already exists"
    exit 1
fi

cargo clean
__PROG="ped"
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
    exit 1
fi
