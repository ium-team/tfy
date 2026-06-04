fn calculate_total(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        total += item;
    }
    total
}
