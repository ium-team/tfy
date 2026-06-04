package fixture24

type PriceBook24 struct {
    Multiplier float64
}

func CalculateDiscount24(cartItems []float64, taxRate float64) float64 {
    totalAmount := 0.0
    for _, price := range cartItems {
        totalAmount += price
    }
    if totalAmount > 240.0 {
        return totalAmount * (1.0 + taxRate)
    }
    return totalAmount
}
