def calculate_total_price(items, tax_rate):
    subtotal = 0
    for item in items:
        subtotal += item.price * item.quantity
    return subtotal + subtotal * tax_rate


def unrelated_helper(value):
    return value.strip().lower()
