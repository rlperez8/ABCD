import * as utilites from './utilities.js'
/**
 * Draws all candlestick objects on the main chart canvas.
 *
 * Converts each candle's price data into pixel coordinates, calculates
 * candle height and width, and renders both the candle body and its border.
 * This function dynamically adjusts candle placement based on chart position,
 * scaling, and spacing defined in `candleChartRef`.
 *
 * @function candles
 * @param {Object} candleChartRef - React ref object containing all chart state and dimension data.
 * @param {Object} candleChartRef.current - Current state of the chart reference.
 * @param {Object} candleChartRef.current.height - Height-related values and Y-baseline positions.
 * @param {number} candleChartRef.current.height.currentBaselineY - The current Y-axis baseline for price-to-pixel conversion.
 * @param {number} candleChartRef.current.height.startingBaselineY - The starting Y position used to center and align the chart.
 * @param {Object} candleChartRef.current.width - Width-related data for chart positioning.
 * @param {number} candleChartRef.current.width.current_X_origin - Current horizontal offset of the chart in pixels.
 * @param {Object} candleChartRef.current.price - Price scaling information.
 * @param {number} candleChartRef.current.price.current_pixels_per_price_unit - Pixel count that represents one price unit.
 * @param {Object} candleChartRef.current.candles - Container for candle data and metadata.
 * @param {Array<Object>} candleChartRef.current.candles.candles - Array of individual candle objects.
 * @param {number} candleChartRef.current.current_candle_width - Width of each candle body in pixels.
 * @param {number} candleChartRef.current.current_pixels_between_candles - Horizontal spacing between each candle.
 * @param {CanvasRenderingContext2D} ctx - The 2D drawing context used to render candles on the canvas.
 *
 * @example
 * // Typical usage inside a render loop:
 * candles(candleChartRef, ctx);
 *
 * @description
 * ### Steps performed:
 * 1. Calculates the starting X-position for the first candle.
 * 2. Converts each candle's open and close prices into pixel Y-coordinates.
 * 3. Determines each candle's height and vertical placement.
 * 4. Draws filled rectangles for candle bodies using `ctx.fillRect()`.
 * 5. Outlines each candle with a black border using `ctx.strokeRect()`.
 * 6. Advances X-position for the next candle based on width and spacing.
 */
export const candles = (candleChartRef, ctx) => {

    // Define Starting X-Location
    let startingX = -(candleChartRef.current.current_pixels_between_candles / 2);
    let x = Math.floor(startingX - candleChartRef.current.width.current_X_origin);

    candleChartRef.current.candles.candles.forEach(item => {

        // Convert Open and Close Price into Pixels
        let candle_open_pixel = utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_open);
        const candle_close_pixel = utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_close);

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

        // Draw Candle Border
        ctx.strokeRect(x, candle_open_pixel, width, candle_height);

        // Update Current X-Location
        x -= candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles;
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


