cross_target := "armv7-unknown-linux-gnueabihf"

cross-controller:
    cross build -p samwise-controller --target {{cross_target}} --release

sync: cross-controller
    scp target/armv7-unknown-linux-gnueabihf/release/samwise-controller pi@faramir.local: