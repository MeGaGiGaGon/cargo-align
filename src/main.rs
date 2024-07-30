use anyhow::Context;
use anyhow::Result;
use std::ops::Not;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path_to_align = match args.len() {
        1 => PathBuf::from(fetch_cargo_metadata()?),
        2 => PathBuf::from(&args[1]),
        len => panic!("Expected either 0 or 1 arguments, got {len}"),
    };
    
    let mut files_failed_to_align = 0;
    let mut files_unchanged = 0;
    let mut files_aligned = 0;
    let files_to_process = get_files_recursively(path_to_align);
    for file_path in files_to_process.iter() {
        let file_content = match std::fs::read_to_string(file_path) {
            Err(_) => {
                files_failed_to_align += 1;
                continue;
            }
            Ok(s) => s,
        };
        let aligned_content = align_string(&file_content);

        if file_content == aligned_content {
            files_unchanged += 1;
            continue;
        }

        if let Err(err) = std::fs::write(file_path, aligned_content).with_context(|| {
            format!(
                "Failed to write aligned content to file at path {}",
                file_path.display()
            )
        }) {
            eprintln!("{err}");
        } else {
            files_aligned += 1;
        }
    }
    
    println!("Aligning finished, {files_failed_to_align} failed to align because of non-utf-8 data, {files_unchanged} unchanged, {files_aligned} aligned.");
    Ok(())
}

fn fetch_cargo_metadata() -> Result<String> {
    let metadata_raw = std::process::Command::new("cargo")
        .arg("metadata")
        .output()
        .context("Failed to run `cargo metadata`.")?
        .stdout;
    let metadata =
        std::str::from_utf8(&metadata_raw).context("`cargo metadata` produced invalid utf-8.")?;
    let workspace_root_plus_garbage = metadata
        .split_once("\"workspace_root\":\"")
        .context("Failed to find `workspace_root` in the output of `cargo metadata`.")?
        .1;
    Ok(extract_quote(
        workspace_root_plus_garbage,
    ))
}

