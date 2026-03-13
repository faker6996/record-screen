# Licensing

## Project license

This repository is distributed under the MIT License.

- Root license file: [`LICENSE`](../../LICENSE)
- Workspace license declaration: [`Cargo.toml`](../../Cargo.toml)

## Third-party software

This project depends on third-party crates, JavaScript packages, and system software.
Those components keep their own licenses.

Important runtime note:

- `ffmpeg` is currently expected as an external runtime dependency
- it is not bundled into the desktop app packages in this repository

That keeps the release packages simpler, but operators still need to review the licenses of the external tools they install on user machines.

## Distribution checklist

Before publishing production installers, review:

- the project MIT license text included in the package
- third-party dependency licenses
- trademark and icon usage
- whether external runtime dependencies are bundled or only documented
