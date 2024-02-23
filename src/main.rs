use std::fmt::{Display, Write};
use std::process::Command;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context};
use argh::FromArgs;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input};
use log::{error, info};
use nanoserde::DeJson;
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
}

impl Default for Args {
    fn default() -> Self {
        Self {
            major: false,
            minor: false,
            patch: false,
            pre: true,
            verbose: false,
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    let mut args: Args = argh::from_env();
    if [args.major, args.minor, args.patch]
        .iter()
        .filter(|v| **v)
        .count()
        > 1
    {
        bail!("Can't set --major, --minor, --patch together");
    }

    if [args.major, args.minor, args.patch, args.pre]
        .iter()
        .filter(|v| **v)
        .count()
        == 0
    {
        info!("No flags given, assuming pretag");
        args.pre = true;
    }

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

    let branch_name = git(&["branch", "--show-current"])?;
    let on_default_branch = ["main", "master"].contains(&branch_name.as_str());

    if on_default_branch && args.pre {
        error!("Pretags are only allowed on branches");
        bail!("branch/parameter missmatch");
    }

    if !on_default_branch && !args.pre {
        error!("On branches other than main/master you have to use --pre");
        bail!("branch/parameter missmatch");
    }

    info!("Updating local tags via git");
    let _ = git(&["fetch", "--tags"])?;

    let github_token = std::env::var("GITHUB_TOKEN")
        .context("missing api tokent ($GITHUB_TOKEN) to talk to github")?;

    let url = git(&["config", "--get", "remote.origin.url"])?;
    let extract_repo_name = Regex::new(r#"^([^:]+):TrueLayer/([^\.]+).git$"#).unwrap();

    let Some(caps) = extract_repo_name.captures(&url) else {
        bail!("Repo does not seem to be a TrueLayer one");
    };

    let name = &caps[2];
    info!("Going to fetch tags for {name}");

    info!("Fetching tags...");
    let response = ureq::get(&format!(
        "https://api.github.com/repos/TrueLayer/{name}/git/refs/tags/v"
    ))
    .set("Accept", "application/vnd.github+json")
    .set("Authorization", &format!("Bearer {github_token}"))
    .set("X-GitHub-Api-Version", "2022-11-28")
    .call()?;

    if response.status() != 200 {
        error!(
            "Failed to get tags from github: {}",
            response.into_string()?
        );
        return Ok(());
    }
    let body = response.into_string().unwrap();

    let refs: Vec<Ref> =
        nanoserde::DeJson::deserialize_json(&body).context("to extract ref data from response")?;

    info!(
        "Going to check for {n} tags for compatibility",
        n = refs.len()
    );

    let mut tags: Vec<_> = refs
        .into_iter()
        .filter_map(|raw| {
            let raw = raw.git_ref;
            let tag = raw.strip_prefix("refs/tags/").unwrap_or(&raw);
            Tag::try_from(tag).ok()
        })
        .collect();

    tags.sort();

    info!("Left with {n} repos afterwards.", n = tags.len());
    let mut proper_releases: Vec<_> = tags.into_iter().filter(|tag| tag.is_release()).collect();

    let last_release: Tag = proper_releases.pop().unwrap_or(Tag::initial());
    let next = increment_tag(last_release, &args);
    let prompt_theme = ColorfulTheme::default();
    'tag: loop {
        let version: String = Input::with_theme(&prompt_theme)
            .with_prompt("Next tag")
            .default(next.to_string())
            .validate_with(|input: &String| Tag::try_from(input.as_str()).map(|_| ()))
            .interact_text()?;

        let t = Tag::try_from(version)?;

        info!("Creating tag {t}");

        match git(&["tag", t.to_string().as_str()]) {
            Ok(_) => {
                info!("Successfully tagged {t}, pushing.");
                git(&["push", "--tags"])?;
                info!("Done");
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
struct Ref {
    #[nserde(rename = "ref")]
    git_ref: String,
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

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
struct Tag(semver::Version);

impl Tag {
    fn is_release(&self) -> bool {
        self.0.pre.is_empty()
    }

    fn initial() -> Self {
        Self(semver::Version::parse("0.1.0").unwrap())
    }

    fn is_prelease(&self) -> bool {
        !self.0.pre.is_empty()
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('v')?;
        self.0.fmt(f)
    }
}

impl TryFrom<&str> for Tag {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let raw = value.strip_prefix("v").unwrap_or(&value);
        raw.parse()
            .map(Tag)
            .map_err(|e| anyhow!("Failed to parse tag: {e}"))
    }
}

impl TryFrom<String> for Tag {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let raw = value.strip_prefix("v").unwrap_or(&value);
        raw.parse()
            .map(Tag)
            .map_err(|e| anyhow!("Failed to parse tag: {e}"))
    }
}

fn increment_tag(before: Tag, params: &Args) -> Tag {
    let mut next = before.0.clone();
    next.build = BuildMetadata::from_str("").unwrap();
    if params.major {
        next.major += 1;
        next.minor = 0;
        next.patch = 0;
        next.pre = if params.pre {
            next_prerelease(&before.0.pre)
        } else {
            Prerelease::from_str("").unwrap()
        };
    }
    if params.minor {
        next.minor += 1;
        next.patch = 0;
        next.pre = if params.pre {
            next_prerelease(&before.0.pre)
        } else {
            Prerelease::from_str("").unwrap()
        };
    }
    if params.patch {
        next.patch += 1;
        next.pre = Prerelease::from_str("").unwrap();
    }
    if params.pre {
        if before.is_prelease() {
            next.pre = next_prerelease(&before.0.pre);
        } else {
            if !(params.major || params.minor || params.patch) {
                next.patch += 1;
                next.pre = Prerelease::from_str("pre0").unwrap();
            }
        }
    }
    Tag(next)
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
