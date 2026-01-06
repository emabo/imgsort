#[macro_use]
extern crate clap;
extern crate exif;
extern crate sha1;


use std::io;
use std::fs::{self, File, DirEntry};
use std::path::Path;
use sha1::{Sha1, Digest};
use chrono::Datelike;
use chrono::Timelike;
use chrono::NaiveDate;

use imgsort::{extract_date, extract_date_from_filename, Stats, Options, ExtensionCount};


fn find_existing_date_dir(base_path: &Path, year_path: &str, date_prefix: &str) -> Option<String> {
    // Search for a directory matching YEAR_MONTH_DAY or YEAR_MONTH_DAY.suffix
    let year_dir = base_path.join(year_path);

    if !year_dir.exists() || !year_dir.is_dir() {
        return None;
    }

    if let Ok(entries) = fs::read_dir(&year_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                if entry.file_type().unwrap().is_dir() {
                    let dir_name = entry.file_name();
                    let dir_name_str = dir_name.to_str().unwrap();

                    // Check if the directory starts with the date prefix
                    if dir_name_str == date_prefix {
                        // Exact match
                        return Some(dir_name_str.to_string());
                    } else if dir_name_str.starts_with(&format!("{}.", date_prefix)) {
                        // Match with suffix after dot
                        return Some(dir_name_str.to_string());
                    }
                }
            }
        }
    }

    None
}

fn compute_hash(filename: &str) -> String {
    let mut file = File::open(filename).unwrap();
    let mut hasher = Sha1::new();

    io::copy(&mut file, &mut hasher).unwrap();

    return format!("{:x}", hasher.finalize());
}

fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry, &Options, &mut Stats, &mut ExtensionCount), options: &Options, file_stats: &mut Stats, ext_count: &mut ExtensionCount, depth: u32) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && (depth < options.max_depth || options.recursive) {
                visit_dirs(&path, cb, &options, file_stats, ext_count, depth + 1)?
            } else {
                cb(&entry, &options, file_stats, ext_count);
            }
        }
    }
    Ok(())
}

fn compute_file(entry: &DirEntry, opts: &Options, stats: &mut Stats, ext_count: &mut ExtensionCount) {
    let path_tmp = entry.path();
    let full_filename_from = path_tmp.to_str().unwrap();
    let mut date1 = NaiveDate::from_ymd_opt(2000,1,1).unwrap().and_hms_opt(0,0,0).unwrap();
    let mut date2 = NaiveDate::from_ymd_opt(2000,1,1).unwrap();
    let date1_present: bool;
    let date2_present: bool;

    if entry.file_type().unwrap().is_dir() {
        return;
    }

    let path_from = Path::new(&full_filename_from);

    // If only counting extensions, just track and return
    if opts.count_extensions {
        let ext = path_from
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        ext_count.add(&ext);
        stats.inc_tot();
        return;
    }

    println!("\nFilename: {}", full_filename_from);

    stats.inc_tot();

    let mut filename_to = path_from
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let original_filename_base = filename_to.clone();
    let mut renamed_due_to_conflict = false;
    if opts.verbose { println!("Filename prefix: {}", filename_to) }

    let result1 = extract_date(&full_filename_from, opts.verbose);
    match result1 {
        Ok(res) => {
            date1_present = true;
            date1 = res;
        },
        Err(e) => {
            date1_present = false;
            println!("WARN: {}", e);
        }
    }
    let result2 = extract_date_from_filename(&filename_to, opts.verbose);
    match result2 {
        Ok(res) => {
            date2_present = true;
            date2 = res;
        },
        Err(e) => {
            date2_present = false;
            println!("WARN: {}", e);
        }
    }
    let chosen_date = if date1_present && date2_present && date1.date() != date2 {
        if opts.prefer_metadata_on_conflict {
            if opts.verbose { println!("Date conflict: using metadata date and renaming with %Y%m%d_%H%M%S"); }
            filename_to = format!(
                "{:04}{:02}{:02}_{:02}{:02}{:02}",
                date1.year(), date1.month(), date1.day(),
                date1.hour(), date1.minute(), date1.second()
            );
            if filename_to != original_filename_base {
                renamed_due_to_conflict = true;
            }
            Some(date1)
        } else {
            stats.inc_skipped();
            println!("ERROR: Date from file and from filename are different, skipping image");
            None
        }
    } else if !date1_present && !date2_present {
        stats.inc_skipped();
        println!("ERROR: Cannot extract date from file or filename");
        None
    } else {
        Some(if date1_present { date1 } else { date2.and_hms_opt(0,0,0).unwrap() })
    };

    if let Some(date_time) = chosen_date {
        let date = date_time.date();

        let orig_path = Path::new(&opts.dir_to);
        let year_dir = format!("{:04}", date.year());
        let date_prefix = format!("{:04}_{:02}_{:02}", date.year(), date.month(), date.day());

        // Check if a directory with this prefix already exists (with or without suffix)
        let dir_name = match find_existing_date_dir(&orig_path, &year_dir, &date_prefix) {
            Some(existing_dir) => {
                if opts.verbose { println!("Found existing directory: {}", existing_dir) }
                existing_dir
            },
            None => {
                // No existing directory, use standard name
                date_prefix.clone()
            }
        };

        let extension = path_from
            .extension()
            .map(|e| e.to_string_lossy().into_owned());
        let base_dir = orig_path.join(&year_dir).join(&dir_name);

        if !base_dir.exists() {
            if !opts.dry_run { 
                if opts.verbose { println!("Create new directory: {}", base_dir.display()) }
                fs::create_dir_all(&base_dir).unwrap_or_else(|e| panic!("ERROR: creating dir: {}", e));
            }
        }
        let mut counter = 0;
        let mut done = false;
        while !done {
            let candidate_name = if counter > 0 {
                let new_filename = format!("{}_{:02}", filename_to, counter);
                if opts.verbose { println!("New filename: {}", new_filename); }
                new_filename
            } else {
                filename_to.clone()
            };

            let final_name = if let Some(ext) = &extension {
                format!("{}.{}", candidate_name, ext)
            } else {
                candidate_name.clone()
            };

            let full_filename_to = base_dir.join(&final_name);

            println!("Destination path: {}", full_filename_to.display());
            if full_filename_to.exists() {
                if opts.verbose { println!("File {} already exists", full_filename_to.display()) }

                let hash = compute_hash(full_filename_from);
                let new_hash = compute_hash(full_filename_to.to_str().unwrap());
                if hash == new_hash {
                    if opts.verbose { println!("The two files are equal") }
                    if !opts.copy {
                        if opts.verbose { println!("Deleting {}", full_filename_from) } 
                        if !opts.dry_run {
                            fs::remove_file(full_filename_from).unwrap_or_else(|e| panic!("ERROR: removing file: {}", e));
                        }
                    }
                    stats.inc_already_present();
                    done = true;
                }
                else {
                    if opts.verbose { println!("The two files are different") }
                    counter += 1;
                }
            }
            else {
                if counter > 0 {
                    if opts.verbose { println!("Renaming file from {} to {}", full_filename_from, full_filename_to.display()) }
                    stats.inc_renamed();
                }
                else if renamed_due_to_conflict && full_filename_to.file_name() != path_from.file_name() {
                    if opts.verbose { println!("Renaming file (date conflict) from {} to {}", full_filename_from, full_filename_to.display()) }
                    stats.inc_renamed();
                    renamed_due_to_conflict = false;
                }
                if opts.copy {
                    if opts.verbose { println!("Copy {} to {}", full_filename_from, full_filename_to.display()) }
                    if !opts.dry_run {
                        fs::copy(full_filename_from, &full_filename_to).unwrap_or_else(|e| panic!("ERROR: copying file: {}", e));
                        stats.inc_copied();
                    }
                }
                else {
                    if opts.verbose { println!("Move {} to {}", full_filename_from, full_filename_to.display()) }
                    if !opts.dry_run {
                        fs::rename(full_filename_from, &full_filename_to).unwrap_or_else(|e| {
                            println!("ERROR: Cannot rename file: {}, trying to move it", e);
                            println!("Try to move it");
                            fs::copy(full_filename_from, &full_filename_to).unwrap_or_else(|e| panic!("ERROR: copying file: {}", e));
                            fs::remove_file(full_filename_from).unwrap_or_else(|e| panic!("ERROR: removing file: {}", e));
                        });
                        stats.inc_moved();
                    }
                }
                done = true;
            }
        }
    }
}

