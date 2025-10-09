import * as tools from './tools.js';

export const reset_candle_canvas = (canvas_chart) => {
    const canvas = canvas_chart.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    canvas.style.width = '100%';
    canvas.style.height = '100%';
    canvas.width = canvas.offsetWidth;
    canvas.height = canvas.offsetHeight;
    return { canvas, ctx };
}
export const reset_price_canvas = (canvas_price) => {
    const cp = canvas_price.current;
    const ctx_price = cp.getContext('2d');
    if (!ctx_price) return;
    cp.style.width = '100%';
    cp.style.height = '100%';
    cp.width = cp.offsetWidth;
    cp.height = cp.offsetHeight;
    return { cp, ctx_price };
}
export const reset_date_canvas = (canvas_dates) => {
    const canvas_date = canvas_dates.current;
    const ctx_date = canvas_date.getContext('2d');
    if (!ctx_date) return;
    canvas_date.style.width = '100%';
    canvas_date.style.height = '100%';
    canvas_date.width = canvas_date.offsetWidth;
    canvas_date.height = canvas_date.offsetHeight;
    return { canvas_date, ctx_date }
}
export const handle_BaselineY = (candleChartRef) => {

    let new_bottom = 
        (candleChartRef.current.candles.starting_candle_Y + 
            candleChartRef.current.height.startingBaselineY ) - 
                (candleChartRef.current.height.startingBaselineY /2)

    candleChartRef.current.height.previousBaselineY = new_bottom
    candleChartRef.current.height.startingBaselineY = new_bottom
    candleChartRef.current.height.currentBaselineY = new_bottom

    return candleChartRef.current
}
export const drawGrid = (candleChartRef, ctx, canvas) => {
    ctx.save()
    ctx.beginPath();
    ctx.strokeStyle = 'gray';
    ctx.lineWidth = .2;
            
    for (
        let y = candleChartRef.current.height.currentBaselineY; 
        y > 0; 
        y -= candleChartRef.current.price.current_pixels_per_price_unit
    ) {
        const yPos = Math.floor(y);
        ctx.moveTo(0, yPos);
        ctx.lineTo(canvas.width, yPos);
    }
    
    ctx.stroke();
    ctx.restore()
};
export const draw_x_grid = (candleChartRef, ctx, canvas)=>{

    ctx.save()
    ctx.beginPath();
    ctx.strokeStyle = 'gray';
    ctx.lineWidth = .2;


    let starting_x_loc = -(candleChartRef.current.candles.complete_width/2)
    let ending_x_loc = -(candleChartRef.current.width.grid_width - candleChartRef.current.width.current_X_origin)

    for (let y = starting_x_loc; y > ending_x_loc; y -= candleChartRef.current.x_grid_width) {
        const yPos = y;
        ctx.moveTo(yPos - candleChartRef.current.width.current_X_origin, 0);
        ctx.lineTo(yPos - candleChartRef.current.width.current_X_origin, canvas.height);
    }
    
    ctx.stroke();
    ctx.restore()

}
export const drawCandles = (candleChartRef, ctx) => {
  let startingX = -(candleChartRef.current.current_pixels_between_candles / 2);

  candleChartRef.current.candles.candles.forEach(item => {
    const x = Math.floor(startingX - candleChartRef.current.width.current_X_origin);
    const y = Math.floor(candleChartRef.current.height.currentBaselineY - (item.candle_open * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.price.counter))
    );

    const width = -candleChartRef.current.current_candle_width;
    const height = -Math.floor(item.current_height);

    // Fill the candle body
    ctx.fillStyle = item.color;
    ctx.fillRect(x, y, width, height);

    // Add a white border
    ctx.strokeStyle = "black";
    ctx.lineWidth = 2; // adjust thickness if desired (e.g., 1.5 for sharper look)
    ctx.strokeRect(x, y, width, height);

    startingX -= candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles;
  });
};
export const drawWicks = (candleChartRef, ctx) => {
    
    let startingX = -(candleChartRef.current.candles.complete_width / 2)
    candleChartRef.current.candles.candles.forEach(item => {
        const x = Math.floor(startingX - candleChartRef.current.width.current_X_origin);
        const yHigh = Math.floor(candleChartRef.current.height.currentBaselineY - ( item.high * (candleChartRef.current.price.current_pixels_per_price_unit/candleChartRef.current.price.counter)));
        const yLow = Math.floor(candleChartRef.current.height.currentBaselineY - ( item.low * (candleChartRef.current.price.current_pixels_per_price_unit/candleChartRef.current.price.counter)));
        ctx.save()
        ctx.beginPath(); // start fresh path per wick
        ctx.strokeStyle = item.color;
        ctx.lineWidth = 1;
        ctx.moveTo(x, yHigh);
        ctx.lineTo(x, yLow);
        ctx.stroke();
        ctx.restore();
    
        startingX -= candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles;
    });
    
};
export const drawPrices = (candleChartRef, ctx_price, cp) => {
            
    const start_pixel = candleChartRef.current.height.currentBaselineY
    const font_size = Math.floor(cp.width / 4);
    const x_position = cp.width / 4;

    ctx_price.font = `${font_size}px Source Sans Pro`;
    ctx_price.fillStyle = 'gray';
    
    
    let price = 0
    for (let y = start_pixel; y >= 0; y -= candleChartRef.current.price.current_pixels_per_price_unit) {
        ctx_price.fillText(price.toFixed(2), x_position, y + 8); 
        price += candleChartRef.current.price.counter;
    }
}
export const highlight_selected_pattern = (candleChartRef, ctx, canvas) => {

    const start = (candleChartRef.current.pattern.highlighter.x_orgin);
    const end = (candleChartRef.current.pattern.highlighter.x_orgin) + (candleChartRef.current.candles.complete_width * (candleChartRef.current.pattern.length+2));

    // const end = (candleChartRef.current.pattern.highlighter.x_orgin) + (candleChartRef.current.candles.complete_width * 10);

    // --- fill background between lines ---
    ctx.save();
    ctx.fillStyle = "rgba(0, 128, 128, 0.2)";
    ctx.fillRect(start, 0, end - start, canvas.height);
    ctx.restore();

    // --- first line ---
    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = "teal";
    ctx.lineWidth = 1;
    ctx.moveTo(start, 0);
    ctx.lineTo(start, canvas.height);
    ctx.stroke();
    ctx.restore();

    // --- second line ---
    ctx.save();
    ctx.beginPath();
    ctx.strokeStyle = "teal";
    ctx.lineWidth = 1;
    ctx.moveTo(end, 0);
    ctx.lineTo(end, canvas.height);
    ctx.stroke();
    ctx.restore();
};
export const display_mid_point = (canvas, ctx, ctx_price, candleChartRef) => {

    let num = (candleChartRef.current.height.currentBaselineY - (candleChartRef.current.height.startingBaselineY/2))/ (candleChartRef.current.price.current_pixels_per_price_unit/candleChartRef.current.price.counter)

    // let num = parseFloat(candleChartRef.current.price.current_mid_price * candleChartRef.current.price.counter)
    const { width, height } = ctx_price.canvas;

    // Price Tag
    ctx_price.beginPath(); 
    ctx_price.fillStyle ="darkred";
    ctx_price.fillRect(0, (height/2)+7.5, width, -15);
    ctx_price.fillStyle = "white";
    ctx_price.font = '20px Source Sans Pro';
    ctx_price.fillText(num.toFixed(2),  width/4, (height/2)+6);
    ctx_price.stroke();
    ctx_price.restore();

    // 
    ctx.save()
    ctx.beginPath();
    ctx.lineWidth = 1
    ctx.strokeStyle = 'red'
    ctx.setLineDash([5, 5]); 
    ctx.moveTo(0,height/2);    
    ctx.lineTo(canvas.width,height/2);  
    ctx.stroke();
    ctx.restore();

    ctx.save()
    ctx.beginPath();
    ctx.lineWidth = 1
    ctx.strokeStyle = 'red'
    ctx.setLineDash([5, 5]); 
    ctx.moveTo(canvas.width/2,0);    
    ctx.lineTo(canvas.width/2,height);  
    ctx.stroke();
    ctx.restore();

    
}
export const draw_Y_price_tag = (candleChartRef, canvas, ctx_price) =>{
            
    let x_mouse_location = candleChartRef.current.mouse.pos.y

    let x = 0
    let width = canvas.width
    let height = 25
    ctx_price.beginPath(); 
    ctx_price.fillStyle = "#151c20e0";
    ctx_price.fillStyle ="#151c20e0";

    ctx_price.fillRect(x, x_mouse_location- (height/2), width, height);
    ctx_price.stroke();
    ctx_price.restore();
}

