mod build;

use anyhow::Result;
pub use build::{build, BuildArgs};

pub fn seer_build(args: BuildArgs) -> Result<()> {
	build(args)
}