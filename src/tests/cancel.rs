use crate::{align_string, AlignmentError};

#[test]
fn cancel() {
    assert!(matches!(
        align_string(
            r#"
                align_by "="
11=1
1 =11
                align_by cancel_file
            "#
        ),
        Err(AlignmentError::FileCanceled)
    ))
}
