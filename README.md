# nutag

A command-line tool for creating and managing semantic version tags in Git and Jujutsu (jj) repositories.

## Features

- 🏷️ Semantic versioning support (major, minor, patch, prerelease)
- 🔄 Works with both Git and Jujutsu repositories
- 🌐 Fetches existing tags from GitHub via GraphQL API
- 📦 Supports monorepo workflows with tag prefixes
- 🎯 Smart branch detection (main/master for releases, other branches for prereleases)
- ✨ Interactive tag creation with validation

## Prerequisites

- Rust toolchain (for building)
- Git or Jujutsu (jj) installed
- GitHub Personal Access Token with `repo` scope (set as `GITHUB_TOKEN` environment variable)

```bash
export GITHUB_TOKEN=your_github_token_here
```

## Installation

```bash
cargo install --path .
```

## Usage

### Basic Usage

When run without arguments, `nutag` automatically determines the appropriate version bump based on your current branch:

```bash
# On main/master branch: suggests next patch version
nutag

# On feature branch: suggests next prerelease version
nutag
```

### Version Bumping

#### Patch Release

```bash
nutag --patch
# Example: v0.1.0 → v0.1.1
```

#### Minor Release

```bash
nutag --minor
# Example: v0.1.5 → v0.2.0
```

#### Major Release

```bash
nutag --major
# Example: v0.2.3 → v1.0.0
```

### Prerelease Versions

#### Create a Prerelease

```bash
nutag --pre
# Example: v0.1.0 → v0.1.1-pre0
```

#### Increment Prerelease

When the current version is already a prerelease, using `--pre` increments the prerelease number:

```bash
nutag --pre
# Example: v0.1.1-pre0 → v0.1.1-pre1
```

#### Prerelease with Version Bump

Combine version bumps with prerelease:

```bash
nutag --minor --pre
# Example: v0.1.5 → v0.2.0-pre0

nutag --major --pre
# Example: v0.2.3 → v1.0.0-pre0
```

### Monorepo Support (Tag Prefixes)

Use prefixes to tag specific packages or components in a monorepo:

```bash
# Tag a specific package
nutag my-package
# Creates: my-package@v0.1.0

# Bump minor version for a package
nutag --minor my-package
# Example: my-package@v0.1.5 → my-package@v0.2.0
```

### Development Workflow

#### Local Tag Creation (No Push)

Create tags locally without pushing to remote:

```bash
nutag --no-push
# Creates tag locally, does not push to remote
```

#### Verbose Output

Enable debug logging to see detailed information:

```bash
nutag --verbose
# Shows detailed logs about tag fetching, repo detection, etc.
```

### Repository Type Detection

`nutag` automatically detects whether you're in a Git or Jujutsu repository:

**Git repositories:**
- Tags the current HEAD commit
- Checks current branch with `git branch --show-current`

**Jujutsu repositories:**
- Tags `trunk()` when on main bookmark
- Tags `@` (current change) for prerelease versions
- Checks for `main` bookmark on current change

## Examples

### Standard Release Workflow

```bash
# Working on feature branch
git checkout -b feature/new-thing

# Create prerelease tags as you develop
nutag
# Creates: v0.1.1-pre0

# Make changes, create another prerelease
nutag
# Creates: v0.1.1-pre1

# Merge to main and create release
git checkout main
git merge feature/new-thing

nutag --patch
# Creates: v0.1.1 (removes prerelease suffix)
```

### Monorepo Workflow

```bash
# Tag multiple packages independently
nutag --minor api
# Creates: api@v0.2.0

nutag --patch web-client
# Creates: web-client@v0.1.1

nutag --major shared-utils
# Creates: shared-utils@v1.0.0
```

### Interactive Tag Editing

When you run `nutag`, it shows a prompt where you can edit the suggested version before creating the tag:

```
Next tag: v0.2.0
```

You can:
- Press Enter to accept the suggested version
- Edit the version number before confirming
- The tool validates your input to ensure it's a valid semantic version

## How It Works

1. **Detects repository type** (Git or Jujutsu)
2. **Fetches existing tags** from GitHub via GraphQL API
3. **Filters tags** by prefix (if provided)
4. **Determines next version** based on flags and current branch
5. **Prompts for confirmation** with interactive editing
6. **Creates annotated tag** with message
7. **Pushes to remote** (unless `--no-push` is used)

## Error Handling

If a tag already exists, `nutag` will:
- Display an error message
- Ask if you want to try a different tag name
- Allow you to enter a new version

## License

See LICENSE file for details.
