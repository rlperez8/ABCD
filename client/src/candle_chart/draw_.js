import * as utilites from './utilities.js'

export class Mouse {

    constructor(candleChartRef){
        this.candleChartRef = candleChartRef
    }
    mouse_Y = (canvas, ctx) => {
        ctx.save()
        let x_mouse_location = this.candleChartRef.current.mouse.pos.y
        ctx.beginPath();
        ctx.lineWidth = .5
        ctx.strokeStyle = 'white'
        ctx.setLineDash([5, 5]); 
        ctx.moveTo(0,x_mouse_location);    
        ctx.lineTo(canvas.width,x_mouse_location);  

        ctx.stroke();
        ctx.restore();
    };
    mouse_X = (canvas, ctx, set_candle_high, set_candle_close, set_candle_open, set_candle_low, current_hovered_candle_index, set_candle_color) => {

        ctx.save()

        // Get Hovered Candle Index Pixel Location
        let mouse_x_loc = this.candleChartRef.current.mouse.pos.x
        let mouse_x_loc_with_x_spacing = Math.floor(-(mouse_x_loc + this.candleChartRef.current.width.current_X_origin))
        let index = Math.floor(mouse_x_loc_with_x_spacing/this.candleChartRef.current.candles.complete_width) + 1
        current_hovered_candle_index.current = index 
        let pixelStart = (index * (this.candleChartRef.current.candles.complete_width)) - (this.candleChartRef.current.candles.complete_width/2)

        ctx.beginPath(); 
        ctx.lineWidth = .5
        ctx.strokeStyle = 'white'
        ctx.setLineDash([5, 5]); 
        ctx.moveTo(-pixelStart - this.candleChartRef.current.width.current_X_origin, canvas.height);    
        ctx.lineTo(-pixelStart - this.candleChartRef.current.width.current_X_origin, 0); 
        ctx.stroke();

        ctx.restore();

        // Set Hovered Candle Data
        const hoveredCandle = this.candleChartRef.current.candles.candles[index-1];
        set_candle_high(hoveredCandle?.candle_high)
        set_candle_close(hoveredCandle?.candle_close)
        set_candle_open(hoveredCandle?.candle_open)
        set_candle_low( hoveredCandle?.candle_low)
        set_candle_color(hoveredCandle?.candle_open > hoveredCandle?.candle_close ? '#ef5350' : hoveredCandle?.candle_open < hoveredCandle?.candle_close ? '#26a69a' : '')
 
    }
    mouse_price = (canvas, ctx_price) => {
        ctx_price.save();

        // Flip Mouse Y-Coordinate (convert canvas coordinate to chart coordinate)
        const flipped_y_coord = Math.abs(
            this.candleChartRef.current.mouse.pos.y - this.candleChartRef.current.height.currentBaselineY
        );

        // Convert flipped Y-Coordinate into a chart price coordinate
        const y_loc_price = flipped_y_coord / this.candleChartRef.current.price.current_pixels_per_price_unit;

        // Compute the actual price
        const price = y_loc_price * this.candleChartRef.current.unit_amount;

        let font_size = Math.floor(canvas.width / 4)
        // Draw the price value near the mouse cursor
        ctx_price.font = `${font_size}px Source Sans Pro`;
        ctx_price.fillStyle = "#FFFFFF"; 
        ctx_price.textBaseline = "middle";
        ctx_price.fillText(price.toFixed(2), font_size, this.candleChartRef.current.mouse.pos.y);

        ctx_price.restore();
    };
    price_background = (canvas, ctx_price) =>{
    ctx_price.save();
    let x_mouse_location =  this.candleChartRef.current.mouse.pos.y

    let x = 0
    let width = canvas.width
    let height = 25
    ctx_price.beginPath(); 
    ctx_price.fillStyle = "#1c1c1ce0";

    
    ctx_price.fillRect(x, x_mouse_location- (height/2), width, height);
    ctx_price.stroke();
    ctx_price.restore();
    }
    mouse_date = (canvas_date, ctx_date) => {
       
        ctx_date.save()
        let mouse_x_loc = this.candleChartRef.current.mouse.pos.x;
        let mouse_x_loc_with_x_spacing = -(mouse_x_loc + this.candleChartRef.current.width.current_X_origin);

        // Track Candle Width
        let full_candle_width = this.candleChartRef.current.candles.complete_width;

        // Track Index
        let index = Math.floor(mouse_x_loc_with_x_spacing / full_candle_width);

        let pixelStart = (index * full_candle_width) + (full_candle_width / 2);
        pixelStart = pixelStart - 2.5;

        // Get Hovered Candle Date
        const hoveredCandle = this.candleChartRef.current.candles.candles[index]?.candle_date;

        // ✅ Just strip time, keep full original string
        const finalFormat = hoveredCandle?.split(" 00:")[0] || "";

        const metrics = ctx_date.measureText(finalFormat);
        const textHeight = metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent;

        let y_text = 30;
        if (index >= 0) {
            ctx_date.beginPath();
            ctx_date.font = "16px Arial";
            ctx_date.fillStyle = "white";
            ctx_date.fillText(
                finalFormat,
                (-pixelStart - this.candleChartRef.current.width.current_X_origin) - 65,
                (canvas_date.height / 2) + textHeight / 2
            );
            ctx_date.stroke();
        }
        ctx_date.restore()
    };
    date_background = (ctx_date, candle_width, canvas_date) => {
        ctx_date.save()
        
        // Find Current x-loc
        let mouse_x_loc = this.candleChartRef.current.mouse.pos.x
        let mouse_x_loc_with_x_spacing = -(mouse_x_loc + this.candleChartRef.current.width.current_X_origin)
        // Track Candle Width
        // let full_candle_width = candle_width.current + 5
        // let full_candle_width = candleChartRef.current.current_candle_width + 5
        let full_candle_width = this.candleChartRef.current.candles.complete_width
        // Track X-Spacing
        let spacing_in_candles = this.candleChartRef.current.width.current_X_origin / full_candle_width
        // Track Index
        let index = Math.floor(mouse_x_loc_with_x_spacing/full_candle_width)
        mouse_x_loc = (mouse_x_loc - this.candleChartRef.current.width.current_X_origin);
        mouse_x_loc = mouse_x_loc - (candle_width.current / 2);
        let pixelStart = (index * full_candle_width) + (full_candle_width/2)
        pixelStart = pixelStart - 2.5
        let x_rect = this.candleChartRef.current.width.current_X_origin + (pixelStart - 75) - candle_width.current;
        let y_rect = 0;
        let width_rect = 150;
        let height_rect = 40;
        if(index>=0){
            ctx_date.beginPath();
            // ctx_date.fillStyle = "teal";
            ctx_date.fillStyle = "#1c1d1de0";
            ctx_date.fillRect( (-pixelStart - this.candleChartRef.current.width.current_X_origin)-80, y_rect, width_rect, canvas_date.height);
            ctx_date.stroke();
            
        }
        ctx_date.restore()
        
        
    }
}
export class Chart {

