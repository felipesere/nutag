use std::fmt::{Display, Write};
use std::process::Command;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context};
use argh::FromArgs;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use log::{debug, error, info, warn};
use nanoserde::{DeJson, SerJson};
use owo_colors::OwoColorize;
use regex_lite::Regex;
use semver::{BuildMetadata, Prerelease};

#[derive(Debug, FromArgs)]
/// Suggest the next version for tagging
struct Args {
    /// suggest the next major version
    #[argh(switch)]
    major: bool,

    /// suggest the next minor version
    #[argh(switch)]
    minor: bool,

    /// suggest the next patch version
    #[argh(switch)]
    patch: bool,

    /// suggest the next prerelease version
    #[argh(switch)]
    pre: bool,

    /// lower the log level to Debug
    #[argh(switch)]
    verbose: bool,

    /// create the tag locally but don't push it
    #[argh(switch)]
    no_push: bool,

    /// a prefix to use when creating the tag
    #[argh(positional)]
    prefix: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            major: false,
            minor: false,
            patch: false,
            pre: true,
            verbose: false,
            no_push: false,
            prefix: None,
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let mut args: Args = argh::from_env();

    let log_level = if args.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            let level = match record.level() {
                log::Level::Error => "ERROR".red().to_string(),
                log::Level::Warn => "WARN".yellow().to_string(),
                log::Level::Info => "INFO".blue().to_string(),
                log::Level::Debug => "DEBUG".green().to_string(),
                log::Level::Trace => "TRACE".magenta().to_string(),
            };

