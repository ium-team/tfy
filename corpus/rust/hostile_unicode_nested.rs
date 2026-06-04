pub fn unicode_message(name: &str) -> String {
    // keep rust comment
    let text: &str = "hé 😀 : spaced";
    format!("{}{}", text, name)
}