export const zoomOut = (candleChartRef, threshold) => {

   
    tools.update_zoom_out_info(candleChartRef)
    tools.zoom_out_threshold(candleChartRef, threshold)
    tools.updateCandles(candleChartRef)

};

export const zoomIn = (candleChartRef, expand_threshold) => {
                
    
    // candleChartRef.current.zoom.current -= 1;
    tools.update_zoom_in_info(candleChartRef)
    tools.zoom_in_threshold(candleChartRef, expand_threshold)
    
    
    const add_shrink_expand_to_candle_top = (obj) => {
        let res = (obj.candle_close - obj.candle_open) / candleChartRef.current.unit_amount
        res = res * candleChartRef.current.price.current_pixels_per_price_unit
        return Math.trunc(res);
                    
    }
    const add_shrink_expand_to_candle = (price) => {
        let res = price / candleChartRef.current.unit_amount
        res = res * candleChartRef.current.price.current_pixels_per_price_unit
        res = res - candleChartRef.current.height.startingBaselineY
        res = res + candleChartRef.current.zoom.shrink_expand_height

        return Math.trunc(res); 
    }
    candleChartRef.current.candles.candles = candleChartRef.current.candles.candles.map((obj) => {

        return {
            ...obj,
            current_high: add_shrink_expand_to_candle(obj.high),
            current_height: add_shrink_expand_to_candle_top(obj),
            // current_bottom: add_shrink_expand_to_candle(obj.open),
            current_low: add_shrink_expand_to_candle(obj.low)
        };
    });
    
    
};

















