use anyhow::Result;
use cargo_metadata::camino::Utf8PathBuf;
use clap::command;
use clap::Arg;
use clap::ArgAction;
use ignore::WalkBuilder;
use indoc::indoc;
use log::error;
use log::info;
use std::f32::consts::E;
use std::ops::Not;
use thiserror::Error;

fn main() -> Result<()> {
    let matches = command!()
        .long_about(indoc! {
            "A simple tool for aligning code

            By default formated from the worksapce root if in a rust project
            
            For in-file usage docs, see https://github.com/MeGaGiGaGon/cargo-align"
        })
        .arg(
            Arg::new("ignore")
                .short('i')
                .long("ignore")
                .action(ArgAction::Append)
                .help("Globs to ignore")
                .long_help(indoc! {
                    "Add additional globs to be ignored for alignment
                    Can be passed in multiple times
                    Uses the same format as .gitignores"
                }),
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .action(ArgAction::Append)
                .help("Additional paths to format")
                .long_help(indoc! {
                    "Set additional paths to align
                    Can be passed in multiple times
                    The root ignores any restrictions in -i arguments or .gitignores
                    but they will be respected when traversing directory contents"
                })
                .required_if_eq("disable-workspace", "true"),
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .help("Align a singular file, ignoring all ignores")
                .long_help(indoc! {
                    "Pass in a singular file to align
                    Ignores any .gitignores/align_by.ignores
                    Does not format from the workspace root"
                })
                .conflicts_with("ignore")
                .conflicts_with("path"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::Count)
                .help("Increase program verbosity")
                .long_help(indoc! {
                    "Increases program verbosity
                By default only errors are emitted
                -v enables warnimg messages
                -vv enables info messages"
                })
                .conflicts_with("quiet"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue)
                .help("Silence all program output"),
        )
        .arg(
            Arg::new("disable-workspace")
                .long("disable-workspace")
                .action(ArgAction::SetTrue)
                .help("Disable the default formatting from cargo workspace root"),
        )
        .arg(
            Arg::new("filesize-limit")
                .long("filesize-limit")
                .help("Set the filesize limit in bytes")
                .default_value("1048576"),
        )
        .get_matches();

    env_logger::builder()
        .filter_level(
            match (matches.get_flag("quiet"), matches.get_count("verbose")) {
                (true, _) => log::LevelFilter::Off,
                (false, 0) => log::LevelFilter::Error,
                (false, 1) => log::LevelFilter::Warn,
                (false, 2) => log::LevelFilter::Info,
                (false, _) => log::LevelFilter::max(),
            },
        )
        .format(|_f, record| {
            use std::io::Write;
            if record.level() >= log::Level::Error {
                writeln!(std::io::stderr(), "{}", record.args())
            } else {
                writeln!(std::io::stdout(), "{}", record.args())
            }
        })
        .init();

    let mut alignment_paths = vec![];
    if !matches.get_flag("disable-workspace") {
        alignment_paths.push(
            cargo_metadata::MetadataCommand::new()
                .exec()?
                .workspace_root,
        );
    }
    alignment_paths.extend(
        matches
            .get_many::<Utf8PathBuf>("path")
            .unwrap_or_default()
            .into_iter()
            .cloned(),
    );

    let mut walk_builder = WalkBuilder::new(&alignment_paths[0]);
    for path in alignment_paths[1..].iter() {
        walk_builder.add(path);
    }

    for ignore in matches.get_many::<String>("ignore").unwrap_or_default() {
        walk_builder.add_ignore(ignore);
    }
    walk_builder.add_custom_ignore_filename("align_by.ignore");

    let max_filesize = matches
        .get_one::<String>("filesize-limit")
        .expect("filesize-limit should have a default value");
    let max_filesize = match max_filesize.parse::<u64>() {
        Ok(x) => x,
        Err(err) => {
            error!(
                r#"filesize-limit expects a valid u64, got {max_filesize:?} with error "{err}""#
            );
            std::process::exit(exitcode::USAGE);
        }
    };
    walk_builder.max_filesize(Some(max_filesize));

    walk_builder.standard_filters(true);

    let mut files_failed_to_align = 0;
    let mut files_unchanged = 0;
    let mut files_aligned = 0;
    let mut file_read_errors = 0;
    let mut file_write_errors = 0;
    let mut files_canceled = 0;

    if let Some(file_path) = matches.get_one::<String>("file") {
        match std::fs::metadata(file_path) {
            Ok(meta) => {
                if !meta.is_file() {
                    error!("Path at \"{file_path}\" passed in though --file is not a file");
                    std::process::exit(exitcode::USAGE);
                }
            },
            Err(err) => {
                error!("Failue getting metadata of file \"{file_path}\" with error {err}");
                std::process::exit(exitcode::IOERR);
            },
        }
        walk_builder = WalkBuilder::new(file_path);
    }

    for file_path in walk_builder.build() {
        let file_path = match file_path {
            Ok(ok) => ok,
            Err(err) => {
                error!("{}", err);
                file_read_errors += 1;
                continue;
            }
        };

        match file_path.metadata() {
            Ok(ok) => {
                if !ok.is_file() {
                    continue;
                }
            }
            Err(err) => {
                error!("{}", err);
                file_read_errors += 1;
                continue;
            }
        }

        let file_content = match std::fs::read_to_string(file_path.path()) {
            Ok(ok) => ok,
            Err(err) => {
                error!("{}", err);
                file_read_errors += 1;
                continue;
            }
        };

        let aligned_content = match align_string(&file_content) {
            Ok(x) => x,
            Err(err) => {
                match err {
                    AlignmentError::FileCanceled => {
                        info!("Canceled file: {}", file_path.path().display());
                        files_canceled += 1;
                    }
                    AlignmentError::InvalidAlignmentStatement(line_number, column, reason) => {
                        error!(
                            "Invalid alignment statement at {}:{}:{} with reason {}",
                            file_path.path().display(),
                            line_number,
                            column,
                            reason
                        );
                        files_failed_to_align += 1;
                    }
                }
                continue;
            }
        };

        if aligned_content == file_content {
            info!("Unchanged file: {}", file_path.path().display());
            files_unchanged += 1;
            continue;
        }

        if let Err(err) = std::fs::write(file_path.path(), aligned_content) {
            error!(
                "Failed to write aligned content to file at path \"{}\" with error {err}",
                file_path.path().display()
            );
            file_write_errors += 1;
            continue;
        } else {
            info!("Successfully aligned file: \"{}\"", file_path.path().display());
            files_aligned += 1;
        }
    }

    if !matches.get_flag("quiet") {
        println!("Aligning finished");
        if files_failed_to_align != 0 {
            println!("Alignment failures: {files_failed_to_align}")
        }
        if files_unchanged != 0 {
            println!("Unchanged files: {files_unchanged}")
        }
        if files_aligned != 0 {
            println!("Aligned files: {files_aligned}")
        }
        if file_read_errors != 0 {
            println!("File read errors: {file_read_errors}")
        }
        if file_write_errors != 0 {
            println!("File write errors: {file_write_errors}")
        }
        if files_canceled != 0 {
            println!("Files canceled: {files_canceled}")
        }
    };

    Ok(())
}

