use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use exif::{DateTime as ExifDateTime, In, Reader, Value, Tag};
use std::fs::File;
use std::io::BufReader;



const PARSERS: [&'static str; 14] = ["%Y:%m:%d %H:%M:%S",
                                     "IMG-%Y%m%d-WA%f",       // IMG-20160807-WA0001.jpg
                                     "IMG-%Y%m%d-WA%f_01",    // IMG-20160807-WA0001_01.jpg
                                     "IMG-%Y%m%d-WA%f_1",     // IMG-20160807-WA0001_1.jpg
                                     "IMG-%Y%m%d-WA%f_01_01", // IMG-20160807-WA0001_01_01.jpg
                                     "PANO_%Y%m%d_%H%M%S",    // PANO_20190427_115542.jpg
                                     "IMG_%Y%m%d_%H%M%S",     // IMG_20190426_102645.jpg
                                     "IMG_%Y-%m-%d-%f",       // IMG_2016-08-16-19343585.png
                                     "%Y%m%d_%H%M%S",         // 20160824_123058.jpg
                                     "VID-%Y%m%d-WA%f",       // VID-20200208-WA0000.mp4
                                     "VID_%Y%m%d_%H%M%S",     // VID_20190428_161901.mp4
                                     "%Y%m%d_%H%M%S_%f",      // 20211208_104956_01.mp4
                                     "%Y%m%d-WA%f",           // 20150511-WA0003.jpg
                                     "%Y-%m-%d %H.%M.%S"];    // 2015-06-04 17.30.00.jpg

pub struct ExtensionCount {
    pub counts: std::collections::HashMap<String, u32>,
}

impl ExtensionCount {
    pub fn new() -> Self {
        ExtensionCount {
            counts: std::collections::HashMap::new(),
        }
    }

    pub fn add(&mut self, ext: &str) {
        *self.counts.entry(ext.to_string()).or_insert(0) += 1;
    }

    pub fn print(&self) {
        if self.counts.is_empty() {
            println!("No files found by extension.");
            return;
        }
        println!("\n\nFile count by extension:");
        let mut sorted: Vec<_> = self.counts.iter().collect();
        sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
        for (ext, count) in sorted {
            println!("  {}: {}", if ext.is_empty() { "(no extension)" } else { ext }, count);
        }
    }
}

pub struct Stats {
    pub tot: u32,
    pub copied: u32,
    pub moved: u32,
    pub renamed: u32,
    pub already_present: u32,
    pub skipped: u32
}

impl Stats {
    pub fn inc_tot(&mut self) {
        self.tot += 1;
    }

    pub fn inc_copied(&mut self) {
        self.copied += 1;
    }

    pub fn inc_moved(&mut self) {
        self.moved += 1;
    }

    pub fn inc_renamed(&mut self) {
        self.renamed += 1;
    }

    pub fn inc_already_present(&mut self) {
        self.already_present += 1;
    }

    pub fn inc_skipped(&mut self) {
        self.skipped += 1;
    }

    pub fn print_all(&self) {
        println!("\n\nTotal number of files: {}", &self.tot);
        println!("Skipped files: {}", &self.skipped);
        println!("Already present files: {}", &self.already_present);
        println!("Copied files: {}", &self.copied);
        println!("Moved files: {}", &self.moved);
        println!("Requiring renaming: {}", &self.renamed);
    }
}

pub struct Options {
    pub dir_from: String,
    pub dir_to: String,
    pub copy: bool,
    pub dry_run: bool,
    pub recursive: bool,
    pub max_depth: u32,
    pub verbose: bool,
    pub prefer_metadata_on_conflict: bool,
    pub count_extensions: bool,
}

pub fn extract_date(filename: &str, verbose: bool) -> Result<NaiveDateTime, String> {
    let default_date = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let default_time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let mut date_time = NaiveDateTime::new(default_date, default_time);
    let mut found = false;
    let file = File::open(filename).unwrap();

    if let Ok(exif) = Reader::new().read_from_container(&mut BufReader::new(&file)) {
        if verbose {
            // To obtain a string representation, `Value::display_as`
            // or `Field::display_value` can be used.  To display a value with its
            // unit, call `with_unit` on the return value of `Field::display_value`.
            let tag_list = [Tag::ExifVersion,
                Tag::PixelXDimension,
                Tag::XResolution,
                Tag::ImageDescription,
                Tag::DateTime];
            for &tag in tag_list.iter() {
                if let Some(field) = exif.get_field(tag, In::PRIMARY) {
                    println!("{}: {}", field.tag, field.display_value().with_unit(&exif));
                }
            }
        }

        // To parse a DateTime-like field, `DateTime::from_ascii` can be used.
        if let Some(field) = exif.get_field(Tag::DateTime, In::PRIMARY) {
            match field.value {
                Value::Ascii(ref vec) if !vec.is_empty() => {
                    if let Ok(datetime) = ExifDateTime::from_ascii(&vec[0]) {
                        found = true;
                        let date = NaiveDate::from_ymd_opt(datetime.year.into(), datetime.month.into(), datetime.day.into()).unwrap();
                        let time = NaiveTime::from_hms_opt(datetime.hour.into(), datetime.minute.into(), datetime.second.into()).unwrap_or(default_time);
                        date_time = NaiveDateTime::new(date, time);
                    }
                },
                _ => {},
            }
        }
    }
    if found {
        Ok(date_time)
    } else {
        Err("Date from file not found".to_string())
    }
}
 
pub fn extract_date_from_filename(filename: &str, verbose: bool) -> Result<NaiveDate, String> {
    let mut date = NaiveDate::from_ymd_opt(2000,1,1).unwrap();
    let mut found = false;

    for parser in PARSERS.iter() {
        let result = NaiveDate::parse_from_str(filename, parser);
        if result.is_ok() {
            if verbose { println!("Format found: {}", parser); }
            date = result.unwrap();
            found = true;
            break;
        }
    }
    if found {
        Ok(date)
    } else {
        Err("Date from filename not found".to_string())
    }
} 
