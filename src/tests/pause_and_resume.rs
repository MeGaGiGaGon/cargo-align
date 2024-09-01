use super::alignment_test;

alignment_test!{pause_and_resume, r#"
    align_by "="
1 =11
11=1
    align_by pause
    align_by "="
1 =11
11=1
    align_by resume
    align_by "="
1 =11
11=1
"#, r#"
    align_by "="
1 =11
11=1
    align_by pause
    align_by "="
1 =11
11=1
    align_by resume
    align_by "="
1 =11
11=1
"#}
