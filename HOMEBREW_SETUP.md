# Homebrew Self-Publishing Setup

This repository is configured to automatically build, release, and publish `nutag` via Homebrew.

## How It Works

### 1. GitHub Workflows

#### Release Workflow (`.github/workflows/release.yml`)
This workflow is triggered when you push a tag starting with `v` (e.g., `v0.1.0`).

It automatically:
- Builds binaries for:
  - Linux x86_64
  - Linux ARM64 (aarch64)
  - macOS x86_64 (Intel)
  - macOS ARM64 (Apple Silicon)
- Strips debug symbols for smaller binaries
- Creates tarballs for each platform
- Generates SHA256 checksums
- Creates a GitHub release with all binaries attached

#### Formula Update Workflow (`.github/workflows/update-formula.yml`)
This workflow is triggered when a release is published.

It automatically:
- Downloads all release artifacts
- Calculates SHA256 checksums for each tarball
- Updates the Homebrew formula in `Formula/nutag.rb` with:
  - New version number
  - New download URLs
  - New SHA256 checksums
- Commits and pushes the updated formula back to the repository

### 2. Homebrew Tap

This repository serves as a Homebrew tap. The formula is located at `Formula/nutag.rb`.

## Usage

### For End Users

Users can install `nutag` using Homebrew:

```bash
# Add the tap (only needed once)
brew tap felipesere/nutag

# Install nutag
brew install nutag

# Or do both in one command
brew install felipesere/nutag/nutag
```

### For Maintainers

#### Creating a New Release

1. **Use the nutag tool itself**:
   ```bash
   # For a patch release
   nutag --patch

   # For a minor release
   nutag --minor

   # For a major release
   nutag --major
   ```

   This will create and push a new version tag.

2. **Or manually create a tag**:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. **Wait for automation**:
   - The `release.yml` workflow will build binaries and create a GitHub release
   - The `update-formula.yml` workflow will update the Homebrew formula
   - No manual intervention needed!

#### First Release Setup

For your first release, you need to:

1. **Ensure the version in `Cargo.toml` matches your tag**:
   ```toml
   [package]
   version = "0.1.0"  # Should match your tag (v0.1.0)
   ```

2. **Create and push your first tag**:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. **Wait for the workflows to complete**:
   - Check the Actions tab in GitHub
   - The release workflow should complete first
   - Then the formula update workflow will run
   - The formula will be automatically updated with checksums

## Verifying the Setup

After your first release:

1. **Check the GitHub Release**:
   - Go to the Releases page
   - Verify all 4 binaries are attached
   - Verify SHA256SUMS file is present

2. **Test the Homebrew installation**:
   ```bash
   brew tap felipesere/nutag
   brew install nutag
   nutag --version
   ```

3. **Check the formula was updated**:
   - Look at `Formula/nutag.rb` in the main branch
   - Verify the version number matches your release
   - Verify all SHA256 checksums are filled in (not empty strings)

## Troubleshooting

### Formula has empty SHA256 checksums
This happens on the first release before the automation runs. After the first release is published, the `update-formula.yml` workflow will automatically fill in the checksums.

### Workflow fails on cross-compilation
Make sure the `release.yml` workflow has the correct cross-compilation tools installed. Linux ARM64 builds require `gcc-aarch64-linux-gnu`.

### Homebrew can't find the tap
Make sure your repository is public, or if private, users need to authenticate:
```bash
brew tap felipesere/nutag https://github.com/felipesere/nutag
```

### Binary doesn't have execute permissions
The workflow strips binaries but preserves permissions. If there's an issue, check the tarball creation step in the release workflow.

## Repository Structure

```
nutag/
├── .github/
│   └── workflows/
│       ├── release.yml           # Builds and releases binaries
│       ├── update-formula.yml    # Updates Homebrew formula
│       └── rust.yml              # CI for PRs
├── Formula/
│   └── nutag.rb                  # Homebrew formula
├── src/
│   └── ...                       # Rust source code
├── Cargo.toml
└── README.md
```

## Advanced: Multi-Architecture Support

The workflows build for all major platforms:
- **Linux x86_64**: Most Linux users
- **Linux ARM64**: Raspberry Pi, ARM servers
- **macOS x86_64**: Intel Macs
- **macOS ARM64**: Apple Silicon Macs (M1, M2, M3, etc.)

The Homebrew formula automatically selects the correct binary based on the user's system using `Hardware::CPU.arm?` checks.