//  const draw_X_letter = () => {

//                 ctx.save();

//                 // --- A Coordinates ---
//                 let a_y_loc = (abcd.x_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.x_date;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 // Set font and measure text
//                 const fontSize = 14;
//                 ctx.font = `${fontSize}px Arial`;
//                 const text = "X";
//                 const textMetrics = ctx.measureText(text);
//                 const padding = 4;

//                 const textWidth = textMetrics.width;
//                 const textHeight = fontSize; // Approximate height since canvas doesn't give this directly

//                 // Draw background box
//                 ctx.fillStyle = "orange";
//                 ctx.fillRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw border (optional)
//                 ctx.strokeStyle = "black";
//                 ctx.strokeRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw the letter A
//                 ctx.fillStyle = "black";
//                 ctx.fillText(text, a_x_loc, a_y_loc);

//                 ctx.restore();

//             }

// const draw_A_letter = () => {    

//                 ctx.save();

//                 // --- A Coordinates ---
//                 let a_y_loc = (abcd.a_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.a;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 // Set font and measure text
//                 const fontSize = 14;
//                 ctx.font = `${fontSize}px Arial`;
//                 const text = "A";
//                 const textMetrics = ctx.measureText(text);
//                 const padding = 4;

//                 const textWidth = textMetrics.width;
//                 const textHeight = fontSize; // Approximate height since canvas doesn't give this directly

