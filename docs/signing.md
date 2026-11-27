# Signing the Windows installer

Release 1.0.0 ships an unsigned Windows installer, so SmartScreen warns on first run. This page
says how the owner gets a free code-signing certificate through the SignPath Foundation, and what
the release workflow already does once the secrets exist.

Sources: the SignPath Foundation terms (<https://signpath.org/terms>, read 2026-11-27) and the
`signpath/github-action-submit-signing-request` action (`action.yml` at tag v3.2, commit
`a2c5b60d9c75f073142b653c354b632eedce097b`, read 2026-11-27).

## What the workflow does now

`.github/workflows/release.yml`, job `windows`:

1. Builds and packages as before (`packaging/windows/package.ps1`).
2. When both `SIGNPATH_API_TOKEN` and `SIGNPATH_ORGANIZATION_ID` are set as repository secrets,
   it uploads the unsigned installer as the artifact `windows-installer-unsigned`, submits it to
   SignPath (project slug `ledgerly`, signing policy `release-signing`), waits for the signed
   file, checks its Authenticode signature with `Get-AuthenticodeSignature` (anything but `Valid`
   stops the release) and replaces the installer with it.
3. When either secret is missing (forks, and today), those three steps are skipped and the
   installer ships unsigned, exactly as in 1.0.0.

The `publish` job copies only the `linux`, `windows`, `windows-installer` and `macos` artifacts
into the release, so the unsigned copy sent to SignPath can never be released. `SHA256SUMS.txt`,
the build provenance attestation and the winget and Chocolatey manifests are all made from the
released (signed, when signing ran) installer.

Only the installer is signed. The `.exe` files inside the zip stay unsigned; signing them too is a
second artifact configuration in SignPath and is not set up.

## Applying to the SignPath Foundation

The Foundation signs open-source projects for free. Its terms ask for, in short:

- **An OSI-approved licence for every component, and no commercial dual licensing.** Ledgerly is
  AGPL-3.0-or-later, with some format crates under MIT OR Apache-2.0 (`LICENSING.md`). Both are
  OSI-approved. Before applying, confirm with the Foundation that the Developer Certificate of
  Origin or CLA choice (spec section 13) is not "commercial dual licensing".
- **No malware or potentially unwanted programs, and no hacking tools.**
- **An active, released project, with its functions described on the download page.** This is the
  GitHub releases page and README.
- **Automated, verifiable builds from this repository.** The release workflow builds from the tag
  on GitHub-hosted runners. Build scripts and CI configuration count as source and need review.
- **Manual approval of every signing request** by an approver.
- **Product name and version set in the signed binary.** Inno Setup writes them from `AppName`
  and `AppVersion` (`packaging/windows/ledgerly.iss`).
- **Team roles named publicly (authors, reviewers, approvers), with multi-factor authentication**
  on SignPath and on GitHub for every member.
- **A "Code signing policy" section, linked from the home page and the download page.** It holds
  the attribution line the terms give, the team roles with links to the members, and a privacy
  statement.
- **A privacy statement.** Ledgerly sends nothing over the network unless the user asks. The opt-in
  update check (off by default) contacts `api.github.com` and GitHub's release download hosts. The
  AI features are not in this release. Say exactly that.
- **A warning before system changes, and uninstall instructions.** The Inno Setup installer has an
  uninstaller listed in Windows' installed apps.
- **The certificate is issued to "SignPath Foundation".** Windows will show that name as the
  publisher, not the project.

### Steps

1. Add a `Code signing policy` section to `README.md` and to `RELEASE-NOTES.md` (the release
   page), with the attribution line copied word for word from <https://signpath.org/terms>, the
   team roles and the privacy statement above.
2. Turn on two-factor authentication for every maintainer on GitHub.
3. Apply through the form at <https://signpath.org/apply>, naming the repository
   `DevanshChoubey/tally-oss` and the artifact (the Inno Setup installer
   `ledgerly-<version>-windows-x86_64-setup.exe`).
4. Once accepted, in SignPath:
   - create the project with slug `ledgerly`;
   - create the signing policy `release-signing`, with manual approval;
   - add the GitHub connector as a trusted build system for the project;
   - make the artifact configuration a single PE file (the installer). The action uploads the
     artifact as a zip, which SignPath unpacks.
5. In GitHub, under Settings > Secrets and variables > Actions, add:
   - `SIGNPATH_API_TOKEN`: an API token of a SignPath CI user that may submit to the project;
   - `SIGNPATH_ORGANIZATION_ID`: the organisation id shown in SignPath.
6. Push a release tag. The `windows` job waits for an approver to approve the request in SignPath,
   then continues.

If the project or policy slugs differ from `ledgerly` and `release-signing`, change them in
`release.yml`.

## Package manager manifests

The templates are in `packaging/winget/` (manifest 1.6.0, three files) and
`packaging/chocolatey/`. The `publish` job runs `scripts/fill-package-manifests.sh`, which reads the
installer's line in `SHA256SUMS.txt` and fills in the version and hash. The filled files are
uploaded as the workflow artifact `package-manifests`. No hash is ever typed by hand, and the
script fails rather than write a manifest without one.

To submit them:

- **winget:** open a pull request to `microsoft/winget-pkgs` that adds the three filled files under
  `manifests/l/Ledgerly/Ledgerly/<version>/`. Check them first with
  `winget validate --manifest <folder>`.
- **Chocolatey:** run `choco pack` in the filled `chocolatey` folder, then
  `choco push ledgerly.<version>.nupkg --source https://push.chocolatey.org/` with the owner's API
  key. The first version goes through community moderation.

Until the installer is signed, both stores accept it, but users still see the SmartScreen warning.
