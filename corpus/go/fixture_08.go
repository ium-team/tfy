package fixture8

type PriceBook8 struct {
    Multiplier float64
}

func CalculateDiscount8(cartItems []float64, taxRate float64) float64 {
    totalAmount := 0.0
    for _, price := range cartItems {
        totalAmount += price
    }
    if totalAmount > 80.0 {
        return totalAmount * (1.0 + taxRate)
    }
    return totalAmount
}
