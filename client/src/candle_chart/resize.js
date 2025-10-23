import * as utilites from './utilities.js'

/**
 * Updates the chart's vertical position and mid-price based on mouse movement.
 * Calculates how many pixels the mouse moved vertically, shifts the chart's baseline,
 * and adjusts the mid-price to stay aligned with the visual chart movement.
 *
 * @function
 * @param {Object} candleChartRef - Reference to the chart object containing mouse, height, and price data.
 * @returns {void}
 *
 * @example
 * handle_y_movement(candleChartRef);
 */
export const chart_Y_movement = (candleChartRef) => {
    // Adjust Baseline-Y
    let pixels_mouse_moved = candleChartRef.current.mouse.down.y - candleChartRef.current.mouse.pos.y;
    candleChartRef.current.height.currentBaselineY = candleChartRef.current.height.previousBaselineY - pixels_mouse_moved;

    // Adjust Current Mid Price
    let convert_pixels = pixels_mouse_moved / candleChartRef.current.price.current_pixels_per_price_unit;
    candleChartRef.current.price.current_mid_price = candleChartRef.current.price.prev_mid_price - convert_pixels;
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
export const push_price_to_middle_screen = (candleChartRef,selected_pattern, size) => {

    // Convert target price into pixel location
    const price_pixel_location = utilites.get_pixel_location_of_a_price(candleChartRef, parseFloat(selected_pattern.pattern_A_high), size);

    // Push Baseline-Y Down by Converted Price Amount
    candleChartRef.current.height.currentBaselineY = price_pixel_location;
    candleChartRef.current.height.previousBaselineY = price_pixel_location;

    // Shift Baseline-Y to Center of Screen
    const half_screen_height = candleChartRef.current.height.startingBaselineY / 2.8;
    candleChartRef.current.height.currentBaselineY += half_screen_height;
    candleChartRef.current.height.previousBaselineY += half_screen_height;

    const mid_price = utilites.get_mid_price(candleChartRef)
    candleChartRef.current.price.current_mid_price = mid_price
    candleChartRef.current.price.prev_mid_price = mid_price

};

export const chart_zoom_out = (candleChartRef, threshold) => {

    // Decrease Unit Size
    candleChartRef.current.price.current_pixels_per_price_unit -= 1;
    candleChartRef.current.price.current_price_unit_pixel_size = Math.floor(candleChartRef.current.price.current_price_unit_pixel_size - 1);

    // Decrease Canvas Height
    candleChartRef.current.height.currentBaselineY -= candleChartRef.current.price.current_mid_price;
    candleChartRef.current.height.previousBaselineY -= candleChartRef.current.price.current_mid_price;

    // Change Grid Size
    if (candleChartRef.current.price.current_price_unit_pixel_size === threshold) {
        
        // Decrease Pixels Per Grid 
        candleChartRef.current.price.current_pixels_per_price_unit *= 2;
        candleChartRef.current.price.prev_pixels_per_price_unit *= 2;
        candleChartRef.current.price.current_price_unit_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit
        
        // Increase Displayed Grid Size
        candleChartRef.current.unit_amount *= 2 

        // Update Mid Price
        const mid_price = utilites.get_mid_price(candleChartRef)
        candleChartRef.current.price.current_mid_price = mid_price
        candleChartRef.current.price.prev_mid_price = mid_price
    }
};
export const chart_zoom_in = (candleChartRef, threshold) => {

    // Increase Unit Size
    candleChartRef.current.price.current_pixels_per_price_unit += 1;
    candleChartRef.current.price.current_price_unit_pixel_size = Math.floor(candleChartRef.current.price.current_price_unit_pixel_size +1)

    // Increase Canvas Height
    candleChartRef.current.height.currentBaselineY += candleChartRef.current.price.current_mid_price;
    candleChartRef.current.height.previousBaselineY += candleChartRef.current.price.current_mid_price;

    // Change Grid Size
    if (candleChartRef.current.price.current_price_unit_pixel_size === threshold) {

        // Increase Pixels Per Grid
        candleChartRef.current.price.current_pixels_per_price_unit /= 2;
        candleChartRef.current.price.prev_pixels_per_price_unit /= 2;
        candleChartRef.current.price.current_price_unit_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit

        // Increase Displayed Grid Size
        candleChartRef.current.unit_amount /= 2
        
        // Update Mid Price
        const mid_price = utilites.get_mid_price(candleChartRef)
        candleChartRef.current.price.current_mid_price = mid_price
        candleChartRef.current.price.prev_mid_price = mid_price

    }
};





export const reposition_candles = (candleChartRef, selected_pattern, selected_candles, set_pattern_abcd) => {
    function normalizeDate(dateStr) {
        if (!dateStr) return null;

        // Handle "MM-DD-YYYY" → "YYYY-MM-DD"
        if (/^\d{2}-\d{2}-\d{4}$/.test(dateStr)) {
            const [mm, dd, yyyy] = dateStr.split("-");
            return `${yyyy}-${mm}-${dd}`;
        }

        // Handle common string formats like "Mon, 01 Nov 1999 00:00:00 GMT"
        const parsed = new Date(dateStr);
        if (!isNaN(parsed)) {
            return parsed.toISOString().split("T")[0]; // normalize all to "YYYY-MM-DD"
        }

        return null;
        }

        function findIndexByDate(candles, patternDate) {
        if (!patternDate) return -1;

        const pivotDate = normalizeDate(patternDate);
        if (!pivotDate) return -1;

        return (
            candles.findIndex(obj => {
            const candleDate = normalizeDate(obj.date || obj.candle_date);
            return candleDate === pivotDate;
            }) + 1
        );
        }

        // ---- Then your logic ----
        let index_A = findIndexByDate(selected_candles, selected_pattern?.pattern_A_pivot_date);
        let index_B = findIndexByDate(selected_candles, selected_pattern?.pattern_B_pivot_date);
        let index_C = findIndexByDate(selected_candles, selected_pattern?.pattern_C_pivot_date);
        let index_D = findIndexByDate(selected_candles, selected_pattern?.trade_entered_date);
        let exit = findIndexByDate(selected_candles, selected_pattern?.trade_exited_date);

        set_pattern_abcd(prev => ({
        ...prev,
        a: index_A,
        b: index_B,
        c: index_C,
        d: index_D,
        a_price: parseFloat(selected_pattern.pattern_A_high),
        b_price: parseFloat(selected_pattern.pattern_B_low),
        c_price: parseFloat(selected_pattern.pattern_C_high),
        d_price: parseFloat(selected_pattern.trade_entered_price),
        stop_loss: parseFloat(selected_pattern.trade_stop_loss),
        take_profit: parseFloat(selected_pattern.trade_take_profit),
        entered_price: parseFloat(selected_pattern.trade_entered_price),
        exit_price: parseFloat(selected_pattern.trade_exited_price),
        exit_date: exit,
        }));

            

    // Grid Unit Height Adjuster
    let x = false
    let size = 1
    while(x===false){
    
        
        let pattern_price_height = (parseFloat(selected_pattern.pattern_A_high) - parseFloat(selected_pattern['trade_entered_price'])).toFixed(2)
        let pattern_box_height = candleChartRef.current.canvas_height / 2.8

        const one_dollar_pixel_size = candleChartRef.current.price.current_pixels_per_price_unit / size;
        const price_pixel_location = pattern_price_height * one_dollar_pixel_size;

        if(price_pixel_location > pattern_box_height){
            size*=2
        }
        else if(price_pixel_location < pattern_box_height){
            
            x = true
            candleChartRef.current.unit_amount = size

            // Push Baseline-Y Down by Converted Price Amount
            candleChartRef.current.height.currentBaselineY = price_pixel_location;
            candleChartRef.current.height.previousBaselineY = price_pixel_location;
        
            // Shift Baseline-Y to Center of Screen
            const half_screen_height = candleChartRef.current.height.startingBaselineY / 2;
            candleChartRef.current.height.currentBaselineY += half_screen_height;
            candleChartRef.current.height.previousBaselineY += half_screen_height;
        
            const mid_price = utilites.get_mid_price(candleChartRef)
            candleChartRef.current.price.current_mid_price = mid_price
            candleChartRef.current.price.prev_mid_price = mid_price
            
        }

    push_price_to_middle_screen(candleChartRef, selected_pattern)

    let y = false;

    while (y === false) {
        const candles = candleChartRef.current.candles;
        const chart = candleChartRef.current;

        // Always recalculate complete width as the sum of its parts
        candles.complete_width = chart.current_candle_width + chart.current_pixels_between_candles;

        let pattern_length = candles.complete_width * selected_pattern.pattern_ABCD_bar_length;
        let pattern_box_width = (chart.canvas_width / 2.8) * 1;

        if (pattern_length > pattern_box_width) {

            if (chart.current_pixels_between_candles > -0.25) {
                chart.current_pixels_between_candles -= 0.25;
            }
            if (chart.current_candle_width > -0.75) {
                chart.current_candle_width -= 0.75;
            }

            // Recalculate after adjustments
            candles.complete_width = chart.current_candle_width + chart.current_pixels_between_candles;
        }
        else if (pattern_length < pattern_box_width) {
            y = true;
        }
    }

    candleChartRef.current.width.current_X_origin = -(candleChartRef.current.width.grid_width/2.8) - (candleChartRef.current.candles.complete_width * index_A) 
    candleChartRef.current.width.prev_X_origin = -(candleChartRef.current.width.grid_width/2.8) - (candleChartRef.current.candles.complete_width * index_A) 
    
    candleChartRef.current.selected_candle = parseFloat(selected_pattern.pattern_A_high)
    
    }
}