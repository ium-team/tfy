package fixture13

type PriceBook13 struct {
    Multiplier float64
}

func CalculateDiscount13(cartItems []float64, taxRate float64) float64 {
    totalAmount := 0.0
    for _, price := range cartItems {
        totalAmount += price
    }
    if totalAmount > 130.0 {
        return totalAmount * (1.0 + taxRate)
    }
    return totalAmount
}
