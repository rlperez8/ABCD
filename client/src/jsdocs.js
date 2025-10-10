/**
 * Draws the live price label on the price-axis canvas as the mouse moves.
 *
 * This function converts the mouse's Y position into a price value
 * based on the current chart scaling and vertical offset. It then
 * renders that price as text next to the mouse cursor.
 *
 * @param {Object} candleChartRef - A React ref object containing all current chart state and geometry info.
 * @param {CanvasRenderingContext2D} ctx_price - The 2D canvas rendering context used to draw the price text.
 *
 * @description
 * Steps performed:
 * 1. Converts the mouse's Y-coordinate into the chart’s inverted coordinate system.
 * 2. Translates that pixel position into a price value using the `pixels_per_price_unit` ratio.
 * 3. Adjusts for the chart’s vertical offset (`current_Y_OffSet`).
 * 4. Multiplies by `unit_amount` to get the actual price.
 * 5. Renders the price value at the mouse’s Y position on the price canvas.
 *
 * @example
 * // Example usage inside a render loop:
 * draw_mouse_price(candleChartRef, ctx_price);
 */
export const draw_mouse_price = (candleChartRef, ctx_price) => {

    ctx_price.beginPath();
    ctx_price.fillStyle = "rgb(74, 13, 13)";

    // Flip Mouse Y-Coordinate (convert canvas coordinate to chart coordinate)
    const flipped_y_coord = Math.abs(
        candleChartRef.current.mouse.pos.y - candleChartRef.current.height.currentBaselineY
    );

    // Convert flipped Y-Coordinate into a chart price coordinate
    const y_loc_price = flipped_y_coord / candleChartRef.current.price.current_pixels_per_price_unit;

    // Compute the actual price
    const price = y_loc_price * candleChartRef.current.unit_amount;

    // Draw the price value near the mouse cursor
    ctx_price.fillStyle = "gray";
    ctx_price.font = "20px Source Sans Pro";
    ctx_price.fillText(price.toFixed(2), 20, candleChartRef.current.mouse.pos.y + 5);
    ctx_price.stroke();
};
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
    const one_dollar_pixel_size =
        candleChartRef.current.price.current_pixels_per_price_unit /
        candleChartRef.current.price.counter;

    const price_pixel_location = price * one_dollar_pixel_size;

    return price_pixel_location;
};
/**
 * Centers the chart vertically around a target price value.
 *
 * This function converts a selected pattern's price into its corresponding
 * pixel location on the canvas, then adjusts the chart's baseline (Y-axis)
 * so that the price appears centered vertically on the screen. It also updates
 * the chart's current, previous, and static mid-price references in price units.
 *

 * @returns {void} This function does not return a value; it directly mutates
 * the `candleChartRef` object to update chart positioning and price metadata.
 *
 * @description
 * Steps performed:
 * 1. Convert the pattern's price into a pixel location using chart scaling.
 * 2. Move the chart's Y-baseline down to align with that price.
 * 3. Shift the baseline by half the screen height to visually center the price.
 * 4. Calculate the total chart height in both pixels and price units.
 * 5. Determine the half-screen price range.
 * 6. Compute and store the chart's mid-price (center price).
 *
 * @example
 * // Example usage:
 * push_price_to_middle_screen();
 *
 * // After execution, the chart will center around selected_pattern.pattern_A_high
 * // and candleChartRef.current.price.current_mid_price will reflect that value.
 */
export const push_price_to_middle_screen = (candleChartRef,selected_pattern) => {
    // Convert target price into pixel location
    const price_pixel_location = get_pixel_location_of_a_price(
        candleChartRef,
        parseFloat(selected_pattern.pattern_A_high)
    );

    // Push Baseline-Y Down by Converted Price Amount
    candleChartRef.current.height.currentBaselineY = price_pixel_location;
    candleChartRef.current.height.previousBaselineY = price_pixel_location;

    // Shift Baseline-Y to Center of Screen
    const half_screen_height =
        candleChartRef.current.height.startingBaselineY / 2;
    candleChartRef.current.height.currentBaselineY += half_screen_height;
    candleChartRef.current.height.previousBaselineY += half_screen_height;

    // Calculate total canvas height (in price units)
    const total_height_pixels = candleChartRef.current.height.currentBaselineY;
    const total_height_prices =
        total_height_pixels /
        candleChartRef.current.price.current_pixels_per_price_unit;

    // Compute half-screen price range
    const half_screen_price =
        half_screen_height /
        candleChartRef.current.price.current_pixels_per_price_unit;

    // Determine and store mid-price value
    const mid_price = total_height_prices - half_screen_price;

    candleChartRef.current.price.current_mid_price = mid_price;
    candleChartRef.current.price.prev_mid_price = mid_price;
    candleChartRef.current.price.static_mid = mid_price;
};