import React, {useState, useRef, useEffect} from "react";
import '../new.css'
import * as CandleChartTools from '../candle_chart_tools.js';
import * as draw_ from './draw_.js'
import * as resize from './resize.js'
import * as utilites from './utilities.js'
export const Candle_Chart = (props) => {

    const {
		chart_data,
        is_price_levels,
        is_retracement,
        is_abcd_pattern,
        market,
        is_sections_expanded

	} = props
    
    
    const canvas_dates = useRef()
    const canvas_price = useRef()
    const canvas_chart = useRef()
    const [hovered_candle, set_hovered_candle] = useState({
        high: 0,
        close: 0,
        open: 0,
        low: 0,
        color: 'white'
    })

    let mouse = null
    let chart = null
    let abcd_ = null

    const current_hovered_candle_index = useRef(0)
    const candle_width = useRef(10)

    let [pattern_abcd, set_pattern_abcd] = useState({

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

                        candles: chart_data.candles,
                        starting_candle_Y: chart_data.candles[0]?.open * (canvas.height / PRICE_UNIT_DIVISOR),
                      
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
                candleChartRef.current.candles.candles = chart_data.candles
                candleChartRef.current.candles.starting_candle_Y = chart_data.candles[0]?.candle_open * (canvas.height  / 10)   
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
                candleChartRef.current.price.current_mid_price = chart_data.candles[0]?.candle_open 
                candleChartRef.current.price.prev_mid_price = chart_data.candles[0]?.candle_open
                candleChartRef.current.price.static_mid = chart_data.candles[0]?.candle_open 
                
        
                // ====== X-Origin
                // candleChartRef.current.width.current_X_origin = -(canvas.width/2) 
                // candleChartRef.current.width.prev_X_origin = -(canvas.width/2)
                
        
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

                candleChartRef.current.selected_candle = chart_data.candles[0]?.candle_open 
                resize.reposition_candles(candleChartRef, chart_data.rust_patterns)
                
               
        
        };
      
        
        if (!chart_data.candles || !chart_data.rust_patterns) return;
        resizeCanvas()
  
        // Re-run on window resize
        window.addEventListener('resize', resizeCanvas);
      
        return () => {
          window.removeEventListener('resize', resizeCanvas);
        };
    }, [chart_data, is_sections_expanded]);
   
    // ===== Pattern Highlighter
    useEffect(()=>{


        if (!chart_data.candles || !chart_data.rust_patterns) return;

        function findIndexByDate(candles, patternDate) {
                return candles.findIndex(obj => {
                    const candleDate = new Date(obj.date);
                    const pivotDate = new Date(patternDate);

                    // Compare just the calendar date (ignores time zones / hours)
                    return candleDate.toDateString() === pivotDate.toDateString();
                }) + 1; // add 1 directly here
                }

        let matchIndex = findIndexByDate(chart_data.candles, chart_data.rust_patterns?.a_date);
        // let matchIndex = chart_data.candles.findIndex(obj => obj.date === chart_data.abcd_pattern?.pattern_A_start_date);
        // matchIndex = matchIndex + 1
        let y = candleChartRef.current.candles.complete_width * matchIndex

        candleChartRef.current.pattern.length = chart_data.rust_patterns?.pattern_ABCD_bar_length
        candleChartRef.current.pattern.highlighter.x_orgin = -(candleChartRef.current.width.current_X_origin + y)
        candleChartRef.current.pattern.highlighter.previous = -(candleChartRef.current.width.current_X_origin + y)



    },[chart_data])
    
    // ===== Canvas Move
    useEffect(() => {

        if (!chart_data.candles || !chart_data?.rust_patterns) return;

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
                let matchIndex = chart_data.candles.findIndex(obj => obj.date === chart_data.abcd_pattern?.pattern_A_start_date);
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
                    let matchIndex = chart_data.candles.findIndex(obj => obj.date === chart_data.abcd_pattern?.pattern_A_start_date);
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
            chart.grid_X(ctx, canvas)
            chart.grid_Y(ctx, canvas)
            // chart.pattern_center(ctx, canvas)

            ctx_date.clearRect(0, 0, canvas_date.width, canvas_date.height);
            mouse.mouse_Y(canvas, ctx)
            mouse.mouse_X(canvas, ctx, current_hovered_candle_index, set_hovered_candle)
            mouse.price_background(canvas, ctx_price)
            mouse.mouse_price(cp, ctx_price)
            mouse.date_background(ctx_date, candle_width, canvas_date)
            mouse.mouse_date(canvas_date, ctx_date)

            if (market === "Bullish") {
                is_abcd_pattern && abcd_.bull_abcd(ctx, chart_data.rust_patterns)
            }
            else if (market === "Bearish") {
                is_abcd_pattern && abcd_.abcd(ctx, chart_data.rust_patterns)
            }
            
            is_retracement && abcd_.retracement(ctx, chart_data.rust_patterns)
            abcd_.price_levels(ctx_price, ctx, canvas, chart_data.rust_patterns)


          const snr_lines = (canvas, ctx, chart_data) => {

                const y_stop_loss = candleChartRef.current.height.currentBaselineY - (chart_data.rust_patterns.trade_snr * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                ctx.strokeStyle = 'orange';
                ctx.setLineDash([5, 5]); 

                ctx.moveTo(0, y_stop_loss);              
                ctx.lineTo(canvas.width, y_stop_loss); 


                ctx.stroke();
                ctx.setLineDash([]); // reset dash style if needed
            }
            snr_lines(canvas, ctx,  chart_data);

        
       
            // CandleChartTools.highlight_chart_data.abcd_pattern(candleChartRef, ctx, canvas)
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



        window.addEventListener('resize', () => {
            handleResize();
            handleResize_price();
        });

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
    }, [chart_data, pattern_abcd, is_price_levels, is_retracement, is_abcd_pattern, is_sections_expanded]);

    // // ===== Format Pattern
    useEffect(()=>{

        if(chart_data.rust_patterns){
            // resize.reposition_candles(candleChartRef, chart_data.abcd_pattern, chart_data.rust_patterns)
            resize.reposition_candles(candleChartRef, chart_data.rust_patterns)
        }
    },[chart_data])




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
    
                        <div className='header_slot' >{chart_data?.rust_patterns.symbol}</div>
                            <div className='header_slot'>
                                <div className='header_one'>H</div>
                                <div className='header_two' style={{color: hovered_candle.color}}>
                                    {hovered_candle.high}
                                </div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>C</div>
                               <div className='header_two' style={{color: hovered_candle.color}}>
                                    {hovered_candle.close}
                                </div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>O</div>
                                <div className='header_two' style={{color: hovered_candle.color}}>
                                    {hovered_candle.open}
                                </div>
                            </div>
                            <div className='header_slot'>
                                <div className='header_one'>L</div>
                                <div className='header_two' style={{color: hovered_candle.color}}>
                                    {hovered_candle.low}
                                </div>
                            </div>

                            <div className='header_slot'>
                                <div className='header_one'>L</div>
                                <div className='header_two' style={{color: hovered_candle.color}}>
                                    {hovered_candle.volume}
                                </div>
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

