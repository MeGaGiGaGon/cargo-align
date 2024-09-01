mod cancel;
mod sorting;
mod quote_gathering;
mod aligning;
mod pause_and_resume;
mod regex;

macro_rules! alignment_test {
    ($test_name:ident, $starting_string:literal, $ending_string:literal) => {
        #[test]
        fn $test_name() -> anyhow::Result<()> {
            assert_eq!(
                crate::align_string(indoc::indoc! {$starting_string})?, 
                indoc::indoc! {$ending_string}
            );
            Ok(())
        }
    };
}

pub(crate) use alignment_test;
