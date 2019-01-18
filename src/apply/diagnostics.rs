// Licensed under the MIT license. See LICENSE.md

//! This module contains functions for diagnosing and reporting patch apply
//! failure. Performance is less important for these functions, since they are
//! called only few times when there was application failure. The priority is
//! to provide user-friendly report.

use std::collections::HashMap;
use std::io::{self, Write};
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use colored::*;

use crate::apply::common::*;
use crate::line::Line;
use crate::modified_file::ModifiedFile;
use crate::patch::{HunkApplyReport, PatchDirection};
use crate::patch::unified::writer::UnifiedPatchHunkWriter;


/// Try if the patch would apply with some fuzz. It doesn't do any permanent changes.
pub fn test_apply_with_fuzzes<'a, H: BuildHasher, L: Line<'a>>(
    patch_status: &PatchStatus<L>,
    modified_files: &HashMap<PathBuf, ModifiedFile<L>, H>)
    -> Option<usize>
{
    let file = modified_files.get(&patch_status.final_filename).unwrap(); // NOTE(unwrap): It must be there, otherwise we got bad modified_files, which would be bug.

    // Make our own copy for experiments
    let mut file = file.clone();

    // Rollback the failed application
    patch_status.file_patch.rollback(&mut file, PatchDirection::Forward, &patch_status.report);

    let current_fuzz = patch_status.report.fuzz();
    let max_fuzz = patch_status.file_patch.max_useable_fuzz();

    if current_fuzz >= max_fuzz {
        return None;
    }

    for fuzz in (current_fuzz + 1)..=max_fuzz {
        // Make another copy for test application
        let mut file = file.clone();

        let report = patch_status.file_patch.apply(&mut file, PatchDirection::Forward, fuzz);

        if report.ok() {
            return Some(fuzz);
        }
    }

    None
}

pub fn test_apply_after_reverting_other<'a, L: Line<'a>, H: BuildHasher>(
    failed_patch_status: &PatchStatus<L>,
    suspect_patch_status: &PatchStatus<L>,
    modified_files: &HashMap<PathBuf, ModifiedFile<L>, H>)
    -> bool
{
    let file = modified_files.get(&failed_patch_status.final_filename).unwrap(); // NOTE(unwrap): It must be there, otherwise we got bad modified_files, which would be bug.

    // Make our own copy for experiments
    let mut file = file.clone();

    // Rollback the failed application
    failed_patch_status.file_patch.rollback(&mut file, PatchDirection::Forward, &failed_patch_status.report);

    // Revert the suspect
    let revert_report = suspect_patch_status.file_patch.apply(&mut file, PatchDirection::Revert, suspect_patch_status.report.fuzz());
    if revert_report.failed() {
        // If we couldn't even revert the suspect, we can't test anything
        return false;
    }

    // Try to apply our failed patch again
    let apply_report = failed_patch_status.file_patch.apply(&mut file, PatchDirection::Forward, failed_patch_status.report.fuzz());

    // Report whether it would apply ok now
    apply_report.ok()
}

/// Render a report into `writer` about why the `broken_patch_index` failed to
/// apply.
pub fn analyze_patch_failure<
    'arena,
    'config,
    'applied_patches,
    'modified_files,
    L: Line<'arena>,
    H: BuildHasher,
    W: Write>
(
    broken_patch_index: usize,
    applied_patches: &'applied_patches Vec<PatchStatus<'config, L>>,
    modified_files: &'modified_files HashMap<PathBuf, ModifiedFile<L>, H>,
    writer: &mut W)
    -> Result<(), io::Error>
{
    for patch_status in applied_patches.iter().rev() {
        if patch_status.index != broken_patch_index {
            break;
        }

        write!(writer, "  {} {} ", "File".yellow(), patch_status.target_filename.display())?;

        if patch_status.report.ok() {
            writeln!(writer, "{}", "OK".bright_green().bold())?;
        } else {
            writeln!(writer, "{}", "FAILED".bright_red().bold())?;

            for (i, hunk_report) in patch_status.report.hunk_reports().iter().enumerate() {
                write!(writer, "    {} #{}: ", "Hunk".yellow(), i + 1)?;

                match hunk_report {
                    HunkApplyReport::Applied { offset, .. } => {
                        write!(writer, "{}", "OK".bright_green().bold())?;

                        if *offset != 0 {
                            write!(writer, " with offset {}", offset)?;
                        } else {
                            write!(writer, "     ")?; // Spaces to balance width of "failed " and "skipped"
                        }
                    }

                    HunkApplyReport::Failed => {
                        write!(writer, "{}", "FAILED ".bright_red().bold())?;
                    }

                    HunkApplyReport::Skipped => {
                        unreachable!(); // This should never happen here. Hunk can be skipped only during rollback.
                    }
                }

                let mut buf = Vec::<u8>::new();
                patch_status.file_patch.hunks()[i].write_header_to(&mut buf)?;
                writeln!(writer, "\t{}", String::from_utf8_lossy(&buf).bright_blue())?;
            }

            // Find which other patches touched this file
            let mut other_patches = Vec::<(&Path, bool)>::new();
            for other_patch_status in applied_patches.iter() {
                if other_patch_status.index >= broken_patch_index {
                    break;
                }

                // TODO: Follow thru renames?

                if other_patch_status.target_filename == patch_status.target_filename {
                    let is_suspect = test_apply_after_reverting_other(patch_status, other_patch_status, modified_files);

                    other_patches.push((other_patch_status.patch_filename, is_suspect));
                }
            }

            // Fuzz hint
            writeln!(writer)?;
            if let Some(working_fuzz) = test_apply_with_fuzzes(patch_status, modified_files) {
                write!(writer, "    {} Patch would apply on this file with fuzz {}", "hint:".purple(), working_fuzz)?;
            } else {
                write!(writer, "    {} Patch would not apply on this file with any fuzz", "hint:".purple())?;
            }
            writeln!(writer)?;

            // Other patches hint
            writeln!(writer)?;
            write!(writer, "    {} ", "hint:".purple())?;

            if other_patches.is_empty() {
                writeln!(writer, "No previous patches touched this file.")?;
            } else {
                writeln!(writer, "{} previous patches touched this file:", other_patches.len())?;

                let mut any_suspect = false;
                for (other_patch, is_suspect) in other_patches {
                    write!(writer, "      {}", other_patch.display())?;
                    if is_suspect {
                        write!(writer, " {}", " !".bright_red())?;
                        any_suspect = true;
                    }
                    writeln!(writer)?;
                }

                if any_suspect {
                    writeln!(writer)?;
                    writeln!(writer, "      {} = Reverting the patch fixes this failure.", "!".bright_red())?;
                }
            }

            writeln!(writer)?;
        }
    }

    Ok(())
}
