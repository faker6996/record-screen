# APT Repository

This project now includes a skeleton flow for a real APT repository so Linux users can eventually install the app with:

```bash
sudo apt install record-screen
```

instead of only:

```bash
sudo apt install ./record-screen_<version>_amd64.deb
```

## What is included

- Repository builder: [`scripts/build-apt-repo.sh`](../../scripts/build-apt-repo.sh)
- Publish workflow: [`.github/workflows/publish-apt-repo.yml`](../../.github/workflows/publish-apt-repo.yml)

## Publishing model

The intended flow is:

1. A release tag such as `v0.1.0` is published.
2. The Linux `.deb` asset is downloaded from that GitHub Release.
3. A signed APT repository is generated.
4. The repository is deployed to GitHub Pages under:

```text
https://<owner>.github.io/<repo>/apt
```

This follows GitHub's documented Pages workflow pattern using:

- `actions/configure-pages@v5`
- `actions/upload-pages-artifact@v4`
- `actions/deploy-pages@v4`

Source:

- https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages

## Required GitHub secrets

Before enabling the publish flow in production, configure:

- `APT_GPG_PRIVATE_KEY`
- `APT_GPG_PASSPHRASE` if the key is passphrase protected

Without `APT_GPG_PRIVATE_KEY`, the workflow is designed to skip cleanly.

## End-user install flow

Once the APT repository is published, users should be able to run:

```bash
curl -fsSL https://<owner>.github.io/<repo>/apt/record-screen-archive-keyring.asc | sudo gpg --dearmor -o /usr/share/keyrings/record-screen-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/record-screen-archive-keyring.gpg] https://<owner>.github.io/<repo>/apt stable main" | sudo tee /etc/apt/sources.list.d/record-screen.list
sudo apt update
sudo apt install record-screen
```

## Notes

- This repository does not publish the APT repo yet until the workflow is pushed and the signing secrets are configured.
- The generated repo currently targets `amd64`.
- The package name remains `record-screen`, regardless of the internal binary file name.
