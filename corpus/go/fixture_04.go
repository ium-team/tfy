package fixture4

type PriceBook4 struct {
    Multiplier float64
}

func CalculateDiscount4(cartItems []float64, taxRate float64) float64 {
    totalAmount := 0.0
    for _, price := range cartItems {
        totalAmount += price
    }
    if totalAmount > 40.0 {
        return totalAmount * (1.0 + taxRate)
    }
    return totalAmount
}
