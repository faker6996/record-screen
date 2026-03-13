# Distribution Overview

Record Screen ships a different package format per operating system:

| Operating system | Primary artifact | Intended install flow |
| :--- | :--- | :--- |
| Linux | `.deb` | Download the package and install it with `apt` |
| macOS | `.dmg` | Download the disk image and drag the app into `Applications` |
| Windows | `Setup.exe` | Run the NSIS installer |

## Package manager support

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

### macOS and Homebrew

macOS can be distributed through Homebrew Cask, but that requires a published cask in either:

- a custom tap maintained by this project
- or the community `homebrew-cask` catalog

This repository does not publish a Homebrew tap yet. The supported install path today is the DMG.

## Runtime dependency policy

The current app expects `ffmpeg` to be available on `PATH` at runtime on all desktop platforms.
That dependency is documented per-platform in the OS guides below.
