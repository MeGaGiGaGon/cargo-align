use super::alignment_test;

alignment_test!{equal_length_sort, r#"
    align_by sort "="
1=1
2=2
"#, r#"
    align_by sort "="
1=1
2=2
"#}
alignment_test!{align_and_sort, r#"
    align_by sort "="
1 =1
22=2
"#, r#"
    align_by sort "="
1 =1
22=2
"#}
alignment_test!{sort_align_and_ignore_align_statements, r#"
    align_by sort "="
    align_by sort "="
1 =1
22=2
"#, r#"
    align_by sort "="
    align_by sort "="
1 =1
22=2
"#}