            out.finish(format_args!("{level}: {message}",))
        })
        .level(log_level)
        .chain(std::io::stderr())
        .apply()?;

    if [args.major, args.minor, args.patch]
        .iter()
        .filter(|v| **v)
        .count()
        > 1
    {
        bail!("Can't set --major, --minor, --patch together");
    }

    let repo_type = detect_repo_type()?;
    debug!("Detected repo type: {:?}", repo_type);

    let on_default_branch = match repo_type {
        RepoType::Git => {
            let branch_name = git(&["branch", "--show-current"])?;
            ["main", "master"].contains(&branch_name.as_str())
        }
        RepoType::Jj => {
            // Check if '@' has 'main' bookmark
            let bookmarks = jj(&["log", "-r", "@", "-T", "bookmarks"])?;
            debug!("Current bookmarks: {}", bookmarks);
            bookmarks.contains("main")
        }
    };

    if [args.major, args.minor, args.patch, args.pre]
        .iter()
        .filter(|v| **v)
        .count()
        == 0
    {
        if on_default_branch {
            info!("No flags given, assuming patch");
            args.patch = true;
        } else {
            info!("No flags given, assuming pretag");
            args.pre = true;
        }
    }

    if args.no_push {
        warn!("Not going to push tag");
    }

    if on_default_branch && args.pre {
        error!("Pretags are only allowed on branches");
        bail!("branch/parameter missmatch");
    }

    if !on_default_branch && !args.pre {
        warn!("On branches other than main/master '--pre' is implied");
        args.pre = true;
    }

    // Get the commit to tag (for jj repos)
    let commit_to_tag = get_commit_to_tag(repo_type, on_default_branch)?;

    info!("Updating local tags via git");
    let _ = git(&["fetch", "--tags"])?;

    let github_token = std::env::var("GITHUB_TOKEN")
        .context("missing api tokent ($GITHUB_TOKEN) to talk to github")?;

    let url = git(&["config", "--get", "remote.origin.url"])?;
    let extract_repo_name = Regex::new(r#"^([^:]+):([^/]+)/([^\.]+)(.git)?$"#).unwrap();

    let Some(caps) = extract_repo_name.captures(&url) else {
        bail!("Unable to parse repository URL: {}", url);
    };

    let owner = &caps[2];
    let name = &caps[3];
    info!("Going to fetch tags for {owner}/{name}");

    #[derive(SerJson)]
    struct GqlRequest<'a> {
        query: &'static str,
        variables: Variables<'a>,
    }

    #[derive(SerJson)]
    struct Variables<'a> {
        owner: &'a str,
        name: &'a str,
    }

    let query = indoc::indoc! {r#"
          query ($owner: String!, $name: String!, $endCursor: String) {
            repository(owner: $owner, name: $name) {
              refs(refPrefix: "refs/tags/", first: 50, after: $endCursor, orderBy:{field: TAG_COMMIT_DATE, direction: DESC }) {
                 pageInfo {
                  endCursor
                  hasNextPage
                }
                nodes {
                  name
                }
              }
            }
          }
        "#
    };

    let body = nanoserde::SerJson::serialize_json(&GqlRequest {
        query,
        variables: Variables {
            owner,
            name,
        },
    });

    debug!("The query is:\n{body}");

    info!("Fetching tags...");
    let response = ureq::post("https://api.github.com/graphql")
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {github_token}"))
        .set("X-GitHub-Api-Version", "2022-11-28")
        .send_bytes(body.as_bytes())?;

    if response.status() != 200 {
        error!(
            "Failed to get tags from github: {}",
            response.into_string()?
        );
        return Ok(());
    }
    let body = response.into_string().unwrap();

    let gql: Graphql =
        nanoserde::DeJson::deserialize_json(&body).context("to extract ref data from response")?;

    info!(
        "Going to check for {n} tags for compatibility",
        n = gql.data.repository.refs.nodes.len()
    );

    let mut tags: Vec<_> = gql
        .data
        .repository
        .refs
        .nodes
        .into_iter()
        .filter_map(|name| Tag::try_from(name.name).ok())
        .filter(|tag| tag.prefix == args.prefix)
        .collect();

    tags.sort();

    info!("Left with {n} repos afterwards.", n = tags.len());
    // let mut proper_releases: Vec<_> = tags.into_iter().filter(|tag| tag.is_release()).collect();

    info!(
        "Considered tags: {}",
        tags.iter()
            .map(|t| t.to_string())
            .collect::<Vec<_>>()
            .join(",\n")
    );

    let latest_tag: Tag = tags.pop().unwrap_or(Tag::initial());
    let next = increment_tag(latest_tag, &args);
    let prompt_theme = ColorfulTheme::default();
    'tag: loop {
        let t: Tag = Input::with_theme(&prompt_theme)
            .with_prompt("Next tag")
            .default(next.to_string())
            .validate_with(|input: &String| Tag::try_from(input.as_str()).map(|_| ()))
            .interact_text()
            .map_err(|e| anyhow::anyhow!(e))
            .and_then(Tag::try_from)?;

        info!("Creating tag {t}");

        let tag_result = if let Some(ref commit) = commit_to_tag {
            // For jj repos, tag the specific commit
            git(&["tag", "-a", "-m", "test", t.to_string().as_str(), commit])
        } else {
            // For git repos, tag HEAD (default behavior)
            git(&["tag", "-a", "-m", "test", t.to_string().as_str()])
        };

        match tag_result {
            Ok(_) => {
                info!("Successfully tagged {t}, pushing.");

                if !args.no_push {
                    git(&["push", "--tags"])?;
                    info!("Done pushing tag");
                }
                break 'tag;
            }
            Err(e) => {
                error!("Failed to create tag {e}");
                if e.to_string().contains("already exists") {
                    let try_again = Confirm::with_theme(&prompt_theme)
                        .with_prompt("Tag already exists. Try a different one?")
                        .interact()?;

                    if !try_again {
                        break 'tag;
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, DeJson)]
struct Graphql {
    data: Data,
}

#[derive(Debug, DeJson)]
struct Data {
    repository: Repository,
}

#[derive(Debug, DeJson)]
struct Repository {
    refs: Refs,
}

#[derive(Debug, DeJson)]
struct Refs {
    nodes: Vec<Name>,
}

#[derive(Debug, DeJson)]
struct Name {
    name: String,
}

fn git(args: &[&str]) -> Result<String, anyhow::Error> {
    let output = Command::new("git").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let args = args.join(" ");
        anyhow::bail!(format!("git {args} failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(stdout)
}

fn jj(args: &[&str]) -> Result<String, anyhow::Error> {
    let output = Command::new("jj").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let args = args.join(" ");
        anyhow::bail!(format!("jj {args} failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(stdout)
}

#[derive(Debug, Clone, Copy)]
enum RepoType {
    Git,
    Jj,
}

fn detect_repo_type() -> Result<RepoType, anyhow::Error> {
    // Check for .jj directory
    if std::path::Path::new(".jj").exists() {
        return Ok(RepoType::Jj);
    }

    // Check for .git directory
    if std::path::Path::new(".git").exists() {
        return Ok(RepoType::Git);
    }

    bail!("Not in a git or jj repository")
}

fn get_commit_to_tag(repo_type: RepoType, on_default_branch: bool) -> Result<Option<String>, anyhow::Error> {
    match repo_type {
        RepoType::Git => {
            // For git, we don't need to specify a commit (tags HEAD by default)
            Ok(None)
        }
        RepoType::Jj => {
            // For jj, we need to get the git commit id
            let commit_id = if on_default_branch {
                // Tag trunk() when on main
                info!("On main bookmark, tagging trunk()");
                jj(&["log", "-r", "trunk()", "-T", "commit_id", "--no-graph"])?
            } else {
                // Tag @ for pretags
                info!("Not on main bookmark, tagging @");
                jj(&["log", "-r", "@", "-T", "commit_id", "--no-graph"])?
            };
            debug!("Commit to tag: {}", commit_id);
            Ok(Some(commit_id))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct Tag {
    prefix: Option<String>,
    v: semver::Version,
}

impl Tag {
    fn initial() -> Self {
        Self {
            prefix: None,
            v: semver::Version::parse("0.1.0").unwrap(),
        }
    }

    fn is_prelease(&self) -> bool {
        !self.v.pre.is_empty()
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(prefix) = &self.prefix {
            f.write_str(prefix)?;
            f.write_char('@')?;
        }
        f.write_char('v')?;
        self.v.fmt(f)
    }
}

impl TryFrom<&str> for Tag {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.to_string().try_into()
    }
}

impl TryFrom<String> for Tag {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (prefix, tag) = if let Some((prefix, tag)) = value.split_once('@') {
            (Some(prefix.to_string()), tag)
        } else {
            (None, value.as_str())
        };

        let raw = tag.strip_prefix("v").unwrap_or(&value);
        let v: semver::Version = raw
            .parse()
            .map_err(|e| anyhow!("Failed to parse tag: {e}"))?;

        Ok(Tag { prefix, v })
    }
}

fn increment_tag(before: Tag, params: &Args) -> Tag {
    let mut next_v = before.v.clone();
    next_v.build = BuildMetadata::from_str("").unwrap();
    if params.major {
        next_v.major += 1;
        next_v.minor = 0;
        next_v.patch = 0;
        next_v.pre = if params.pre {
            next_prerelease(&before.v.pre)
        } else {
            Prerelease::from_str("").unwrap()
        };
    }
    if params.minor {
        next_v.minor += 1;
        next_v.patch = 0;
        next_v.pre = if params.pre {
            next_prerelease(&before.v.pre)
        } else {
            Prerelease::from_str("").unwrap()
        };
    }
    if params.patch {
        if !before.is_prelease() {
            next_v.patch += 1;
        }
        next_v.pre = Prerelease::from_str("").unwrap();
    }
    if params.pre {
        if before.is_prelease() {
            next_v.pre = next_prerelease(&before.v.pre);
        } else {
            if !(params.major || params.minor || params.patch) {
                next_v.patch += 1;
                next_v.pre = Prerelease::from_str("pre0").unwrap();
            }
        }
    }
    Tag {
        prefix: before.prefix.clone(),
        v: next_v,
    }
}

fn next_prerelease(before: &Prerelease) -> Prerelease {
    let prerelase = before.as_str();
    let attempt: i32 = prerelase
        .strip_prefix("pre")
        .and_then(|raw| raw.parse::<i32>().ok())
        .map(|n| n + 1)
        .unwrap_or(0);

    Prerelease::from_str(&format!("pre{attempt}")).unwrap()
}

#[cfg(test)]
mod tests {
    use crate::{increment_tag, Tag};

    #[test]
    fn bumps_the_major_version() {
        let before = Tag::try_from("v0.1.0").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: true,
                minor: false,
                patch: false,
                pre: false,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v1.0.0").unwrap());
    }

    #[test]
    fn bumps_the_minor_version() {
        let before = Tag::try_from("v0.1.1").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: true,
                patch: false,
                pre: false,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.2.0").unwrap());
    }

    #[test]
    fn bumps_the_patch_version() {
        let before = Tag::try_from("v0.1.1").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: false,
                patch: true,
                pre: false,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.1.2").unwrap());
    }

    #[test]
    fn bumps_to_the_next_pretag() {
        let before = Tag::try_from("v0.1.1-pre5").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: false,
                patch: false,
                pre: true,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.1.1-pre6").unwrap());
    }

    #[test]
    fn bumps_to_the_version_without_pretag_suffix() {
        let before = Tag::try_from("v0.1.1-pre5").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: false,
                patch: true,
                pre: false,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.1.1").unwrap());
    }

    #[test]
    fn when_not_a_pretag_bumps_the_patch_as_well() {
        let before = Tag::try_from("v0.1.1").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: false,
                patch: false,
                pre: true,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.1.2-pre0").unwrap());
    }

    #[test]
    fn can_choose_to_bump_any_other_field_with_pretag() {
        let before = Tag::try_from("v0.1.1").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: false,
                minor: true,
                patch: false,
                pre: true,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v0.2.0-pre0").unwrap());

        let before = Tag::try_from("v0.1.1").unwrap();
        let after = increment_tag(
            before,
            &crate::Args {
                major: true,
                minor: false,
                patch: false,
                pre: true,
                ..Default::default()
            },
        );

        assert_eq!(after, Tag::try_from("v1.0.0-pre0").unwrap());
    }
}
