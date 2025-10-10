



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
