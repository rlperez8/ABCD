import * as utilites from './utilities.js'

/**
 * Draws candlesticks on a canvas for the given chart reference.
 * 
 * This function iterates over all candles in `candleChartRef.current.candles.candles`
 * and renders each candle at its correct pixel position. It calculates the
 * vertical pixel position based on the candle’s open price and the current chart baseline.
 * Each candle is filled with its color and outlined with a black border.
 * 
 * @function
 * @param {Object} candleChartRef - Reference object for the chart containing candle data and chart settings.
 * @param {CanvasRenderingContext2D} ctx - The canvas 2D rendering context where the candles will be drawn.
 * 
 * @example
 * // Assuming you have a canvas context `ctx` and a candle chart reference `candleChartRef`
 * drawCandles(candleChartRef, ctx);
 */
export const candles = (candleChartRef, ctx) => {
  let startingX = -(candleChartRef.current.current_pixels_between_candles / 2);

  candleChartRef.current.candles.candles.forEach(item => {
    const x = Math.floor(startingX - candleChartRef.current.width.current_X_origin);

    const price_pixel_height = utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_open)
    const price_pixel_location = Math.floor(candleChartRef.current.height.currentBaselineY - price_pixel_height);

    const width = -candleChartRef.current.current_candle_width;
    const height = -Math.floor(item.current_height);

    // Fill the candle body
    ctx.fillStyle = item.color;
    ctx.fillRect(x, price_pixel_location, width, height);

    // Add a black border
    ctx.strokeStyle = "black";
    ctx.lineWidth = 2; // adjust thickness if desired (e.g., 1.5 for sharper look)
    ctx.strokeRect(x, price_pixel_location, width, height);

    startingX -= candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles;
  });
};

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
export const mouse_price = (candleChartRef, ctx_price) => {

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


