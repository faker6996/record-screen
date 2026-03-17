# Licensing

## Project license

This repository is distributed under the MIT License.

- Root license file: [`LICENSE`](../../LICENSE)
- Workspace license declaration: [`Cargo.toml`](../../Cargo.toml)

## Third-party software

This project depends on third-party crates, JavaScript packages, and system software.
Those components keep their own licenses.

Important runtime note:

- runtime dependencies are platform-specific and are not bundled uniformly into every desktop package in this repository
- Linux now expects native GStreamer / PipeWire runtime packages instead of a Linux `ffmpeg` dependency
- Windows still relies on `ffmpeg` as part of the current runtime path

That keeps the release packages simpler, but operators still need to review the licenses of the external tools they install on user machines.

## Distribution checklist

Before publishing production installers, review:

- the project MIT license text included in the package
- third-party dependency licenses
- trademark and icon usage
- whether external runtime dependencies are bundled or only documented
