# Homebrew Tap Template

This repository now includes a Homebrew cask template and a tap skeleton so macOS distribution can move to Homebrew later without rebuilding the release process from scratch.

## What is included

- Cask template: [`packaging/homebrew/Casks/record-screen.rb.template`](../../packaging/homebrew/Casks/record-screen.rb.template)
- Render script: [`scripts/render-homebrew-cask.sh`](../../scripts/render-homebrew-cask.sh)
- Tap skeleton: [`packaging/homebrew/tap-skeleton/README.md`](../../packaging/homebrew/tap-skeleton/README.md)
- Tap bootstrap script: [`scripts/bootstrap-homebrew-tap.sh`](../../scripts/bootstrap-homebrew-tap.sh)

## Recommended repository layout

Use a dedicated tap repository:

```text
faker6996/homebrew-tap
└── Casks/
    └── record-screen.rb
```

Homebrew documents third-party taps separately from the main `homebrew/cask` catalog, and the normal install flow for a custom tap is:

```bash
brew tap faker6996/tap
brew install --cask faker6996/tap/record-screen
```

Source:

- Homebrew taps documentation: https://docs.brew.sh/Taps
- Homebrew cask authoring guide: https://docs.brew.sh/Cask-Cookbook

## How to render the cask

After you have:

- a published GitHub release tag such as `v0.1.0`
- a public DMG asset URL
- the SHA-256 of that DMG

run:

```bash
VERSION=0.1.0 \
SHA256=<sha256-of-dmg> \
URL=https://github.com/faker6996/record-screen/releases/download/v0.1.0/<dmg-file-name> \
scripts/render-homebrew-cask.sh
```

That generates:

```text
packaging/homebrew/Casks/record-screen.rb
```

You can then copy that file into the tap repository under `Casks/record-screen.rb`.

## Bootstrap a separate tap repository

To create a starter repository layout for `faker6996/homebrew-tap` or another custom tap:

```bash
scripts/bootstrap-homebrew-tap.sh /path/to/homebrew-tap
```

Behavior:

- if a rendered cask already exists, the bootstrap script copies it into the new repo as `Casks/record-screen.rb`
- otherwise it copies the template files and keeps workflow files as `.template`

## Suggested publish flow

1. Build and publish the signed macOS DMG on GitHub Releases.
2. Compute the SHA-256 checksum of the DMG.
3. Run `scripts/render-homebrew-cask.sh`.
4. Copy the generated cask to the tap repo.
5. Commit and push the tap repo.
6. Test locally:

```bash
brew install --cask ./Casks/record-screen.rb
```

7. Test from the tap:

```bash
brew tap faker6996/tap
brew install --cask faker6996/tap/record-screen
```

## Notes

- This project does not publish a Homebrew tap yet.
- The cask template assumes the installed application bundle is named `Record Screen.app`.
- The generated cask uses `strategy :github_latest` in `livecheck`, which is suitable if releases are published on GitHub Releases.
