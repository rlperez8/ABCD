export const get_mid_price = (candleChartRef) => {

    // Calculate total canvas height (in price units)
    const total_height_pixels = candleChartRef.current.height.currentBaselineY;
    const total_height_prices = total_height_pixels / candleChartRef.current.price.current_pixels_per_price_unit;
    const half_screen_height = candleChartRef.current.height.startingBaselineY / 2;

    // Compute half-screen price range
    const half_screen_price = half_screen_height / candleChartRef.current.price.current_pixels_per_price_unit;

    // Determine and store mid-price value
    const mid_price = total_height_prices - half_screen_price;

    return mid_price

}
/**
 * Calculates the pixel position on the chart that corresponds to a given price value.
 *
 * This function converts a price (e.g., $50) into its vertical pixel location
 * on the chart, based on the current scaling factor defined in `candleChartRef`.
 *
 * @param {number} price - The price value to convert into a pixel location.
 * @returns {number} The pixel coordinate representing the given price on the chart.
 *
 * @example
 * // If each dollar equals 2 pixels, a price of 50 would be at 100 pixels:
 * const pixelY = get_pixel_location_of_a_price(50);
 * console.log(pixelY); // → 100
 */
export const get_pixel_location_of_a_price = (candleChartRef, price) => {
    
    const one_dollar_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount;
    const price_pixel_location = price * one_dollar_pixel_size;

    return price_pixel_location;
};
