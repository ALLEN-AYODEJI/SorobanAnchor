# Release Artifact Signing and Verification

SorobanAnchor release artifacts are signed so consumers can confirm that a
published binary or WASM file came from the intended source and was not
tampered with in transit.

---

## Table of Contents

1. [What is signed](#1-what-is-signed)
2. [Signing backends](#2-signing-backends)
3. [Signing a release](#3-signing-a-release)
4. [Verifying a release](#4-verifying-a-release)
5. [CI integration](#5-ci-integration)
6. [Key management](#6-key-management)
7. [Dry-run support](#7-dry-run-support)

---

## 1  What is signed

Every release bundle produced by `make release` contains:

| Artifact | Description |
|----------|-------------|
| `anchorkit-<VERSION>.tar.gz` | Complete release tarball |
| `anchorkit-<VERSION>.sha256` | SHA-256 checksum of the tarball |

The signing workflow (`sign_release.sh`) produces detached signatures for both
the tarball and the checksum file, giving two independent verification paths.

---

## 2  Signing backends

Two backends are supported.  Select one via the environment variable:

```bash
export ANCHORKIT_SIGNING_BACKEND=gpg       # default
export ANCHORKIT_SIGNING_BACKEND=minisign
```

| Backend | Signature files | Prerequisites |
|---------|----------------|---------------|
| **GPG** (default) | `.sig` (ASCII-armored) | `gpg` installed, signing key in keyring |
| **minisign** | `.minisig` | `minisign` installed, secret key file |

---

## 3  Signing a release

### Step 1 — Build the release artifacts

```bash
make release
# Produces: dist/anchorkit-<VERSION>.tar.gz
#           dist/anchorkit-<VERSION>.sha256
```

### Step 2 — Sign

**GPG (default):**

```bash
# Sign with the default GPG key:
make release-sign

# Sign with a specific key:
ANCHORKIT_GPG_KEY_ID=releases@example.com make release-sign

# Produces:
#   dist/anchorkit-<VERSION>.tar.gz.sig
#   dist/anchorkit-<VERSION>.sha256.sig
```

**minisign:**

```bash
ANCHORKIT_SIGNING_BACKEND=minisign \
ANCHORKIT_MINISIGN_KEY=~/.minisign/minisign.key \
make release-sign

# Produces:
#   dist/anchorkit-<VERSION>.tar.gz.minisig
#   dist/anchorkit-<VERSION>.sha256.minisig
```

### Step 3 — Publish

Include both signature files alongside the tarball and checksum in your GitHub
Release / distribution channel.

---

## 4  Verifying a release

Download the tarball and its accompanying `.sha256` and signature files, then
run:

### Verify checksum integrity only

From the repository root, with the downloaded files under `dist/`, verify the
packaged tarball checksum:

**Linux / macOS:**

```bash
VERSION="0.2.0" # Replace with the downloaded release version.
sha256sum --check "dist/anchorkit-${VERSION}.sha256"
# Expected: dist/anchorkit-0.2.0.tar.gz: OK
```

**macOS (alternative):**

```bash
VERSION="0.2.0"
shasum -a 256 --check "dist/anchorkit-${VERSION}.sha256"
# Expected: dist/anchorkit-0.2.0.tar.gz: OK
```

**Windows (PowerShell):**

```powershell
$version = "0.2.0"
$expected = (Get-Content "dist\anchorkit-$version.sha256").Split()[0]
$actual = (Get-FileHash "dist\anchorkit-$version.tar.gz" -Algorithm SHA256).Hash.ToLower()
if ($expected -eq $actual) {
    Write-Host "dist\anchorkit-$version.tar.gz: OK" -ForegroundColor Green
} else {
    Write-Host "dist\anchorkit-$version.tar.gz: FAILED" -ForegroundColor Red
    Write-Host "Expected: $expected" -ForegroundColor Red
    Write-Host "Actual:   $actual" -ForegroundColor Red
    exit 1
}
```

#### What checksum verification proves

A matching checksum confirms that:

- The downloaded file is bitwise-identical to the file used to generate the
  published checksum.
- No corruption or incomplete download occurred.

A matching checksum **does not** prove:

- **Who** published the file (authentication).
- **When** the file was built (freshness).
- **Whether** the publisher is trustworthy.

**Trust boundary:** Checksum verification detects accidental mismatch or
corruption, but it does not independently authenticate the publisher. An
attacker with access to the distribution channel could publish both a
malicious tarball and a matching checksum.

For authentication, verify the detached signature separately (see below).

#### What to do if verification fails

If checksum verification fails:

1. **Re-download** the tarball — network errors can corrupt downloads.
2. **Check the source** — ensure you downloaded from the official GitHub
   Releases page or a trusted mirror.
3. **Verify the checksum file** — ensure the `.sha256` file itself was not
   corrupted or tampered with. Compare its signature (see signature
   verification below).
4. **Report the mismatch** — if the problem persists, report it to the
   maintainers immediately. Do not use the tarball.

### Verify the signed release

```bash
# GPG (default):
make release-verify TARBALL=dist/anchorkit-0.1.0.tar.gz

# Or directly:
./scripts/verify_release.sh dist/anchorkit-0.1.0.tar.gz
```

The script performs three checks:

1. **SHA-256 checksum** — confirms file integrity.
2. **Signature verification** — confirms the publisher's identity.
3. **Bundle content inspection** — confirms all required artifacts are present.

**Verifying the signer identity (GPG):**

```bash
ANCHORKIT_GPG_SIGNER="releases@example.com" \
./scripts/verify_release.sh dist/anchorkit-0.1.0.tar.gz
```

**Verifying with minisign:**

```bash
ANCHORKIT_SIGNING_BACKEND=minisign \
ANCHORKIT_MINISIGN_PUBKEY=anchorkit-release.pub \
./scripts/verify_release.sh dist/anchorkit-0.1.0.tar.gz
```

Exit code `0` means all checks passed.  Non-zero means at least one check
failed; the output identifies the failing step.

---

## 5  CI integration

The CI workflow does **not** perform signing automatically on PRs or branch
builds — signing requires access to the release key, which is only available
to maintainers.

For production releases (`push` to `main` with a version tag), add a manual
workflow dispatch step or a protected environment that injects the signing key:

```yaml
# .github/workflows/ci.yml  — release-package job (extract)
- name: Sign release artifacts
  if: startsWith(github.ref, 'refs/tags/v')
  env:
    ANCHORKIT_GPG_KEY_ID: ${{ secrets.RELEASE_GPG_KEY_ID }}
  run: |
    echo "${{ secrets.RELEASE_GPG_KEY }}" | gpg --import
    bash scripts/sign_release.sh

- name: Verify signatures
  if: startsWith(github.ref, 'refs/tags/v')
  run: bash scripts/verify_release.sh dist/anchorkit-${VERSION}.tar.gz

- name: Upload signed artifacts
  if: startsWith(github.ref, 'refs/tags/v')
  uses: actions/upload-artifact@v4
  with:
    name: anchorkit-signed-release
    path: |
      dist/anchorkit-*.tar.gz
      dist/anchorkit-*.sha256
      dist/anchorkit-*.sig
    retention-days: 90
```

---

## 6  Key management

### Custody boundaries

Release signing divides into two responsibilities:

| Responsibility | Who | Where | Requires signing key |
|----------------|-----|-------|---------------------|
| **Build** | Any contributor | Local or CI | ❌ No |
| **Sign** | Maintainers only | Protected environment | ✅ Yes |
| **Verify** | Anyone | Any environment | ❌ No (public key only) |

**Contributors** build and test release artifacts locally but **cannot sign**
them. Signing requires access to the private signing key, which is restricted
to maintainers.

**Maintainers** sign artifacts in a protected environment (a secure local
machine or a CI job with secret access). The signing step runs **after** the
build and artifact packaging steps complete successfully.

**Users** verify signatures using the published public key. The public key is
safe to distribute and does not grant signing authority.

### Key storage

- The release signing key must **never** be committed to the repository.
- Store the private key in GitHub Actions secrets (`RELEASE_GPG_KEY`,
  `RELEASE_GPG_KEY_ID`) or in a hardware security module for mainnet releases.
- Publish the **public key** in the repository (e.g. `docs/anchorkit-release.pub`)
  and in your release notes so users can import it before verifying.
- Rotate keys annually or immediately after any suspected compromise.

### Local signing workflow (maintainers only)

Maintainers with the signing key installed locally can sign a release after
building:

```bash
# 1. Build release artifacts (any contributor can do this)
make release

# 2. Sign artifacts (maintainers only — requires GPG key in keyring)
ANCHORKIT_GPG_KEY_ID=releases@example.com make release-sign

# 3. Verify signatures before publishing
make release-verify TARBALL=dist/anchorkit-0.1.0.tar.gz
```

If you do not have the signing key, you can still build and test release
artifacts. The CI pipeline will handle signing when a tagged release is
pushed by a maintainer.

### Questions about key access

If you need signing access or suspect a key compromise, contact the
maintainers privately (do not open a public issue). See
[SECURITY.md](../SECURITY.md) for contact information.

---

## 7  Dry-run support

To see what signing commands would be executed without actually signing:

```bash
make release-sign-dry-run

# Or directly:
./scripts/sign_release.sh --dry-run
```

The dry-run mode prints each command prefixed with `[dry-run]` and exits
without writing any signature files.  This is useful for validating CI
pipeline configuration before a real release.

---

## References

- [sign_release.sh](../scripts/sign_release.sh)
- [verify_release.sh](../scripts/verify_release.sh)
- [package_release.sh](../scripts/package_release.sh)
- [Artifact Provenance Inspection](artifact-provenance.md)
- [Governance and Security](governance-and-security.md)
- [Upgrade Playbook](upgrade-playbook.md)
