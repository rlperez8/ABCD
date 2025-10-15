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


export const draw_candle = (item, candleChartRef, ctx, x) =>{

    // Convert Open and Close Price into Pixels
    let candle_open_pixel = get_pixel_location_of_a_price(candleChartRef, item.candle_open);
    const candle_close_pixel = get_pixel_location_of_a_price(candleChartRef, item.candle_close);

    // Get Candle Height
    let candle_height = candle_open_pixel - candle_close_pixel;

    // Get Price Location on Canvas
    candle_open_pixel = Math.floor(candleChartRef.current.height.currentBaselineY - candle_open_pixel);    

    // Get Candle Width
    const width = -candleChartRef.current.current_candle_width;

    // Add Color to Candle
    ctx.fillStyle = item.color;

    // Draw Candle
    ctx.fillRect(x, candle_open_pixel, width, candle_height);

    // Add Border
    ctx.strokeStyle = "black";
    ctx.lineWidth = 2;

}


export const draw_wick = (item, candleChartRef, ctx, x) => {

    const yHigh = Math.floor(candleChartRef.current.height.currentBaselineY - get_pixel_location_of_a_price(candleChartRef, item.candle_high));
    const yLow = Math.floor(candleChartRef.current.height.currentBaselineY - get_pixel_location_of_a_price(candleChartRef, item.candle_low));
    ctx.save()
    ctx.beginPath(); // start fresh path per wick
    ctx.strokeStyle = item.color;
    ctx.lineWidth = 1;
    ctx.moveTo(x, yHigh);
    ctx.lineTo(x, yLow);
    ctx.stroke();
    ctx.restore();
}