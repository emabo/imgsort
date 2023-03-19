use chrono::NaiveDate;
use exif::{DateTime as ExifDateTime, In, Reader, Value, Tag};
use std::fs::File;
use std::io::BufReader;



const PARSERS: [&'static str; 11] = ["%Y:%m:%d %H:%M:%S",
                                     "IMG-%Y%m%d-WA%f",    // IMG-20160807-WA0001.jpg
                                     "PANO_%Y%m%d_%H%M%S", // PANO_20190427_115542.jpg
                                     "IMG_%Y%m%d_%H%M%S",  // IMG_20190426_102645.jpg
                                     "IMG_%Y-%m-%d-%f",    // IMG_2016-08-16-19343585.png
                                     "%Y%m%d_%H%M%S",      // 20160824_123058.jpg
                                     "VID-%Y%m%d-WA%f",    // VID-20200208-WA0000.mp4
                                     "VID_%Y%m%d_%H%M%S",  // VID_20190428_161901.mp4
                                     "%Y%m%d_%H%M%S_%f",   // 20211208_104956_01.mp4
                                     "%Y%m%d-WA%f",        // 20150511-WA0003.jpg
                                     "%Y-%m-%d %H.%M.%S"]; // 2015-06-04 17.30.00.jpg

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
    pub verbose: bool
}

pub fn extract_date(filename: &str, verbose: bool) -> Result<NaiveDate, String> {
    let mut date = NaiveDate::from_ymd_opt(2000,1,1).unwrap();
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
                        date = NaiveDate::from_ymd_opt(datetime.year.into(), datetime.month.into(), datetime.day.into()).unwrap();
                    }
                },
                _ => {},
            }
        }
    }
    if found {
        Ok(date)
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
