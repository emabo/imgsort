#[macro_use]
extern crate clap;
extern crate exif;
extern crate sha1;


use std::io;
use std::fs::{self, File, DirEntry};
use std::path::Path;
use sha1::{Sha1, Digest};
use chrono::Datelike;
use chrono::NaiveDate;

use imgsort::{extract_date, extract_date_from_filename, Stats, Options};


fn compute_hash(filename: &str) -> String { 
    let mut file = File::open(filename).unwrap();
    let mut hasher = Sha1::new();

    io::copy(&mut file, &mut hasher).unwrap();

    return format!("{:x}", hasher.finalize());
}

fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry, &Options, &mut Stats), options: &Options, file_stats: &mut Stats, depth: u32) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && (depth < options.max_depth || options.recursive) {
                visit_dirs(&path, cb, &options, file_stats, depth + 1)?;
            } else {
                cb(&entry, &options, file_stats);
            }
        }
    }
    Ok(())
}

fn compute_file(entry: &DirEntry, opts: &Options, stats: &mut Stats) {
    let path_tmp = entry.path();
    let full_filename_from = path_tmp.to_str().unwrap();
    let mut date1 = NaiveDate::from_ymd_opt(2000,1,1).unwrap();
    let mut date2 = NaiveDate::from_ymd_opt(2000,1,1).unwrap();
    let date1_present: bool;
    let date2_present: bool;

    if entry.file_type().unwrap().is_dir() {
        return;
    }

    println!("\nFilename: {}", full_filename_from);

    stats.inc_tot();

    let path_from = Path::new(&full_filename_from);
    let filename_to = path_from.file_stem().unwrap().to_str().unwrap();
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
    if date1_present && date2_present && date1 != date2 { 
        stats.inc_skipped();
        println!("ERROR: Date from file and from filename are different, skipping image");
    } else if !date1_present && !date2_present {
        stats.inc_skipped();
        println!("ERROR: Cannot extract date from file or filename");
    }
    else {
        let date;

        if date1_present {
            date = date1;
        } else {
            date = date2;
        }

        let orig_path = Path::new(&opts.dir_to);
        let new_dir = format!("{}/{:04}_{:02}_{:02}/", date.year(), date.year(), date.month(), date.day());
        let mut full_filename_to = orig_path.join(&new_dir);

        if !full_filename_to.exists() {
            if !opts.dry_run { 
                if opts.verbose { println!("Create new directory: {}", full_filename_to.display()) }
                fs::create_dir_all(&full_filename_to).unwrap_or_else(|e| panic!("ERROR: creating dir: {}", e));
            }
        }
        let mut counter = 0;
        let mut done = false;
        while !done {
            if counter > 0 {
                let extension = path_from.extension().unwrap();
                let new_filename = format!("{}_{:02}", filename_to, counter);
                full_filename_to.set_file_name(&new_filename);
                full_filename_to.set_extension(extension);
                println!("New filename: {}", new_filename);
            } else {
                full_filename_to.push(path_from.file_name().unwrap().to_str().unwrap());
            }

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
    ).get_matches();
 
    let mut options = Options { dir_from: "".to_string(), dir_to: "".to_string(), copy: true, dry_run: false, recursive: false, max_depth: 0, verbose: false };
    options.dir_from = matches.value_of("from").unwrap_or(".").to_string();
    options.dir_to = matches.value_of("to").unwrap_or(".").to_string();
    options.copy = matches.is_present("copy");
    options.dry_run = matches.is_present("dry_run");
    options.recursive = matches.is_present("recursive");
    options.max_depth = value_t!(matches.value_of("max_depth"), u32).unwrap_or(0);
    options.verbose = matches.is_present("verbose");

    let mut file_stats = Stats { tot: 0, copied: 0, moved: 0, renamed: 0, already_present: 0, skipped: 0 };


    println!("Value for from: {}", options.dir_from);
    let path = Path::new(&options.dir_from);
    let depth = 0;

    let _result = visit_dirs(path, &compute_file, &options, &mut file_stats, depth);

    file_stats.print_all();
}
