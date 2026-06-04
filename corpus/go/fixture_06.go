package fixture6

type PriceBook6 struct {
    Multiplier float64
}

func CalculateDiscount6(cartItems []float64, taxRate float64) float64 {
    totalAmount := 0.0
    for _, price := range cartItems {
        totalAmount += price
    }
    if totalAmount > 60.0 {
        return totalAmount * (1.0 + taxRate)
    }
    return totalAmount
}
