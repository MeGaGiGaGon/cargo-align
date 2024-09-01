use crate::extract_quote;

#[test]
fn quote_gathering() {
    assert_eq!(extract_quote(""), "");
    assert_eq!(extract_quote("\\"), "\\");
    assert_eq!(extract_quote("\""), "");
    assert_eq!(extract_quote("\\\""), "\\\"");
    assert_eq!(extract_quote("=\""), "=");
    assert_eq!(extract_quote("hello \\\"world\\\"!\""),"hello \\\"world\\\"!");
}