    constructor(candleChartRef){
        this.candleChartRef = candleChartRef
    }
    grid_X  = (ctx, canvas)=>{

        ctx.save()
        ctx.beginPath();
        ctx.strokeStyle = 'gray';
        ctx.lineWidth = .2;


        let starting_x_loc = -(this.candleChartRef.current.candles.complete_width/2)
        let ending_x_loc = -(this.candleChartRef.current.width.grid_width - this.candleChartRef.current.width.current_X_origin)

        for (let y = starting_x_loc; y > ending_x_loc; y -= this.candleChartRef.current.x_grid_width) {
            const yPos = y;
            ctx.moveTo(yPos - this.candleChartRef.current.width.current_X_origin, 0);
            ctx.lineTo(yPos - this.candleChartRef.current.width.current_X_origin, canvas.height);
        }
        
        ctx.stroke();
        ctx.restore()

    }
    grid_Y = (ctx, canvas) => {
        ctx.save()
        ctx.beginPath();
        ctx.strokeStyle = 'gray';
        ctx.lineWidth = .2;
                
        for (let y = this.candleChartRef.current.height.currentBaselineY; y > 0; y -= this.candleChartRef.current.price.current_pixels_per_price_unit) {
            const yPos = Math.floor(y);
            ctx.moveTo(0, yPos);
            ctx.lineTo(canvas.width, yPos);
        }
        
        ctx.stroke();
        ctx.restore()
    };
    prices = (ctx_price, cp) => {
            
        const start_pixel = this.candleChartRef.current.height.currentBaselineY
        const font_size = Math.floor(cp.width / 4);
        const x_position = cp.width / 4;

        ctx_price.font = `${font_size}px Source Sans Pro`;
        ctx_price.fillStyle = 'gray';
        
        
        let price = 0
        for (let y = start_pixel; y >= 0; y -= this.candleChartRef.current.price.current_pixels_per_price_unit) {
            ctx_price.fillText(price.toFixed(2), x_position, y + 8); 
            price += this.candleChartRef.current.unit_amount;
        }
    }
    dates = (ctx_date,canvas_date) => {

        // REMOVE DRAW FOR THIS FUNCTION WHEN JUST MOUSE MOVES
        let startingX = -(this.candleChartRef.current.current_candle_width / 2);
        let candle_index = 0;

        this.candleChartRef.current.candles.candles.forEach(item => {
            const x = Math.floor(startingX - this.candleChartRef.current.width.current_X_origin);

            // ✅ keep the full date string, but cut off the time part
            const date = this.candleChartRef.current.candles.candles[candle_index]?.candle_date.split(" 00:")[0];

            const metrics = ctx_date.measureText(date);
            const textHeight = metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent;

            ctx_date.save();
            ctx_date.beginPath();
            ctx_date.fillText(date, x - 25, (canvas_date.height / 2) + textHeight / 2);
            ctx_date.stroke();
            ctx_date.restore();

            startingX -= this.candleChartRef.current.x_grid_width;
            let current_index = -Math.trunc(startingX / this.candleChartRef.current.candles.complete_width);
            candle_index = current_index;
        });
    }; 
    candles = (ctx) => {
    
        let candle_X = -(this.candleChartRef.current.current_pixels_between_candles / 2);
        candle_X = Math.floor(candle_X - this.candleChartRef.current.width.current_X_origin);

        let wick_X = -(this.candleChartRef.current.candles.complete_width / 2)
        wick_X = Math.floor(wick_X - this.candleChartRef.current.width.current_X_origin);

        this.candleChartRef.current.candles.candles.forEach(item => {

            this._draw_candle(item, this.candleChartRef, ctx, candle_X)
            this._draw_wick(item, this.candleChartRef, ctx, wick_X)

            candle_X -= this.candleChartRef.current.current_candle_width + this.candleChartRef.current.current_pixels_between_candles;
            wick_X -= this.candleChartRef.current.current_candle_width + this.candleChartRef.current.current_pixels_between_candles;
    })};
    pattern_center = (ctx, canvas) => {
        ctx.fillStyle = 'red';      // Fill color
        ctx.strokeStyle = 'red';  // Border color
        ctx.lineWidth = 2;

        const x = canvas.width / 2.8;
        const y = canvas.height / 2.8;
        const width = canvas.width / 2.8;
        const height = canvas.height / 2.8; 

        ctx.strokeRect(x, y, width, height);
    }
    _draw_candle = (item, candleChartRef, ctx, x) => {
            // Convert Open and Close Price into Pixels
            let candle_open_pixel = utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_open);
            const candle_close_pixel = utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_close);

            // Get Candle Height
            let candle_height = candle_open_pixel - candle_close_pixel;

            // Get Price Location on Canvas
            candle_open_pixel = Math.floor(candleChartRef.current.height.currentBaselineY - candle_open_pixel);    

            // Get Candle Width
            const width = -candleChartRef.current.current_candle_width;

            // Set Border Color
            ctx.strokeStyle = item.candle_close > item.candle_open ? '#008080' : '#bf4240';
            ctx.lineWidth = 2;

            // Draw Only the Outline (no fill)
            ctx.strokeRect(x, candle_open_pixel, width, candle_height);

            // Optional: draw wicks separately if needed
        }
    _draw_wick = (item, candleChartRef, ctx, x) => {
    
        const yHigh = Math.floor(candleChartRef.current.height.currentBaselineY - utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_high));
        const yLow = Math.floor(candleChartRef.current.height.currentBaselineY - utilites.get_pixel_location_of_a_price(candleChartRef, item.candle_low));
        ctx.save()
        ctx.beginPath(); // start fresh path per wick
        ctx.strokeStyle = item.color;
        ctx.lineWidth = 1;
        ctx.moveTo(x, yHigh);
        ctx.lineTo(x, yLow);
        ctx.stroke();
        ctx.restore();
    }

}
export class ABCD {
    
