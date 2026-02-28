mod build;

use anyhow::Result;
pub use build::{build, BuildArgs};

/// Runs the Seer build functionality, replicating the CLI build command.
/// Accepts `BuildArgs` and returns a Result.
pub fn seer_build(args: BuildArgs) -> Result<()> {
	build(args)
}