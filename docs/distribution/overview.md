# Distribution Overview

Record Screen ships a different package format per operating system:

| Operating system | Primary artifact | Intended install flow |
| :--- | :--- | :--- |
| Linux | `.deb` | Download the package and install it with `apt` |
| macOS | `.dmg` | Download the disk image and drag the app into `Applications` |
| Windows | `Setup.exe` | Run the NSIS installer |

## Package manager support

## GitHub Releases vs GitHub Packages

For this project, desktop binaries should be published as GitHub Release assets:

- macOS: `.dmg`
- Linux: `.deb`
- Windows: setup `.exe`

The GitHub `Packages` section is designed for supported registries such as container, npm, Maven, NuGet, and similar ecosystems.
It is not the primary distribution channel for desktop installer files, so it is normal for the `Packages` section to remain empty even when release packaging is fully configured.

### Linux and `apt`

Linux users can install the downloaded Debian package with:

```bash
sudo apt install ./record-screen_<version>_amd64.deb
```

This uses `apt`, but it is not the same as publishing a first-class APT repository.
To support `apt install record-screen` without a local file path, the project would still need:

- a signed APT repository
- repository metadata generation
- a published installation key and source list instructions

The repository skeleton for that future flow now exists in:

- [`scripts/build-apt-repo.sh`](../../scripts/build-apt-repo.sh)
- [`docs/distribution/apt-repo.md`](./apt-repo.md)

### macOS and Homebrew

macOS can be distributed through Homebrew Cask, but that requires a published cask in either:

- a custom tap maintained by this project
- or the community `homebrew-cask` catalog

This repository now includes an automated custom tap publish path for a dedicated `homebrew-tap` repository once the required token is configured.
The direct DMG install path still remains valid.

## Runtime dependency policy

The current app expects `ffmpeg` to be available on `PATH` at runtime on all desktop platforms.
That dependency is documented per-platform in the OS guides below.