//                 // Draw background box
//                 ctx.fillStyle = "orange";
//                 ctx.fillRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw border (optional)
//                 ctx.strokeStyle = "black";
//                 ctx.strokeRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw the letter A
//                 ctx.fillStyle = "black";
//                 ctx.fillText(text, a_x_loc, a_y_loc);

//                 ctx.restore();

//             }
//             const draw_B_letter = () => {

//                 ctx.save();

//                 // --- A Coordinates ---
//                 let a_y_loc = (abcd.b_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.b;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 // Set font and measure text
//                 const fontSize = 14;
//                 ctx.font = `${fontSize}px Arial`;
//                 const text = "B";
//                 const textMetrics = ctx.measureText(text);
//                 const padding = 4;

//                 const textWidth = textMetrics.width;
//                 const textHeight = fontSize; // Approximate height since canvas doesn't give this directly

//                 // Draw background box
//                 ctx.fillStyle = "orange";
//                 ctx.fillRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw border (optional)
//                 ctx.strokeStyle = "black";
//                 ctx.strokeRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw the letter A
//                 ctx.fillStyle = "black";
//                 ctx.fillText(text, a_x_loc, a_y_loc);

//                 ctx.restore();

//             }
//             const draw_C_letter = () => {

//                 ctx.save();

//                 // --- A Coordinates ---
//                 let a_y_loc = (abcd.c_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.c;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 // Set font and measure text
//                 const fontSize = 14;
//                 ctx.font = `${fontSize}px Arial`;
//                 const text = "C";
//                 const textMetrics = ctx.measureText(text);
//                 const padding = 4;

//                 const textWidth = textMetrics.width;
//                 const textHeight = fontSize; // Approximate height since canvas doesn't give this directly

//                 // Draw background box
//                 ctx.fillStyle = "orange";
//                 ctx.fillRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw border (optional)
//                 ctx.strokeStyle = "black";
//                 ctx.strokeRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw the letter A
//                 ctx.fillStyle = "black";
//                 ctx.fillText(text, a_x_loc, a_y_loc);

//                 ctx.restore();

//             }
//             const draw_D_letter = () => {

//                 ctx.save();

//                 // --- A Coordinates ---
//                 let a_y_loc = (abcd.d_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.d;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 // Set font and measure text
//                 const fontSize = 14;
//                 ctx.font = `${fontSize}px Arial`;
//                 const text = "D";
//                 const textMetrics = ctx.measureText(text);
//                 const padding = 4;

//                 const textWidth = textMetrics.width;
//                 const textHeight = fontSize; // Approximate height since canvas doesn't give this directly

//                 // Draw background box
//                 ctx.fillStyle = "orange";
//                 ctx.fillRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw border (optional)
//                 ctx.strokeStyle = "black";
//                 ctx.strokeRect(
//                     a_x_loc - padding,
//                     a_y_loc - textHeight - padding,
//                     textWidth + padding * 2,
//                     textHeight + padding * 2
//                 );

//                 // Draw the letter A
//                 ctx.fillStyle = "black";
//                 ctx.fillText(text, a_x_loc, a_y_loc);

//                 ctx.restore();

//             }


//             const draw_price_levels = () => {
//                             ctx.save();

//                             // Calculate all y positions
//                             const y_stop_loss = -( (abcd.stop_loss / unit_amount.current) * current_pixels_per_price_unit.current - starting_canvas_height.current ) - shrink_expand_height.current - current_y_spacing.current;
//                             const y_take_profit = -( (abcd.take_profit / unit_amount.current) * current_pixels_per_price_unit.current - starting_canvas_height.current ) - shrink_expand_height.current - current_y_spacing.current;
//                             const y_entered_price = -( (abcd.entered_price / unit_amount.current) * current_pixels_per_price_unit.current - starting_canvas_height.current ) - shrink_expand_height.current - current_y_spacing.current;

//                             // Calculate horizontal start and end positions
//                             let x_start = -chart.current.current_full_candle_width * abcd.d;
//                             x_start -= current_x_spacing.current;
//                             x_start += chart.current.current_full_candle_width / 2;

