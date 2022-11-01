use git2::Repository;
use anyhow::Context;

fn main() -> Result<(), anyhow::Error> {
    let repo = Repository::open_from_env().context("Couldnt open Repository")?;

    let remote = repo.find_remote("origin").context("No remote named 'origin'")?;



    println!("It worked");

    Ok(())
}
