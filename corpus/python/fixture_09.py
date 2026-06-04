def calculate_discount_9(cart_items, tax_rate):
    total_amount = sum(item["price"] * item.get("qty", 1) for item in cart_items)
    if total_amount > 90:
        return total_amount * (1 + tax_rate)
    return total_amount

class PriceBook9:
    def lookup_price(self, sku):
        return len(sku) + 9
