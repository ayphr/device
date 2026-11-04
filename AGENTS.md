# Releasing Ayphr Device (App & Firmware)

Releases are tag-driven. Pushing a version tag triggers the GitHub Actions workflows that build, sign, and publish the release assets. No manual version edits are required — the workflows sync `package.json`, `tauri.conf.json`, and `Cargo.toml` from the tag during the build.

## Prerequisites

- `gh` authenticated: `gh auth status`
- Clean `main` branch: `git checkout main && git pull --ff-only`
- CI green on `main`

## Determine the next version

```bash
# Latest app and firmware releases
gh release list --limit 5
```

App tags use the `app-vX.Y.Z` prefix; firmware tags use `firmware-vX.Y.Z`. Increment the patch (or minor/major) number as appropriate.

## Release the companion app

```bash
VERSION=0.1.34   # replace with the target version
git tag "app-v${VERSION}"
git push origin "app-v${VERSION}"
```

Monitor and verify:

```bash
gh run list --workflow release-app.yml --limit 3
gh run watch   # optional: follow the run
gh release view "app-v${VERSION}"
```

The `release-app` workflow builds all platforms, then publishes the merged updater feed to the `app-latest` release (`latest.json`), which the in-app updater consumes.

## Release firmware

```bash
VERSION=0.1.18   # replace with the target version
git tag "firmware-v${VERSION}"
git push origin "firmware-v${VERSION}"
```

Monitor and verify:

```bash
gh run list --workflow release-firmware.yml --limit 3
gh run watch
gh release view "firmware-v${VERSION}"
gh release view firmware-latest   # manifest + signature consumed by the app
```

Firmware releases are created as prereleases and publish `version.json`, the signed `.bin`, and its `.sig` to both the versioned release and the rolling `firmware-latest` release.

## Manual fallback (gh CLI only)

If you must create a release without CI, use `gh release create` with the built assets, but prefer the tag-driven flow above so assets are built and signed consistently:

```bash
gh release create "app-v${VERSION}" ./path/to/assets/* \
  --title "App v${VERSION}" --generate-notes

gh release create "firmware-v${VERSION}" ./dist/* \
  --title "Firmware v${VERSION}" --prerelease --generate-notes
```

## Notes

- Firmware signing requires the `FIRMWARE_SIGNING_KEY` secret; app signing requires `TAURI_SIGNING_PRIVATE_KEY`.
- Do not delete the rolling `app-latest` / `firmware-latest` releases — the companion app depends on them.
- If a tag was pushed in error, delete it locally and remotely before re-pushing: `git tag -d <tag> && git push origin :refs/tags/<tag>`.
```

Then commit/push? The user asked make file. Should we commit? Probably yes, but maybe not necessary. Let's commit and push to keep repo updated.<|end|><|start|> fileList <|constrain|>xml<|message|><rfl:parameter name="command">
cd /Users/proplayer919/Documents/Coding/Ayphr/device && git add AGENTS.md && git commit -m "Add release instructions for app and firmware" && git push 2>&1 | tail -2