#[derive(Error, Debug)]
enum AlignmentError {
    #[error("FileCanceled")]
    FileCanceled,
    #[error("InvalidAlignmentStatement({0}, {1}, {2})")]
    InvalidAlignmentStatement(usize, usize, InvalidAlignmentStatement),
}

impl AlignmentError {
    fn unexpected_eof(line: usize, column: usize) -> Self {
        Self::InvalidAlignmentStatement(line, column, InvalidAlignmentStatement::UnexpectedEOF)
    }

    fn missing_space(line: usize, column: usize) -> Self {
        Self::InvalidAlignmentStatement(line, column, InvalidAlignmentStatement::MissingSpace)
    }

    fn unclosed_quotes(line: usize, column: usize) -> Self {
        Self::InvalidAlignmentStatement(line, column, InvalidAlignmentStatement::UnclosedQuotes)
    }

    fn missing_quote(line: usize, column: usize) -> Self {
        Self::InvalidAlignmentStatement(line, column, InvalidAlignmentStatement::MissingQuote)
    }
}

#[derive(Error, Debug)]
enum InvalidAlignmentStatement {
    #[error("unexpected end-of-file")]
    UnexpectedEOF,
    #[error("missing space")]
    MissingSpace,
    #[error("unclosed quote")]
    UnclosedQuotes,
    #[error("missing quote")]
    MissingQuote,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Sort,
    Regex,
    RegexSort,
}

