# Embedded Binaries

Place the compiled `snapdog` binary here before building the app.

```bash
# From the snapdog workspace root:
cargo build --release -p snapdog
cp target/release/snapdog macos-helper/SnapDogHelper/Sources/SnapDogHelper/Resources/bin/
```

The binary is embedded in the app bundle at `Contents/Resources/bin/snapdog`.
