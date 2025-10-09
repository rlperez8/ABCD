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
export const update_zoom_out_info = (candleChartRef) => {

    console.log('current mid:',candleChartRef.current.price.current_mid_price)
    console.log('previous mid:',candleChartRef.current.price.prev_mid_price)

 
    candleChartRef.current.price.current_pixels_per_price_unit -= 1;
    candleChartRef.current.height.currentBaselineY -= candleChartRef.current.price.current_mid_price;
    candleChartRef.current.height.previousBaselineY -= candleChartRef.current.price.current_mid_price;
    candleChartRef.current.price.current_price_unit_pixel_size = Math.floor(candleChartRef.current.price.current_price_unit_pixel_size - 1);
    candleChartRef.current.candles.starting_candle_Y = candleChartRef.current.selected_candle  * (candleChartRef.current.price.current_pixels_per_price_unit/candleChartRef.current.price.counter)
  
};
export const update_zoom_in_info = (candleChartRef) => {


   
    // candleChartRef.current.zoom.current -= 1;
    candleChartRef.current.price.current_pixels_per_price_unit += 1;
    candleChartRef.current.height.currentBaselineY += candleChartRef.current.price.current_mid_price;
    candleChartRef.current.height.previousBaselineY += candleChartRef.current.price.current_mid_price;
    candleChartRef.current.price.current_price_unit_pixel_size = Math.floor(candleChartRef.current.price.current_price_unit_pixel_size +1)
    candleChartRef.current.candles.starting_candle_Y = candleChartRef.current.selected_candle  * (candleChartRef.current.price.current_pixels_per_price_unit/candleChartRef.current.price.counter)

    
  

};
export const zoom_out_threshold = (candleChartRef, threshold) => {

        if (candleChartRef.current.price.current_price_unit_pixel_size === threshold) {

        candleChartRef.current.grid_size_count+=1
        
        // Increase Unit Size
        candleChartRef.current.price.current_pixels_per_price_unit *= 2;
        candleChartRef.current.price.prev_pixels_per_price_unit *= 2;
        
        // Increase Displayed Numbers
        candleChartRef.current.price.counter *= 2;
        
    
        candleChartRef.current.price.current_mid_price = ((candleChartRef.current.height.currentBaselineY - (candleChartRef.current.height.startingBaselineY /2)) / candleChartRef.current.price.current_pixels_per_price_unit) 
        candleChartRef.current.price.prev_mid_price = ((candleChartRef.current.height.currentBaselineY - (candleChartRef.current.height.startingBaselineY /2)) / candleChartRef.current.price.current_pixels_per_price_unit);

        

        candleChartRef.current.unit_amount *= 2
        candleChartRef.current.price.current_price_unit_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit
        }

}
export const zoom_in_threshold = (candleChartRef, expand_threshold) => {

    if (candleChartRef.current.price.current_price_unit_pixel_size === expand_threshold) {

    // Descrease Unit Pixel Size
    candleChartRef.current.price.current_pixels_per_price_unit /= 2;
    candleChartRef.current.price.prev_pixels_per_price_unit /= 2;

    // Descrease Displayed Numbers
    candleChartRef.current.price.counter /= 2;
    


    candleChartRef.current.price.current_mid_price = ((candleChartRef.current.height.currentBaselineY - (candleChartRef.current.height.startingBaselineY /2)) / candleChartRef.current.price.current_pixels_per_price_unit);
    candleChartRef.current.price.prev_mid_price = ((candleChartRef.current.height.currentBaselineY - (candleChartRef.current.height.startingBaselineY /2)) / candleChartRef.current.price.current_pixels_per_price_unit);

    candleChartRef.current.unit_amount /= 2
    candleChartRef.current.price.current_price_unit_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit


    }
}
export const handle_y_movement = (candleChartRef) => {

    // Adjust Baseline-Y
    let pixels_mouse_moved = candleChartRef.current.mouse.down.y - candleChartRef.current.mouse.pos.y
    candleChartRef.current.height.currentBaselineY = candleChartRef.current.height.previousBaselineY - pixels_mouse_moved

    // Adjust Current Mid Price
    let convert_pixels = pixels_mouse_moved / candleChartRef.current.price.current_pixels_per_price_unit
    candleChartRef.current.price.current_mid_price = candleChartRef.current.price.prev_mid_price - convert_pixels


}
const add_shrink_expand_to_candle_top = (obj, candleChartRef) => {
    let res = (obj.candle_close - obj.candle_open) / candleChartRef.current.unit_amount;
    res = res * candleChartRef.current.price.current_pixels_per_price_unit;
    return res;
};
const add_shrink_expand_to_candle = (price, candleChartRef) => {
    let res = price / candleChartRef.current.unit_amount;
    res = res * candleChartRef.current.price.current_pixels_per_price_unit;
    res = res - candleChartRef.current.height.startingBaselineY;
    res = res + candleChartRef.current.zoom.shrink_expand_height;

    return res;
};
export const updateCandles = (candleChartRef) => {
    candleChartRef.current.candles.candles = candleChartRef.current.candles.candles.map((obj) => ({
        ...obj,
        current_high: add_shrink_expand_to_candle(obj.high, candleChartRef),
        current_height:
            Math.abs(obj.current_height) > 1
                ? add_shrink_expand_to_candle_top(obj, candleChartRef)
                : 1,
        // current_bottom: add_shrink_expand_to_candle(obj.open, candleChartRef),
        current_low: add_shrink_expand_to_candle(obj.low, candleChartRef),
    }));
};
