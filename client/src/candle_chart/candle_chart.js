import React, {useState, useRef, useEffect} from "react";
import '../new.css'
import * as CandleChartTools from '../candle_chart_tools.js';
import * as draw_ from './draw_.js'
import * as resize from './resize.js'
import * as utilites from './utilities.js'
export const Candle_Chart = (props) => {

    const {
		selected_candles,
        set_is_listing_status,
        is_listing_status,
        ticker_symbol,
        set_canvas_dimensions,
        selected_abcd,
        selected_ab,
        selected_pattern,
        is_price_levels,
        is_retracement,
        is_abcd_pattern

	} = props
    
    const canvas_dates = useRef()
    const canvas_price = useRef()
    const canvas_chart = useRef()
    const [candle_high, set_candle_high] = useState(0)
    const [candle_close, set_candle_close] = useState(0)
    const [candle_open, set_candle_open] = useState(0)
    const [candle_low, set_candle_low] = useState(0)
    const [candle_color, set_candle_color] = useState('')
    let mouse = null
    let chart = null
    let abcd_ = null
    let mouseEvents = null

    const current_hovered_candle_index = useRef(0)
    const candle_width = useRef(10)
    let [abcd, set_abcd] = useState({
        a: 0,
        b: 0,
        c: 0,
        d: 0,
        a_price: 0,
        b_price: 0,
        c_price: 0,
        d_price: 0,
        x_date: 0,
        x_price: 0,
        exit_price: 0,
        exit_date: 0,
        result: 0
    })
    let [ab, set_ab] = useState({

    })
    const candleChartRef = useRef({})

    // ===== Canvas Mount
    useEffect(() => {

        const resizeCanvas = () => {

            if (!canvas_chart.current || !canvas_price.current) return;
                
                const canvas = canvas_chart.current;
                const ctx = canvas.getContext('2d');
                if (!ctx) return;
                canvas.style.width = '100%';
                canvas.style.height = '100%';
                canvas.height = canvas.offsetHeight;
                canvas.width = canvas.offsetWidth;
                const canvas_ = canvas_price.current;
                const ctx_ = canvas_.getContext('2d');

                if (!ctx_) return;
                canvas_.style.width = '100%';
                canvas_.style.height = '100%';
                canvas_.height = canvas_.offsetHeight;
                canvas_.width = canvas_.offsetWidth;

                const canvas_date = canvas_dates.current;
                const ctx_dates = canvas_date.getContext('2d');
                if (!ctx_dates) return;
                canvas_date.style.width = '100%';
                canvas_date.style.height = '100%';
                canvas_date.height = canvas_date.offsetHeight;
                canvas_date.width = canvas_date.offsetWidth;
                
            
                const PRICE_UNIT_DIVISOR = 10;
              
                candleChartRef.current = {

                    canvas_height: canvas.offsetHeight,
                    canvas_width: canvas.offsetWidth,

                    mouse: {
                    down: { x: 0, y: 0 },   
                    pos: { x: 0, y: 0 },   
                    isPressed: false,      
                    isPressedOnCandles: false,
                    isPressedOnPrices: false
                    },
                    dimensions: {
                        canvas: {
                            candles:0,
                            prices: 0,
                            dates: 0
                        },
                        width: 0,
                        origin:{ 
                            x: {
                                previous: 0,
                                current: 0,
                                starting: 0,
                                offset: {
                                    current: 0,
                                    previous: 0
                                },
                            },
                            y: {
                                previous: 0,
                                current: 0,
                                starting: 0,
                                offset: {
                                    current: 0,
                                    previous: 0
                                },
                            },
                        },
                    },
                    candles: {
                        list: [],
                        location:{
                            y:{},
                            x:{}
                        },
                        size: {
                            spacing: 0,
                            width: 0,
                            total: 0
                        },

                        candles: selected_candles,
                        starting_candle_Y: selected_candles[0]?.open * (canvas.height / PRICE_UNIT_DIVISOR),
                        complete_width: selected_pattern?.pattern_B_end_date
                    },
                    price: {
                        unit: {
                            previous: 0,
                            start: 0,
                            current: 0,
                        },
                        mid: {
                            previous: 0,
                            start: 0,
                            current: 0,
                        },
                        counter: 0,
                        
                        
                        starting_price_unit_pixel_size: 0,
                        current_price_unit_pixel_size: 0,
                        prev_pixels_per_price_unit: 0,

                        prev_mid_price: 0,
                        current_mid_price: 0,
                        static_mid: 0,

                        current_pixels_per_price_unit: 0

                    },
                    width: {
                        style_width: "100%",
                        width: canvas.offsetWidth,
                        prev_X_origin: 0,
                        current_X_origin: 0,
                    },
                    height: {
                        style_height: "100%",
                        height: 0,
                        previousBaselineY: 0,
                        startingBaselineY: 0,
                        currentBaselineY: 0,
                        initialBaselineY: 0,
                        prev_Y_OffSet: 0,
                        current_Y_OffSet: 0,
                    },
                    zoom: {
                        current: 0,
                        shrink_expand_height: 0
                    },
                    pattern:
                    {
                        highlighter: {
                            x_orgin: canvas.width / 2,
                            previous: canvas.width / 2,
                            
                        },
                        length: selected_pattern?.pattern_AB_bar_duration
                    },
             
                    unit_amount: 0,
                    current_pixels_between_candles: 0,
                    current_candle_width: 0,
                    x_grid_increaser: 0,
                    x_grid_width: 0,
                    grid_size_count: 0
                    
                }   
                candleChartRef.current = CandleChartTools.handle_BaselineY(candleChartRef)

                // Set Canvas Height to the Height of the Parent Div Container.
                candleChartRef.current.height.startingBaselineY = canvas.offsetHeight
                candleChartRef.current.candles.candles = selected_candles
                candleChartRef.current.candles.starting_candle_Y = selected_candles[0]?.candle_open * (canvas.height  / 10)   
                candleChartRef.current.height.current_Y_OffSet =  candleChartRef.current.candles.starting_candle_Y - (candleChartRef.current.height.startingBaselineY / 2)
                candleChartRef.current.height.prev_Y_OffSet =  candleChartRef.current.candles.starting_candle_Y - (candleChartRef.current.height.startingBaselineY / 2)
                candleChartRef.current.height.currentBaselineY = candleChartRef.current.height.startingBaselineY + candleChartRef.current.height.current_Y_OffSet
                candleChartRef.current.height.previousBaselineY = candleChartRef.current.height.startingBaselineY + candleChartRef.current.height.current_Y_OffSet
    
                // Set Canvas Width to the Width of the Parent Div Container.
                candleChartRef.current.width.grid_width = canvas.width

                // Set the Size of Price Unit.
                candleChartRef.current.price.starting_price_unit_pixel_size  = canvas.offsetHeight / 10
                candleChartRef.current.price.current_price_unit_pixel_size = canvas.offsetHeight / 10
                candleChartRef.current.price.prev_pixels_per_price_unit = canvas.offsetHeight/ 10
                candleChartRef.current.price.current_pixels_per_price_unit = canvas.offsetHeight / 10
                candleChartRef.current.price.current_mid_price = selected_candles[0]?.candle_open 
                candleChartRef.current.price.prev_mid_price = selected_candles[0]?.candle_open
                candleChartRef.current.price.static_mid = selected_candles[0]?.candle_open 
        
                // ====== X-Origin
                candleChartRef.current.width.current_X_origin = -(canvas.width/2) 
                candleChartRef.current.width.prev_X_origin = -(canvas.width/2)

        
                candleChartRef.current.zoom.current = 0
                candleChartRef.current.zoom.shrink_expand_height = 0
                
                
                let pixels_between_candles = 5
                let candle_width = 11
                let full_width = candle_width + pixels_between_candles


                candleChartRef.current.unit_amount = 1
                candleChartRef.current.candles.complete_width = full_width
                candleChartRef.current.current_pixels_between_candles = pixels_between_candles
                candleChartRef.current.current_candle_width = candle_width
                candleChartRef.current.x_grid_increaser = 10
                candleChartRef.current.x_grid_width = full_width * 10
                candleChartRef.current.grid_size_count = 0

                candleChartRef.current.selected_candle = selected_candles[0]?.candle_open 

 
                set_canvas_dimensions((prev)=> ({
                    ...prev,
                    chart_height: canvas.height,
                    price_height: canvas_.height,
                    date_height: canvas_date.height
                }))
                
        
        };
        // Run on mount
        resizeCanvas();

        
      
        // Re-run on window resize
        window.addEventListener('resize', resizeCanvas);
      
        return () => {
          window.removeEventListener('resize', resizeCanvas);
        };
    }, [selected_candles]);
   
    // ===== Pattern Highlighter
    useEffect(()=>{

         function findIndexByDate(candles, patternDate) {
                return candles.findIndex(obj => {
                    const candleDate = new Date(obj.date);
                    const pivotDate = new Date(patternDate);

                    // Compare just the calendar date (ignores time zones / hours)
                    return candleDate.toDateString() === pivotDate.toDateString();
                }) + 1; // add 1 directly here
                }

        let matchIndex = findIndexByDate(selected_candles, selected_pattern?.pattern_A_start_date);
        // let matchIndex = selected_candles.findIndex(obj => obj.date === selected_pattern?.pattern_A_start_date);
        // matchIndex = matchIndex + 1
        let y = candleChartRef.current.candles.complete_width * matchIndex

        candleChartRef.current.pattern.length = selected_pattern?.pattern_ABCD_bar_length
        candleChartRef.current.pattern.highlighter.x_orgin = -(candleChartRef.current.width.current_X_origin + y)
        candleChartRef.current.pattern.highlighter.previous = -(candleChartRef.current.width.current_X_origin + y)



    },[selected_pattern])
    
    // ===== Canvas Move
    useEffect(() => {
        

        if (!canvas_chart.current) return;
        const { canvas, ctx } = CandleChartTools.reset_candle_canvas(canvas_chart);


        if (!canvas_price.current) return;
        const { cp, ctx_price } = CandleChartTools.reset_price_canvas(canvas_price);
      
        if (!canvas_dates.current) return;
        const canvas_date = canvas_dates.current;
        const ctx_date = canvas_date.getContext('2d');

        if (!ctx_date) return;
        canvas_date.style.width = '100%';
        canvas_date.style.height = '100%';
        canvas_date.width = canvas_date.offsetWidth;
        canvas_date.height = canvas_date.offsetHeight;

        let animationFrameId = null;

        mouse = new draw_.Mouse(candleChartRef);
        chart = new draw_.Chart(candleChartRef)
        abcd_ = new draw_.ABCD(candleChartRef)
         
        // Mouse Events
        const handleMouseMove = (e) => {

            
            candleChartRef.current.mouse.pos.x = e.clientX - canvas.getBoundingClientRect().left;
            candleChartRef.current.mouse.pos.y = e.clientY - canvas.getBoundingClientRect().top;
        
            // Mouse drag handling: update canvas offsets and mid-price while mouse is pressed
            if(candleChartRef.current.mouse.isPressed){   

              
                // Movement (X)
         
                // Calculate how many pixels the mouse moved horizontally while mouse down
                let pixels_mouse_moved_x = candleChartRef.current.mouse.down.x - candleChartRef.current.mouse.pos.x

                // Update X-Origin Based On Pixels Moved
                candleChartRef.current.width.current_X_origin = candleChartRef.current.width.prev_X_origin + pixels_mouse_moved_x


                candleChartRef.current.pattern.highlighter.x_orgin = candleChartRef.current.pattern.highlighter.previous - pixels_mouse_moved_x

                
              
                resize.chart_Y_movement(candleChartRef)
                
    
            
            }   
            if (!animationFrameId) {
                animationFrameId = requestAnimationFrame(draw);
            }
        };
        const handleWheel = (e) => {
            if (animationFrameId) cancelAnimationFrame(animationFrameId);
            animationFrameId = requestAnimationFrame(draw); 
        };
        const handleResize = () => {
            draw(); 
        };
        const handleResize_price = () => {
            draw(); 
        };
        const candle_height_zoom = (e) => {

            let threshold = Math.floor(candleChartRef.current.price.starting_price_unit_pixel_size  * 0.5)
            let expand_threshold = Math.floor(candleChartRef.current.price.starting_price_unit_pixel_size  * 1.5)
        
            
            if (e.deltaY < 0) resize.chart_zoom_in(candleChartRef, expand_threshold);
            if (e.deltaY > 0) resize.chart_zoom_out(candleChartRef, threshold)
            if (!animationFrameId) {
                animationFrameId = requestAnimationFrame(draw);
            }
   
        };
        const candle_width_zoom = (event) => {

            const zoomIn = () => {       


                
                // Increase Grid Width
                candleChartRef.current.x_grid_width += .75 * candleChartRef.current.x_grid_increaser
                
                // Increase Candle Width
                candleChartRef.current.current_candle_width += .50

                // Increase Pixels Between Candles
                candleChartRef.current.current_pixels_between_candles += .25
                
                // Track Candle Complete Width
                candleChartRef.current.candles.complete_width = candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles
                
                // Adjust X-Origin 
                let multiplier = 2 * current_hovered_candle_index.current - 1
                let offset = (0.25 + 0.125) * multiplier
                candleChartRef.current.width.current_X_origin -= offset
                candleChartRef.current.width.prev_X_origin -= offset

                // Adjust Pattern Highlight Position
                let matchIndex = selected_candles.findIndex(obj => obj.date === selected_pattern?.pattern_A_start_date);
                matchIndex = matchIndex + 1
                let diff = current_hovered_candle_index.current - (matchIndex+1)
                let X = 2 * diff
                let z = (0.25 + 0.125) * X
                candleChartRef.current.pattern.highlighter.x_orgin += (z+0.375) 
                candleChartRef.current.pattern.highlighter.previous += (z+0.375) 

                // Handle Displayed Grid Amount
                const zoomLevels = [
                    { threshold: 5, increaser: 20},
                    { threshold: 10, increaser: 10},
                    { threshold: 25, increaser: 5},
                    { threshold: 35, increaser: 3},
                    { threshold: 100, increaser: 1},
                ];            
                for(const level of zoomLevels){
                    if(level.threshold === candleChartRef.current.candles.complete_width){
                        candleChartRef.current.x_grid_increaser = level.increaser
                        candleChartRef.current.x_grid_width = candleChartRef.current.candles.complete_width * level.increaser
                        
                    }
                }
              
            }
            const zoomOut = () => {
                
                if(candleChartRef.current.current_candle_width>0.5){

                    // Increase Grid Width
                    candleChartRef.current.x_grid_width -= .75 * candleChartRef.current.x_grid_increaser
                    candleChartRef.current.current_candle_width -= .50
                    candleChartRef.current.current_pixels_between_candles -= .25
                    candleChartRef.current.candles.complete_width = candleChartRef.current.current_candle_width + candleChartRef.current.current_pixels_between_candles
                    
                    // Adjust X-Origin 
                    let multiplier = 2 * current_hovered_candle_index.current - 1
                    let offset = (0.25 + 0.125) * multiplier
                    candleChartRef.current.width.current_X_origin += offset
                    candleChartRef.current.width.prev_X_origin += offset
        

                    // Adjust Pattern Highlight Position
                    let matchIndex = selected_candles.findIndex(obj => obj.date === selected_pattern?.pattern_A_start_date);
                    matchIndex = matchIndex + 1
                    let diff = current_hovered_candle_index.current - (matchIndex+1)
                    let X = 2 * diff
                    let z = (0.25 + 0.125) * X
                    candleChartRef.current.pattern.highlighter.x_orgin -= (z+0.375) 
                    candleChartRef.current.pattern.highlighter.previous -= (z+0.375) 
       
                // Handle Displayed Grid Amount
                const zoomLevels = [
                     { threshold: 1.75, increaser: 320},
                    { threshold: 5, increaser: 40},
                    { threshold: 10, increaser: 20},
                    { threshold: 25, increaser: 10},
                    { threshold: 35, increaser: 5},
                    { threshold: 100, increaser: 3},
                ];            
                for(const level of zoomLevels){
                    if(level.threshold === candleChartRef.current.candles.complete_width){
                        candleChartRef.current.x_grid_increaser = level.increaser
                        candleChartRef.current.x_grid_width = candleChartRef.current.candles.complete_width * level.increaser
                        
                    }
                }
 }


                
            }

            event.deltaY < 0 && zoomIn()
            event.deltaY > 0 && zoomOut()
     
            if (animationFrameId) cancelAnimationFrame(animationFrameId);
            animationFrameId = requestAnimationFrame(draw); 
        }
        
        const draw = () => {

           
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.beginPath();
            ctx_price.clearRect(0, 0, cp.width, cp.height);
            
            chart.candles(ctx);
            chart.prices(ctx_price, cp);
            // chart.grid_X(ctx, canvas)
            chart.grid_Y(ctx, canvas)
            chart.pattern_center(ctx, canvas)

            ctx_date.clearRect(0, 0, canvas_date.width, canvas_date.height);
            mouse.mouse_Y(canvas, ctx)
            mouse.mouse_X(canvas, ctx, set_candle_high, set_candle_close, set_candle_open, set_candle_low, current_hovered_candle_index, set_candle_color)
            mouse.price_background(canvas, ctx_price)
            mouse.mouse_price(cp, ctx_price)
            mouse.date_background(ctx_date, candle_width, canvas_date)
            mouse.mouse_date(canvas_date, ctx_date)

            is_abcd_pattern && abcd_.abcd(ctx, ab, abcd)
            is_retracement && abcd_.retracement(ctx, ab)
            is_price_levels &&  abcd_.price_levels(ctx_price, ctx, canvas, ab)

        
       
            // CandleChartTools.highlight_selected_pattern(candleChartRef, ctx, canvas)
            // CandleChartTools.display_mid_point(canvas, ctx, ctx_price, candleChartRef)

            animationFrameId = null;
        };
        
        canvas.addEventListener('mousemove', handleMouseMove);

        canvas.addEventListener('mouseup', handleMouseUpPrices);
        canvas.addEventListener('mousedown', handleMouseDownPrices);

        canvas.addEventListener('resize', handleResize);
        canvas.addEventListener('wheel', candle_width_zoom)
        cp.addEventListener('mouseup', handleMouseUpPrices);
        cp.addEventListener('mousedown', handleMouseDownPrices);
        cp.addEventListener('resize', handleResize_price);
        cp.addEventListener('wheel', candle_height_zoom)

        draw();
        

        return () => {
            cancelAnimationFrame(animationFrameId);
    
            canvas.removeEventListener('wheel', handleWheel);
            canvas.removeEventListener('mousemove', handleMouseMove);
            canvas.removeEventListener('mouseup', handleMouseUp);
            canvas.removeEventListener('mousedown', handleMouseDown);
            canvas.removeEventListener('resize', handleResize);
            canvas.removeEventListener('wheel', candle_width_zoom)

            cp.removeEventListener('mouseup', handleMouseUpPrices);
            cp.removeEventListener('mousedown', handleMouseDownPrices);
            cp.removeEventListener('resize', handleResize_price);
            cp.removeEventListener('wheel', candle_height_zoom)
    
        };
    }, [selected_candles, abcd, selected_pattern, ab, is_price_levels, is_retracement, is_abcd_pattern]);

    // ===== Format Pattern
    useEffect(()=>{
        if(selected_abcd){

            const get_formatted_date = (candle) => {
                const date = new Date(candle.date);
                return date.toISOString().split("T")[0];
            }
            
            set_abcd(prev=> ({
                ...prev,
                a: get_formatted_date(selected_abcd['pattern_A_pivot_date']) + 1,
                b: get_formatted_date(selected_abcd['pattern_B_pivot_date'])  + 1,
                c: get_formatted_date(selected_abcd['pattern_C_pivot_date'])  + 1,
                d: get_formatted_date(selected_abcd['trade_entered_date'])  + 1,
                exit_date: get_formatted_date(selected_abcd['trade_exited_date']) + 1,
                x_date: get_formatted_date(selected_abcd['x_pivot_date']) + 1,
                x_price: parseFloat(selected_abcd['x_pivot_price']),
                a_price: parseFloat(selected_abcd['pivot_A_price']),
                b_price: parseFloat(selected_abcd['pivot_B_price']),
                c_price: parseFloat(selected_abcd['pivot_C_price']),
                d_price: parseFloat(selected_abcd['trade_entered_price']),
                stop_loss: parseFloat(selected_abcd['trade_stop_loss']),
                take_profit: parseFloat(selected_abcd['trade_exited_price']),
                entered_price: parseFloat(selected_abcd['trade_entered_price']),
                ab_price_length: parseFloat(selected_abcd['ab_price_length']),
                exit_price: parseFloat(selected_abcd['trade_exited_price']),
                result: selected_abcd['trade_result']
            }))
            

            


        }

        if(selected_pattern){


            function normalizeDate(dateStr) {
                if (/^\d{2}-\d{2}-\d{4}$/.test(dateStr)) {
                    const [mm, dd, yyyy] = dateStr.split("-");
                    return `${yyyy}-${mm}-${dd}`;
                }
                return dateStr; 
            }

            function findIndexByDate(candles, patternDate) {
                const pivotDate = new Date(normalizeDate(patternDate));
                return candles.findIndex(obj => {
                    const candleDate = new Date(normalizeDate(obj.date));
                    return candleDate.toDateString() === pivotDate.toDateString();
                }) + 1;
            }

            let index_A = findIndexByDate(selected_candles, selected_pattern?.pattern_A_pivot_date);
            let index_B = findIndexByDate(selected_candles, selected_pattern?.pattern_B_pivot_date);
            let index_C = findIndexByDate(selected_candles, selected_pattern?.pattern_C_pivot_date);
            let index_D = findIndexByDate(selected_candles, selected_pattern?.trade_entered_date);
            let exit = findIndexByDate(selected_candles, selected_pattern?.trade_exited_date);

            set_ab(prev=> ({
                ...prev,
                a: index_A,
                b: index_B,
                c: index_C,
                d: index_D,
                a_price: parseFloat(selected_pattern.pattern_A_high),
                b_price: parseFloat(selected_pattern.pattern_B_low),
                c_price: parseFloat(selected_pattern.pattern_C_high),
                d_price: parseFloat(selected_pattern.trade_entered_price),
                stop_loss: parseFloat(selected_pattern['trade_stop_loss']),
                take_profit: parseFloat(selected_pattern['trade_take_profit']),
                entered_price: parseFloat(selected_pattern['trade_entered_price']),
                exit_price: parseFloat(selected_pattern['trade_exited_price']),
                exit_date: exit,
            }))

   
            // X
            
            
            // Grid Unit Height Adjuster
            let x = false
            let size = 1
            while(x===false){
                
                let pattern_price_height = (parseFloat(selected_pattern.pattern_A_high) - parseFloat(selected_pattern['trade_stop_loss'])).toFixed(2)
                let pattern_box_height = candleChartRef.current.canvas_height / 2

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
        
          
                
            resize.push_price_to_middle_screen(candleChartRef, selected_pattern)



            let y = false;

            while (y === false) {
                const candles = candleChartRef.current.candles;
                const chart = candleChartRef.current;

                // Always recalculate complete width as the sum of its parts
                candles.complete_width = chart.current_candle_width + chart.current_pixels_between_candles;

                let pattern_length = candles.complete_width * selected_pattern.pattern_ABCD_bar_length;
                let pattern_box_width = (chart.canvas_width / 8) * 6;

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
            
            console.log('candle width:', candleChartRef.current.current_candle_width)
            console.log('pxs between',candleChartRef.current.current_pixels_between_candles)
            console.log('complete width:',candleChartRef.current.candles.complete_width)

            candleChartRef.current.width.current_X_origin = -(candleChartRef.current.width.grid_width/8) - (candleChartRef.current.candles.complete_width * index_A) 
            candleChartRef.current.width.prev_X_origin = -(candleChartRef.current.width.grid_width/8) - (candleChartRef.current.candles.complete_width * index_A) 
            candleChartRef.current.selected_candle = parseFloat(selected_pattern.pattern_A_high)


            
            
            

                                
    

            }



          



   


        }
    },[selected_abcd, selected_pattern])

    const handleMouseDown = () => {
        candleChartRef.current.mouse.isPressed = true;
        candleChartRef.current.mouse.down.y = candleChartRef.current.mouse.pos.y
        candleChartRef.current.mouse.down.x = candleChartRef.current.mouse.pos.x

      
    };
    const handleMouseDownPrices = () => {
        candleChartRef.current.mouse.isPressedOnPrices = true;
        candleChartRef.current.mouse.down.y = candleChartRef.current.mouse.pos.y
        candleChartRef.current.mouse.down.x = candleChartRef.current.mouse.pos.x
    };
    const handleMouseUp = () => {
        candleChartRef.current.mouse.isPressed = false;
        candleChartRef.current.height.previousBaselineY = candleChartRef.current.height.currentBaselineY
        candleChartRef.current.height.prev_Y_OffSet = candleChartRef.current.height.current_Y_OffSet

        candleChartRef.current.width.prev_X_origin = candleChartRef.current.width.current_X_origin
        
        candleChartRef.current.price.prev_mid_price = candleChartRef.current.price.current_mid_price
        candleChartRef.current.price.prev_pixels_per_price_unit = candleChartRef.current.price.current_pixels_per_price_unit


        // ==========
        candleChartRef.current.height.previousBaselineY = candleChartRef.current.height.currentBaselineY

        candleChartRef.current.pattern.highlighter.previous = candleChartRef.current.pattern.highlighter.x_orgin
        

        
    };
    const handleMouseUpPrices = () => {
        candleChartRef.current.mouse.isPressedOnPrices = false;
        candleChartRef.current.height.previousBaselineY = candleChartRef.current.height.currentBaselineY
        candleChartRef.current.price.prev_mid_price = candleChartRef.current.price.current_mid_price
        candleChartRef.current.price.prev_pixels_per_price_unit = candleChartRef.current.price.current_pixels_per_price_unit
   
        for (const candle of candleChartRef.current.candles.candles) {
            candle.prev_high = candle.current_high;
            candle.prev_bottom = candle.current_bottom;
            candle.prev_low = candle.current_low;
            candle.prev_height = candle.current_height;
        }

      
    };



    return(
        <div className='candle_chart_container'>
         
            <div className='candle_chart_wrapper'>
                <div className='canvas'>

                    <div className='header-bar'>
    
                        <div className='header_slot' onClick={()=>{set_is_listing_status(!is_listing_status)}}>{ticker_symbol}</div>
                            <div className='header_slot'>
                                <div className='header_one'>H</div>
                                <div className='header_two' style={{color: candle_color}}>{candle_high}</div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>C</div>
                                <div className='header_two' style={{color: candle_color}}>{candle_close}</div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>O</div>
                                <div className='header_two' style={{color: candle_color}}>{candle_open}</div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>L</div>
                                <div className='header_two' style={{color: candle_color}}>{candle_low}</div>
                            </div>
                    </div>
    
                    <canvas id='canvas' 
                    ref={canvas_chart}
                        onMouseDown={handleMouseDown}
                        onMouseUp={handleMouseUp}
                        // onWheel={(event) => {zoom(event)}}
                    >

                    </canvas>
                </div>
    
                <div className='canvas_dates'>
                    <canvas id='canvasDatesBar' ref={canvas_dates}></canvas>
                </div>
    
            </div>	
                    
            <div className='canvas_prices'>
                <div className='pricesb'>
                    <canvas ref={canvas_price} 
                        onMouseDown={handleMouseDownPrices}
                        onMouseUp={handleMouseUpPrices}></canvas>
                </div>
            </div>
                    
        </div>
    )
}