fn get_files_recursively(path: PathBuf) -> Vec<PathBuf> {
    let path_metadata = match std::fs::metadata(&path)
        .with_context(|| format!("Failed to get metadata of path {}", path.display()))
    {
        Err(err) => {
            eprintln!("{err}");
            return vec![];
        }
        Ok(meta) => meta,
    };

    if path_metadata.is_file() {
        if path_metadata.len() > 1 << 20 {
            eprintln!(
                "Skipping file {} because it is over 1 MB in size.",
                path.display()
            );
            return vec![];
        } else {
            return vec![path];
        }
    }

    if path.file_name() == Some(std::ffi::OsStr::new(".git")) {
        return vec![];
    }

    let dir_contents = match std::fs::read_dir(&path)
        .with_context(|| format!("Failed to read contents of path {}", path.display()))
    {
        Err(err) => {
            eprintln!("{err}");
            return vec![];
        }
        Ok(read_dir) => read_dir,
    }
    .filter_map(|x| {
        match x.with_context(|| {
            format!(
                "Failure occured on reading one item from the contents of path {}",
                path.display()
            )
        }) {
            Err(err) => {
                eprintln!("{err}");
                None
            }
            Ok(dir_entry) => Some(dir_entry),
        }
    })
    .collect::<Vec<_>>();

    let dir_contents = match dir_contents.iter().find(|d| d.file_name() == ".gitignore") {
        None => dir_contents,
        Some(git_ignore_dir) => {
            match std::fs::read_to_string(git_ignore_dir.path()).with_context(|| {
                format!(
                    "Failed to read content of `.gitignore` at path {}",
                    git_ignore_dir.path().display()
                )
            }) {
                Err(err) => {
                    eprintln!("{err}");
                    dir_contents
                }
                Ok(git_ignore_content) => {
                    let ignored_directories = git_ignore_content
                        .lines()
                        .filter_map(|line| {
                            if line.len() > 1 && line.starts_with('/') && !line[1..].contains('/') {
                                Some(&line[1..])
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    dir_contents
                        .into_iter()
                        .filter(|d| {
                            ignored_directories
                                .iter()
                                .all(|line| d.file_name() != *line)
                        })
                        .collect()
                }
            }
        }
    };

    dir_contents
        .into_iter()
        .flat_map(|d| get_files_recursively(d.path()))
        .collect()
}

fn align_string(s: &str) -> String {
    let mut lines = s.lines().peekable();
    let mut aligned_file = Vec::new();
    let mut stopped = false;

    while let Some(line) = lines.next() {
        aligned_file.push(line.to_string());
        aligned_file.push("\n".to_string());
        if line.contains(&["align_by", " stop"].concat()) {
            stopped = true;
        }

        if stopped {
            continue;
        }

        let Some(align_by_index) = line.find("align_by") else {
            continue;
        };
        let align_by_index = align_by_index + 8;
        let line = &line[align_by_index..];

        let (alignment_statement, sort) = if line.starts_with(" sort \"") {
            (
                match line.get(7..) {
                    Some(x) => x,
                    None => continue,
                },
                true,
            )
        } else if line.starts_with(" \"") {
            (
                match line.get(2..) {
                    Some(x) => x,
                    None => continue,
                },
                false,
            )
        } else {
            continue;
        };

        let alignment_parts = extract_quote(alignment_statement);
        if alignment_parts.is_empty() {
            continue;
        }
        let alignment_parts = alignment_parts
            .split_ascii_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let mut lines_to_be_modified = Vec::new();

        loop {
            let Some(next_line) = lines.peek() else {
                break;
            };
            if next_line.contains("align_by \"") || next_line.contains("align_by sort \""){
                break;
            }

            let Some(next_line) = lines.peek() else {
                break;
            };

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

        if sort {
            modified_lines.sort();
        }

        aligned_file.push(modified_lines.concat());
    }

    [aligned_file.concat().trim_end(), "\n"].concat()
}

fn extract_quote(s: &str) -> String {
    s.chars().scan(false, |escaped, x| {
        if x == '\\' {
            *escaped = true;
            Some(x)
        } else if x == '"' && escaped.not() {
            None
        } else {
            *escaped = false;
            Some(x)
        }
}).collect()
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
#[rustfmt::skip] // align_by stop
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn quote_gathering() {
        assert_eq!(extract_quote(""), "");
        assert_eq!(extract_quote("\\"), "\\");
        assert_eq!(extract_quote("\""), "");
        assert_eq!(extract_quote("\\\""), "\\\"");
        assert_eq!(extract_quote("=\""), "=");
        assert_eq!(extract_quote("hello \\\"world\\\"!\""),"hello \\\"world\\\"!");
    }

    #[test]
    fn aligning() {
        assert_eq!(align_string(""), "\n");
        assert_eq!(align_string("\n"), "\n");
        assert_eq!(align_string(indoc! {r#"
            align_by "="aa
        "#}), indoc! {r#"
            align_by "="aa
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by "="
            1 = 222
            111 = 2
        "#}), indoc! {r#"
            align_by "="
            1   = 222
            111 = 2
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by "="
            1      = 222
            111 =     2
        "#}), indoc! {r#"
            align_by "="
            1   = 222
            111 = 2
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by "= ;"
            1 = 222;
            111 = 2;
        "#}), indoc! {r#"
            align_by "= ;"
            1   = 222;
            111 = 2  ;
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by "="
            align_by "="
            1=1
        "#}), indoc! {r#"
            align_by "="
            align_by "="
            1=1
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by "="
            1=1
            22=2
        "#}), indoc! {r#"
            align_by "="
            1 =1
            22=2
        "#});
    }

    #[test]
    fn sorting() {
        assert_eq!(align_string(indoc! {r#"
            align_by sort "="
            2=2
            1=1
        "#}), indoc! {r#"
            align_by sort "="
            1=1
            2=2
        "#});

        assert_eq!(align_string(indoc! {r#"
            align_by sort "="
            22=2
            1=1
        "#}), indoc! {r#"
            align_by sort "="
            1 =1
            22=2
        "#});
        
        assert_eq!(align_string(indoc! {r#"
            align_by sort "="
            align_by sort "="
            22=2
            1=1
        "#}), indoc! {r#"
            align_by sort "="
            align_by sort "="
            1 =1
            22=2
        "#});
    }

    #[test]
    fn stop() {
        assert_eq!(align_string(indoc! {r#"
            1
            2
            3
            4
            5
            align_by stop
        "#}), indoc! {r#"
            1
            2
            3
            4
            5
            align_by stop
        "#});
    }
}
