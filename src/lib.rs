mod build;
mod temp_file;

use anyhow::Result;
pub use build::{build, BuildArgs};

pub fn seer_build(args: BuildArgs) -> Result<()> {
	build(args)
}