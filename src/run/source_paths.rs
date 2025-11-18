use anyhow::{Context, Result};
use gimli::{self, DwarfSections, EndianSlice, Reader, RunTimeEndian, SectionId};
use memmap2::Mmap;
use object::{Object, ObjectSection};
use path_clean::PathClean;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn extract_source_paths(debug_file: &Path, cwd: &Path) -> Result<Vec<PathBuf>> {
    let file = fs::File::open(debug_file)
        .with_context(|| format!("failed to open debug file: {}", debug_file.display()))?;
    let mmap = unsafe { Mmap::map(&file)? };
    let bytes: &'static [u8] = Box::leak(mmap.to_vec().into_boxed_slice());

    let obj = object::File::parse(bytes)
        .with_context(|| format!("failed to parse object: {}", debug_file.display()))?;

    let sections = DwarfSections::load(|id: SectionId| -> io::Result<Vec<u8>> {
        match obj.section_by_name(id.name()) {
            Some(s) => Ok(s
                .uncompressed_data()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .into_owned()),
            None => Ok(Vec::new()),
        }
    })?;
    let dwarf = sections.borrow(|bytes| EndianSlice::new(bytes, RunTimeEndian::Little));

    let result = extract_from_debug_line(cwd, &dwarf)?;

    Ok(result)
}

pub fn extract_from_debug_line<R: Reader<Offset = usize>>(
    cwd: &Path,
    dwarf: &gimli::Dwarf<R>,
) -> Result<Vec<PathBuf>> {
    let mut units = dwarf.units();

    while let Some(header) = units.next()? {
        let unit = dwarf.unit(header)?;

        if let Some(line_prog) = unit.line_program.clone() {
            let header = line_prog.header();

            for dir_attr in header.include_directories() {
                let cow = dwarf.attr_string(&unit, dir_attr.clone())?;
                let dir_str = cow.to_string_lossy()?.into_owned();

                let dir_path = PathBuf::from(&dir_str).clean();

                if dir_path.is_absolute() {
                    continue;
                }

                let resolved = cwd.join(&dir_path).clean();

                if resolved.exists() && resolved.is_dir() {
                    let files: Vec<PathBuf> = std::fs::read_dir(&resolved)?
                        .filter_map(|entry| entry.ok())
                        .map(|entry| entry.path())
                        .filter(|p| p.is_file())
                        .map(|p| {
                            p.strip_prefix(cwd)
                                .map(|rel| PathBuf::from("./").join(rel))
                                .unwrap_or(p) // fallback to absolute path if strip fails
                        })
                        .collect();

                    return Ok(files);
                }
            }
        }
    }

    Ok(vec![])
}