//                             let x_end = -chart.current.current_full_candle_width * abcd.exit_date;
//                             x_end -= current_x_spacing.current;
//                             x_end += chart.current.current_full_candle_width / 2;

//                             // Draw stop loss line
//                             ctx.beginPath();
//                             ctx.strokeStyle = '#ef5350';
//                             ctx.lineWidth = 3;
//                             ctx.moveTo(x_start, y_stop_loss);
//                             ctx.lineTo(x_end, y_stop_loss);
//                             ctx.stroke();

//                             // Draw take profit line
//                             ctx.beginPath();
//                             ctx.strokeStyle = '#26a69a';
//                             ctx.lineWidth = 3;
//                             ctx.moveTo(x_start, y_take_profit);
//                             ctx.lineTo(x_end, y_take_profit);
//                             ctx.stroke();

//                             // Draw entered price line
//                             ctx.beginPath();
//                             ctx.strokeStyle = 'white';
//                             ctx.lineWidth = 3;
//                             ctx.moveTo(x_start, y_entered_price);
//                             ctx.lineTo(x_end, y_entered_price);
//                             ctx.stroke();

//                             // Fill area above entered price line with teal (take profit zone)
//                             let topFillY = y_take_profit;
//                             let heightTop = y_entered_price - y_take_profit;
//                             if (heightTop > 0) {  // sanity check so height is positive
//                                 ctx.fillStyle = 'rgba(38, 166, 154, 0.2)';  // semi-transparent teal
//                                 ctx.fillRect(x_start, topFillY, x_end - x_start, heightTop);
//                             }

//                             // Fill area below entered price line with red (stop loss zone)
//                             let bottomFillY = y_entered_price;
//                             let heightBottom = y_stop_loss - y_entered_price;
//                             if (heightBottom > 0) {
//                                 ctx.fillStyle = 'rgba(239, 83, 80, 0.2)';  // semi-transparent red
//                                 ctx.fillRect(x_start, bottomFillY, x_end - x_start, heightBottom);
//                             }

//                             ctx.restore();
//             }
//             const draw_ab_price = () => {

//                 ctx.save()
//                 let a_y_loc = (abcd.a_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 a_y_loc = -(a_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let a_x_loc = -chart.current.current_full_candle_width * abcd.a;
//                 a_x_loc -= current_x_spacing.current;
//                 a_x_loc += chart.current.current_full_candle_width / 2;

//                 let b_y_loc = (abcd.b_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 b_y_loc = -(b_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let b_x_loc = -chart.current.current_full_candle_width * abcd.b;
//                 b_x_loc -= current_x_spacing.current;
//                 b_x_loc += chart.current.current_full_candle_width / 2;

//                 let c_y_loc = (abcd.c_price / unit_amount.current) * current_pixels_per_price_unit.current;
//                 c_y_loc = -(c_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 let c_x_loc = -chart.current.current_full_candle_width * abcd.c;
//                 c_x_loc -= current_x_spacing.current;
//                 c_x_loc += chart.current.current_full_candle_width / 2;

//                 let d_y_loc = ((abcd.c_price-abcd.ab_price_length) / unit_amount.current) * current_pixels_per_price_unit.current;
//                 d_y_loc = -(d_y_loc - starting_canvas_height.current) - shrink_expand_height.current - current_y_spacing.current;

//                 ctx.beginPath();


//                 // AB
//                 ctx.moveTo(a_x_loc, a_y_loc);
//                 ctx.lineTo(a_x_loc, b_y_loc);

//                 ctx.moveTo(c_x_loc, c_y_loc);
//                 ctx.lineTo(c_x_loc, d_y_loc);



//                 // --- Optional stroke on top ---
    
//                 ctx.lineWidth = 5;
//                 ctx.strokeStyle = 'white';
//                 ctx.lineWidth = 3
//                 ctx.setLineDash([5, 5]); 

//                 ctx.stroke();
//                 ctx.restore()


//             }