fn main() {
    let matches = clap_app!(imgsort =>
        (version: "1.0.0")
        (about: "Sort images and videos in directories using Exif creation date and filename")
        (@arg from: -f --from +takes_value "Directory where getting images")
        (@arg to: -t --to +takes_value "Directory where moving/copying images")
        (@arg copy: -c --copy "Copy instead of moving images")
        (@arg dry_run: -d --dry_run "Dry run without touching files")
        (@arg recursive: -r --recursive "Recursively visit subdirectories")
        (@arg max_depth: -m --max_depth +takes_value "Visit maximum max_depth levels in recursion")
        (@arg verbose: -v --verbose "Print information verbosely")
        (@arg prefer_metadata: --prefer_metadata "On date conflicts, use metadata date and rename with %Y%m%d_%H%M%S")
        (@arg count_extensions: --count_extensions "Count files by extension")
    ).get_matches();
 
    let mut options = Options { dir_from: "".to_string(), dir_to: "".to_string(), copy: true, dry_run: false, recursive: false, max_depth: 0, verbose: false, prefer_metadata_on_conflict: false, count_extensions: false };
    options.dir_from = matches.value_of("from").unwrap_or(".").to_string();
    options.dir_to = matches.value_of("to").unwrap_or(".").to_string();
    options.copy = matches.is_present("copy");
    options.dry_run = matches.is_present("dry_run");
    options.recursive = matches.is_present("recursive");
    options.max_depth = value_t!(matches.value_of("max_depth"), u32).unwrap_or(0);
    options.verbose = matches.is_present("verbose");
    options.prefer_metadata_on_conflict = matches.is_present("prefer_metadata");
    options.count_extensions = matches.is_present("count_extensions");

    let mut file_stats = Stats { tot: 0, copied: 0, moved: 0, renamed: 0, already_present: 0, skipped: 0 };
    let mut ext_count = ExtensionCount::new();

    println!("Value for from: {}", options.dir_from);
    let path = Path::new(&options.dir_from);
    let depth = 0;

    let _result = visit_dirs(path, &compute_file, &options, &mut file_stats, &mut ext_count, depth);

    file_stats.print_all();
    if options.count_extensions {
        ext_count.print();
    }
}
