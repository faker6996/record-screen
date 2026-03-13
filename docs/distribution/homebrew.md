# Homebrew Tap

This repository now includes a real publish path for a dedicated Homebrew tap, following the same release workflow style used for the APT repository.

## What is included

- Cask template: [`packaging/homebrew/Casks/record-screen.rb.template`](../../packaging/homebrew/Casks/record-screen.rb.template)
- Render script: [`scripts/render-homebrew-cask.sh`](../../scripts/render-homebrew-cask.sh)
- Publish script: [`scripts/publish-homebrew-tap.sh`](../../scripts/publish-homebrew-tap.sh)
- Tap skeleton: [`packaging/homebrew/tap-skeleton/README.md`](../../packaging/homebrew/tap-skeleton/README.md)
- Tap bootstrap script: [`scripts/bootstrap-homebrew-tap.sh`](../../scripts/bootstrap-homebrew-tap.sh)
- Integrated publish workflow: [`.github/workflows/build-installers.yml`](../../.github/workflows/build-installers.yml)

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

## Publishing model

The intended flow is:

1. A new version is pushed to `main`.
2. The main release workflow builds the macOS `.dmg`.
3. The workflow creates the Git tag and GitHub Release.
4. The workflow computes the DMG SHA-256 and renders `record-screen.rb`.
5. The workflow clones the Homebrew tap repo and updates `Casks/record-screen.rb`.
6. The tap repo is committed and pushed automatically.

By default, the tap target is:

```text
<owner>/homebrew-tap
```

and the default branch is:

```text
main
```

## Required GitHub secret and optional repository variables

Configure the following in the main project repository:

- secret: `HOMEBREW_TAP_TOKEN`

Optional:

- variable: `HOMEBREW_TAP_REPOSITORY`
- variable: `HOMEBREW_TAP_BRANCH`

`HOMEBREW_TAP_TOKEN` should be a token that can push to the tap repository.

Without `HOMEBREW_TAP_TOKEN`, the Homebrew publish step is designed to skip cleanly while the normal desktop release still succeeds.

## How to render the cask manually

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

## Manual fallback flow

If you do not want to use the automated publish path, the manual process is still:

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

- The tap repository itself must already exist; this workflow updates it, but does not create it on GitHub for you.
- The cask template assumes the installed application bundle is named `Record Screen.app`.
- The generated cask uses `strategy :github_latest` in `livecheck`, which is suitable if releases are published on GitHub Releases.