impl Mode {
    fn sort(&self) -> bool {
        self == &Self::Sort || self == &Self::RegexSort
    }
    
    fn regex(&self) -> bool {
        self == &Self::Regex || self == &Self::RegexSort
    }
}

enum NewlineStyle {
    CR,
    LF,
    CRLF,
}

impl ToString for NewlineStyle {
    fn to_string(&self) -> String {
        match self {
            NewlineStyle::CR => "\r".to_owned(),
            NewlineStyle::LF => "\n".to_owned(),
            NewlineStyle::CRLF => "\r\n".to_owned(),
        }
    }
}

struct NewlineCount {
    cr: usize,
    lf: usize,
    crlf: usize,
}

impl NewlineCount {
    fn new(cr: usize, lf: usize, crlf: usize) -> Self {
        Self { cr, lf, crlf }
    }

    fn max(self) -> NewlineStyle {
        let Self { cr, lf, crlf } = self;
        if cr > lf && cr > crlf {
            NewlineStyle::CR
        } else if lf > cr && lf > crlf {
            NewlineStyle::LF
        } else if crlf > cr && crlf > lf {
            NewlineStyle::CRLF
        } else {
            // default because why not
            NewlineStyle::LF
        }
    }
}

impl std::ops::Add for NewlineCount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            cr: self.cr + rhs.cr,
            lf: self.lf + rhs.lf,
            crlf: self.crlf + rhs.crlf,
        }
    }
}

fn detect_newline_style(s: &str) -> NewlineStyle {
    s.chars()
        .collect::<Vec<char>>()
        .windows(2)
        .map(|x| match x {
            ['\r', '\n'] => NewlineCount::new(0, 0, 1),
            ['\r', _] => NewlineCount::new(1, 0, 0),
            ['\n', _] => NewlineCount::new(0, 1, 0),
            _ => NewlineCount::new(0, 0, 0),
        })
        .fold(NewlineCount::new(0, 0, 0), |x, y| x + y)
        .max()
}

