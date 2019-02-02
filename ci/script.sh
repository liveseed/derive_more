# This script takes care of testing your crate

set -ex

# TODO This is the "test phase", tweak it as you see fit
main() {
    cross build --target "$TARGET"
    cross build --target "$TARGET" --release

    if [ -n "$DISABLE_TESTS" ]; then
        return
    fi

    cross test --target "$TARGET"
    cross test --target "$TARGET" --release
    cross test --target "$TARGET" --test no_std --features no_std
    cross test --target "$TARGET" --release --test no_std --features no_std
}

# we don't run the "test phase" when doing deploys
main