    constructor(candleChartRef){
        this.candleChartRef = candleChartRef
    }
    abcd = (ctx, ab, abcd) => {
 
        // A
        let a_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.a_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
        let a_x_loc = -this.candleChartRef.current.candles.complete_width * ab.a;
        a_x_loc -= this.candleChartRef.current.width.current_X_origin;
        a_x_loc += this.candleChartRef.current.candles.complete_width / 2;


        
        // B
        let b_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.b_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
        let b_x_loc = -this.candleChartRef.current.candles.complete_width * ab.b;
        b_x_loc -= this.candleChartRef.current.width.current_X_origin;
        b_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // C
        let c_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.c_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
        let c_x_loc = -this.candleChartRef.current.candles.complete_width * ab.c;
        c_x_loc -= this.candleChartRef.current.width.current_X_origin;
        c_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // D
        let d_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.d_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
        let d_x_loc = -this.candleChartRef.current.candles.complete_width * ab.d;
        d_x_loc -= this.candleChartRef.current.width.current_X_origin;
        d_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // Exit
        let exit_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.exit_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

    
        let exit_x_loc = -this.candleChartRef.current.candles.complete_width * ab.exit_date;
        exit_x_loc -= this.candleChartRef.current.width.current_X_origin;
        exit_x_loc += this.candleChartRef.current.candles.complete_width / 2;
        
        
        
        ctx.beginPath();
        ctx.lineTo(a_x_loc, a_y_loc);
        ctx.lineTo(b_x_loc, b_y_loc);
        ctx.lineTo(c_x_loc, c_y_loc);
        ctx.lineTo(d_x_loc, d_y_loc);
        ctx.lineTo(exit_x_loc, exit_y_loc);
        ctx.strokeStyle = 'rgb(48, 48, 48);';
        ctx.lineWidth = 5;
        ctx.stroke();

            // --- Optional stroke on top ---
        ctx.strokeStyle = '#303030';
        ctx.lineWidth = 2;

        ctx.stroke();
        ctx.restore()

        ctx.save();

        //  return 

        // --- Common Values ---
        const arrowLength = 20;
        const arrowAngle = Math.PI / 6;

        // --- Calculate angle of the line ---
        const angle = Math.atan2(exit_y_loc - d_y_loc, exit_x_loc - d_x_loc);

        // --- Calculate shortened end point for line (before arrow) ---
        const shortened_x = exit_x_loc - arrowLength * Math.cos(angle);
        const shortened_y = exit_y_loc - arrowLength * Math.sin(angle);

        // --- Draw line (stops before arrowhead) ---
        ctx.beginPath();
        ctx.moveTo(d_x_loc, d_y_loc);
        ctx.lineTo(shortened_x, shortened_y);
        ctx.strokeStyle = abcd.result === 'Win' ? '#303030' : '#303030';
        ctx.lineWidth = 3;
        ctx.shadowColor = abcd.result === 'Win' ? '#303030' : '#303030'; // match line color
        ctx.shadowBlur = 25
        ctx.stroke();

        // --- Draw Arrowhead ---
        ctx.beginPath();
        ctx.moveTo(exit_x_loc, exit_y_loc);
        ctx.lineTo(
        exit_x_loc - arrowLength * Math.cos(angle - arrowAngle),
        exit_y_loc - arrowLength * Math.sin(angle - arrowAngle)
        );
        ctx.lineTo(
        exit_x_loc - arrowLength * Math.cos(angle + arrowAngle),
        exit_y_loc - arrowLength * Math.sin(angle + arrowAngle)
        );
        ctx.lineTo(exit_x_loc, exit_y_loc); // Back to tip
        ctx.closePath();

        ctx.fillStyle = 'white'; // visible arrowhead color
        ctx.fill();

        ctx.restore();
        
    }
    retracement = (ctx, ab) => {
                
        ctx.save()
        // --- A Coordinates ---
        let a_y_loc =  this.candleChartRef.current.height.currentBaselineY - (ab.a_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

        let a_x_loc = -this.candleChartRef.current.candles.complete_width * ab.a;
        a_x_loc -= this.candleChartRef.current.width.current_X_origin;
        a_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // --- B Coordinates ---
        let b_y_loc = this.candleChartRef.current.height.currentBaselineY - (ab.b_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

        let b_x_loc = -this.candleChartRef.current.candles.complete_width * ab.b;
        b_x_loc -= this.candleChartRef.current.width.current_X_origin;
        b_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // --- C Coordinates ---
        let c_y_loc = this.candleChartRef.current.height.currentBaselineY - (ab.c_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

        let c_x_loc = -this.candleChartRef.current.candles.complete_width * ab.c;
        c_x_loc -= this.candleChartRef.current.width.current_X_origin;
        c_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // --- D Coordinates ---
        let d_y_loc = this.candleChartRef.current.height.currentBaselineY - (ab.d_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

        let d_x_loc = -this.candleChartRef.current.candles.complete_width * ab.d;
        d_x_loc -= this.candleChartRef.current.width.current_X_origin;
        d_x_loc += this.candleChartRef.current.candles.complete_width / 2;

        // --- Draw Lines ---
        ctx.beginPath();
        ctx.strokeStyle = 'yellow';
        ctx.lineWidth = 3
        ctx.setLineDash([5, 5]); 

        // AB
        ctx.moveTo(a_x_loc, a_y_loc);
        ctx.lineTo(c_x_loc, c_y_loc);

        ctx.moveTo(b_x_loc, b_y_loc);
        ctx.lineTo(d_x_loc, d_y_loc);


        // --- Optional stroke on top ---
        ctx.strokeStyle = 'gray';
        // ctx.lineWidth = 7;

        ctx.stroke();
        ctx.restore()

    }
    price_levels = (ctx_price, ctx, canvas, ab) => {
                ctx.save();

                // // Calculate all y positions
                const y_stop_loss = this.candleChartRef.current.height.currentBaselineY - (ab.stop_loss  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
                const y_take_profit = this.candleChartRef.current.height.currentBaselineY - (ab.take_profit  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))
                const y_entered_price = this.candleChartRef.current.height.currentBaselineY - (ab.entered_price  * (this.candleChartRef.current.price.current_pixels_per_price_unit / this.candleChartRef.current.unit_amount))

                // Calculate horizontal start and end positions
                let x_start = -this.candleChartRef.current.candles.complete_width * ab.d;
                x_start -= this.candleChartRef.current.width.current_X_origin;
                x_start += this.candleChartRef.current.candles.complete_width / 2;

                let x_end = -this.candleChartRef.current.candles.complete_width * ab.exit_date;
                x_end -= this.candleChartRef.current.width.current_X_origin;
                x_end += this.candleChartRef.current.candles.complete_width / 2;
                

                // Draw stop loss line
                ctx.beginPath();
                ctx.strokeStyle = '#ef5350';
                ctx.lineWidth = 3;
                ctx.moveTo(x_start, y_stop_loss);
                ctx.lineTo(x_end, y_stop_loss);
                ctx.stroke();

                // Draw take profit line
                ctx.beginPath();
                ctx.strokeStyle = '#26a69a';
                ctx.lineWidth = 3;
                ctx.moveTo(x_start, y_take_profit);
                ctx.lineTo(x_end, y_take_profit);
                ctx.stroke();

                // Draw entered price line
                ctx.beginPath();
                ctx.strokeStyle = 'white';
                ctx.lineWidth = 3;
                ctx.moveTo(x_start, y_entered_price);
                ctx.lineTo(x_end, y_entered_price);
                ctx.stroke();

                // Fill area above entered price line with teal (take profit zone)
                let topFillY = y_take_profit;
                let heightTop = y_entered_price - y_take_profit;
                if (heightTop > 0) {  // sanity check so height is positive
                    ctx.fillStyle = 'rgba(38, 166, 154, 0.2)';  
                    ctx.fillStyle = 'rgba(50, 50, 50, 0.2)';
                    ctx.fillStyle = 'rgba(0, 0, 0, 0.2)';
                    
                    ctx.fillRect(x_start, topFillY, x_end - x_start, heightTop);
                }

                // Fill area below entered price line with red (stop loss zone)
                let bottomFillY = y_entered_price;
                let heightBottom = y_stop_loss - y_entered_price;
                if (heightBottom > 0) {
                    ctx.fillStyle = 'rgba(239, 83, 80, 0.2)';  // semi-transparent red
                    ctx.fillRect(x_start, bottomFillY, x_end - x_start, heightBottom);
                }
                

                // ================================================

                // DRAW DASHED PRICE LINE
                let starting_ = -this.candleChartRef.current.candles.complete_width * ab.exit_date;
                starting_ -= this.candleChartRef.current.width.current_X_origin;
                starting_ += this.candleChartRef.current.candles.complete_width / 2;
                let end_ = this.candleChartRef.current.width.grid_width
                ctx.beginPath();
                ctx.strokeStyle = 'white';
                ctx.lineWidth = 1;
                ctx.setLineDash([5, 5]);   
                ctx.moveTo(starting_, y_entered_price);
                ctx.lineTo(end_, y_entered_price);
                ctx.stroke();
                ctx.setLineDash([]);

                let profit_starting_ = -this.candleChartRef.current.candles.complete_width * ab.exit_date;
                profit_starting_ -= this.candleChartRef.current.width.current_X_origin;
                profit_starting_ += this.candleChartRef.current.candles.complete_width / 2;
                let profit_end_ = this.candleChartRef.current.width.grid_width
                ctx.beginPath();
                ctx.strokeStyle = 'teal';
                ctx.lineWidth = 3;
                ctx.setLineDash([5, 5]);   
                ctx.moveTo(profit_starting_, y_take_profit);
                ctx.lineTo(profit_end_, y_take_profit);
                ctx.stroke();
                ctx.setLineDash([]);


                ctx.beginPath();
                ctx.strokeStyle = 'red';
                ctx.lineWidth = 3;
                ctx.setLineDash([5, 5]);;   
                ctx.moveTo(profit_starting_, y_stop_loss);
                ctx.lineTo(profit_end_, y_stop_loss);
                ctx.stroke();
                ctx.setLineDash([]);


                ctx.restore();

                ctx_price.beginPath(); 
          
      
                // ENTER, TAKE PROFIT, STOP LOSS PRICE MARKER
                
                // ctx_price.fillStyle = "teal";
                // ctx_price.font = '20px Source Sans Pro';
                // ctx_price.fillText(ab.take_profit, 20 , y_take_profit);
                // ctx_price.stroke();
                // ctx_price.fillStyle = "white";
                // ctx_price.font = '20px Source Sans Pro';
                // ctx_price.fillText(ab.entered_price, 20 , y_entered_price);
                // ctx_price.stroke();


                
                let x = 0;
                let width = canvas.width;
                let height = 25;

                // =================================================

                // Stop Loss Rectangle + Text
                ctx_price.fillStyle = "black";          
                ctx_price.fillRect(x, y_stop_loss - (height/2), width, height);
                // ctx_price.strokeStyle = "white";              
                ctx_price.strokeRect(x, y_stop_loss - (height/2), width, height);

                ctx_price.fillStyle = "red";                 
                ctx_price.font = '20px Source Sans Pro';
                ctx_price.fillText(ab.stop_loss, 20, y_stop_loss + height/4); 
             

                // Entered Price Rectangle + Text
                ctx_price.fillStyle = "black";      
                ctx_price.fillRect(x, y_entered_price - (height/2), width, height);
                // ctx_price.strokeStyle = "white";
                ctx_price.strokeRect(x, y_entered_price - (height/2), width, height);

                ctx_price.fillStyle = "gray";
                ctx_price.font = '20px Source Sans Pro';
                ctx_price.fillText(ab.entered_price, 20, y_entered_price + height/4);
          

                // Entered Price Rectangle + Take Profit
                ctx_price.fillStyle = "black";          
                ctx_price.fillRect(x, y_take_profit - (height/2), width, height);
                // ctx_price.strokeStyle = "white";
                ctx_price.strokeRect(x, y_take_profit - (height/2), width, height);

                ctx_price.fillStyle = "teal";
                ctx_price.font = '20px Source Sans Pro';
                ctx_price.fillText(ab.take_profit, 20, y_take_profit + height/4);
         
    }

      


}