fn align_string(s: &str) -> std::result::Result<String, AlignmentError> {
    let newline = detect_newline_style(s).to_string();
    let mut lines = s.lines().enumerate().peekable();
    let mut aligned_file = String::new();
    let mut paused = false;

    while let Some((line_index, line)) = lines.next() {
        let orig_line_len = line.len();
        aligned_file.push_str(line);
        aligned_file.push_str(&newline);

        let Some(align_by_index) = line.find("align_by") else {
            continue;
        };
        let Some(line) = line.get(align_by_index + const { "align_by".len() }..) else {
            return Err(AlignmentError::unexpected_eof(
                line_index,
                orig_line_len - line.len(),
            ));
        };

        let line = if line.starts_with(" ") {
            &line[1..]
        } else {
            return Err(AlignmentError::missing_space(
                line_index,
                orig_line_len - line.len(),
            ));
        };

        let (line, mode) = if line.starts_with("cancel_file") {
            return Err(AlignmentError::FileCanceled);
        } else if line.starts_with("pause") {
            paused = true;
            continue;
        } else if line.starts_with("resume") {
            paused = false;
            continue;
        } else if line.starts_with("regex sort ") {
            (&line[const { "regex sort ".len() }..], Mode::RegexSort)
        } else if line.starts_with("regex ") {
            (&line[const { "regex ".len() }..], Mode::Regex)
        } else if line.starts_with("sort ") {
            (&line[const { "sort ".len() }..], Mode::Sort)
        } else {
            (line, Mode::Normal)
        };

        if paused {
            continue;
        }

        let line = if line.starts_with("\"") {
            &line[1..]
        } else {
            return Err(AlignmentError::missing_quote(
                line_index,
                orig_line_len - line.len(),
            ));
        };

        if !line.contains('"') {
            return Err(AlignmentError::unclosed_quotes(
                line_index,
                orig_line_len - line.len(),
            ));
        }

        let alignment_parts = extract_quote(line);
        if alignment_parts.is_empty() {
            continue;
        }
        let alignment_parts = alignment_parts
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let mut lines_to_be_modified = Vec::new();

        loop {
            let Some((_, next_line)) = lines.peek() else {
                break;
            };

            if next_line.contains("align_by") {
                break;
            }

            if let Some(broken_str) = seperate_str_on_alignments(
                next_line
                    .split_ascii_whitespace()
                    .flat_map(|x| [x, " "])
                    .collect::<String>()
                    .trim_end()
                    .to_string(),
                &alignment_parts,
            ) {
                lines.next();
                lines_to_be_modified.push(broken_str);
            } else {
                break;
            }
        }

        if lines_to_be_modified.is_empty() {
            continue;
        }

        let transposed_unmodified_lines = (0..lines_to_be_modified[0].len())
            .map(|col| {
                (0..lines_to_be_modified.len())
                    .map(|row| lines_to_be_modified[row][col].clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut modified_columns = Vec::new();
        for unaligned_line in
            transposed_unmodified_lines[..transposed_unmodified_lines.len() - 2].iter()
        {
            let column_max_len = unaligned_line.iter().map(String::len).max().unwrap();
            let adjustment_line = unaligned_line
                .iter()
                .map(|s| " ".repeat(column_max_len - s.len()))
                .collect::<Vec<_>>();
            modified_columns.push(unaligned_line.clone());
            modified_columns.push(adjustment_line);
        }
        modified_columns
            .push(transposed_unmodified_lines[transposed_unmodified_lines.len() - 2].clone());
        modified_columns
            .push(transposed_unmodified_lines[transposed_unmodified_lines.len() - 1].clone());

        let mut modified_lines = (0..modified_columns[0].len())
            .map(|col| {
                (0..modified_columns.len())
                    .map(|row| modified_columns[row][col].clone())
                    .collect::<Vec<_>>()
                    .concat()
            })
            .collect::<Vec<_>>();

        if mode.sort() {
            modified_lines.sort();
        }

        aligned_file.push_str(&modified_lines.concat());
    }

    if !s.ends_with(&newline) {
        Ok(aligned_file[..aligned_file.len().saturating_sub(newline.len())].to_owned())
    } else {
        Ok(aligned_file)
    }
}

fn extract_quote(s: &str) -> String {
    s.chars()
        .scan(false, |escaped, x| {
            if x == '\\' {
                *escaped = true;
                Some(x)
            } else if x == '"' && escaped.not() {
                None
            } else {
                *escaped = false;
                Some(x)
            }
        })
        .collect()
}

fn seperate_str_on_alignments(s: String, alignment_parts: &[String]) -> Option<Vec<String>> {
    if alignment_parts.is_empty() {
        return Some(vec![s, "\n".to_string()]);
    }

    let (x, y) = s.split_once(alignment_parts.first()?)?;
    Some(
        [
            vec![x.to_string(), alignment_parts.first()?.clone()],
            seperate_str_on_alignments(y.to_string(), &alignment_parts[1..])?,
        ]
        .concat(),
    )
}

#[cfg(test)]
mod tests;
