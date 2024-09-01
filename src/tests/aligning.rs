use super::alignment_test;

alignment_test! {dont_insert_newline, "", ""}
alignment_test! {dont_remove_newline, "\n", "\n"}
alignment_test! {parsing_ignores_trailing_garbage,
    r#"align_by "="1"#,
    r#"align_by "="1"#
}
alignment_test! {standard_example, r#"
    align_by "="
1   = 222
111 = 2
"#, r#"
    align_by "="
1   = 222
111 = 2
"#}
alignment_test! {collapse_spaces, r#"
    align_by "="
1   = 222
111 = 2
"#, r#"
    align_by "="
1   = 222
111 = 2
"#}
alignment_test! {standard_example_double_parsing, r#"
    align_by "= ;"
1   = 222;
111 = 2  ;
"#, r#"
    align_by "= ;"
1   = 222;
111 = 2  ;
"#}
alignment_test! {alignment_statements_dont_align_each_other, r#"
    align_by "="
    align_by "="
1=1
"#, r#"
    align_by "="
    align_by "="
1=1
"#}
alignment_test! {spaces_are_expanded, r#"
    align_by "="
1 =1
22=2
"#, r#"
    align_by "="
1 =1
22=2
"#}
