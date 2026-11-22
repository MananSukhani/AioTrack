# Signing keystore

The release keystore is **not** stored in this repository.

GitHub Actions signs the release APK with two repository secrets:

- `ANDROID_KEYSTORE_BASE64` — base64-encoded `release.jks` (PKCS12, alias `aiotrack`)
- `ANDROID_KEYSTORE_PASSWORD` — the keystore/key password

The workflow decodes the keystore into the runner's temporary directory at build
time and never writes it into the checkout.

For local builds, generate or supply your own keystore, put it in this folder,
and export `CARGO_APK_RELEASE_KEYSTORE` and `CARGO_APK_RELEASE_KEYSTORE_PASSWORD`.
Never commit the keystore or its password.

> The old key that was committed in early versions was rotated. Because the
> signing identity changed, existing installs must be replaced with a fresh
> install of the new release APK.
