import React, {useState, useRef, useEffect} from "react";
import './new.css'
import * as CandleChartTools from './candle_chart_tools.js';
import * as tools from './tools.js';
import * as docs from './jsdocs.js'
import * as resize from './resize.js'

export const Candle_Chart = (props) => {

    const {
		selected_candles,
        set_is_listing_status,
        is_listing_status,
        ticker_symbol,
        set_canvas_dimensions,
        selected_abcd,
        selected_ab,
        selected_pattern

	} = props
    


    const canvas_dates = useRef()
    const canvas_price = useRef()
    const canvas_chart = useRef()
    const [candle_high, set_candle_high] = useState(0)
    const [candle_close, set_candle_close] = useState(0)
    const [candle_open, set_candle_open] = useState(0)
    const [candle_low, set_candle_low] = useState(0)
    const [candle_color, set_candle_color] = useState('')

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
    const candleChartRef = useRef({

        })

    // ===== Canvas Mount
    useEffect(() => {

        
        // let matchIndex = selected_candles.findIndex(obj => obj.date === selected_pattern?.pattern_A_start_date);
        // matchIndex = matchIndex + 1
     
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
   
                candleChartRef.current.unit_amount = 1
                candleChartRef.current.candles.complete_width = 16
                candleChartRef.current.current_pixels_between_candles = 5
                candleChartRef.current.current_candle_width = 11
                candleChartRef.current.x_grid_increaser = 10
                candleChartRef.current.x_grid_width = 16*10
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
         

        // Mouse Pointer
        const draw_Y_mouse = () => {
            ctx.save()
            let x_mouse_location = candleChartRef.current.mouse.pos.y
            ctx.beginPath();
            ctx.lineWidth = 2
            ctx.strokeStyle = 'white'
            ctx.setLineDash([5, 5]); 
            ctx.moveTo(0,x_mouse_location);    
            ctx.lineTo(canvas.width,x_mouse_location);  

            ctx.stroke();
            ctx.restore();
        };
        const draw_X_mouse = () => {
            ctx.save()
            
          
            // Find Current x-loc
            let mouse_x_loc = candleChartRef.current.mouse.pos.x
            let mouse_x_loc_with_x_spacing = Math.floor(-(mouse_x_loc + candleChartRef.current.width.current_X_origin))
      

            // Track Index
            let index = Math.floor(mouse_x_loc_with_x_spacing/candleChartRef.current.candles.complete_width) + 1
            current_hovered_candle_index.current = index 
         

            let pixelStart = (index * (candleChartRef.current.candles.complete_width)) - (candleChartRef.current.candles.complete_width/2)
         
 
       
            // const reversedCandles = [...candles].reverse();
            const hoveredCandle = candleChartRef.current.candles.candles[index-1];
            set_candle_high(hoveredCandle?.candle_high)
            set_candle_close(hoveredCandle?.candle_close)
            set_candle_open(hoveredCandle?.candle_open)
            set_candle_low( hoveredCandle?.candle_low)
            set_candle_color(hoveredCandle?.candle_open > hoveredCandle?.candle_close ? '#ef5350' : hoveredCandle?.candle_open < hoveredCandle?.candle_close ? '#26a69a' : '')
            
            const date = new Date(hoveredCandle?.date);
            const options = { weekday: 'short', day: '2-digit', month: 'short' };
            const formattedDate = date.toLocaleDateString('en-GB', options);

           
            // // Extract the last two digits of the year
            const shortYear = `'${date.getFullYear().toString().slice(-2)}`;

            let d = `${formattedDate} ${shortYear}`
     
            ctx.beginPath(); 
            ctx.lineWidth = 2
            ctx.strokeStyle = 'white'
            ctx.setLineDash([5, 5]); 
            ctx.moveTo(-pixelStart - candleChartRef.current.width.current_X_origin, canvas.height);    
            ctx.lineTo(-pixelStart - candleChartRef.current.width.current_X_origin, 0); 

            // ctx.moveTo(-candleChartRef.current.width.current_X_origin, canvas.height);    
            // ctx.lineTo(-candleChartRef.current.width.current_X_origin, 0); 

            // let candle_size = candleChartRef.current.candles.complete_width * 4
            // let x_loc = -(candleChartRef.current.width.current_X_origin + candle_size)

            // ctx.moveTo(-candleChartRef.current.width.current_X_origin - candleChartRef.current.candles.complete_width, canvas.height);    
            // ctx.lineTo(-candleChartRef.current.width.current_X_origin - candleChartRef.current.candles.complete_width, 0); 

            // ctx.moveTo(x_loc  , canvas.height);    
            // ctx.lineTo(x_loc  , 0); 
            
            // ctx.moveTo(-candleChartRef.current.width.current_X_origin - candleChartRef.current.candles.complete_width * 5 , canvas.height);    
            // ctx.lineTo(-candleChartRef.current.width.current_X_origin - candleChartRef.current.candles.complete_width * 5, 0); 

       
            // ctx.moveTo(-candleChartRef.current.width.current_X_origin, canvas.height);    
            // ctx.lineTo(-candleChartRef.current.width.current_X_origin, 0); 
            // ctx.moveTo(-candleChartRef.current.width.current_X_origin + (candle_width.current + 2.5 + 2.5), canvas.height);    
            // ctx.lineTo(-candleChartRef.current.width.current_X_origin +(candle_width.current + 2.5 + 2.5), 0); 

            ctx.stroke();
            ctx.restore();
        }
    
        

        // Date
        const draw_X_date_tag = () => {
            ctx_date.save()
            

            // Find Current x-loc
            let mouse_x_loc = candleChartRef.current.mouse.pos.x
            let mouse_x_loc_with_x_spacing = -(mouse_x_loc + candleChartRef.current.width.current_X_origin)

            // Track Candle Width
            // let full_candle_width = candle_width.current + 5
            // let full_candle_width = candleChartRef.current.current_candle_width + 5
            let full_candle_width = candleChartRef.current.candles.complete_width

            // Track X-Spacing
            let spacing_in_candles = candleChartRef.current.width.current_X_origin / full_candle_width

            // Track Index
            let index = Math.floor(mouse_x_loc_with_x_spacing/full_candle_width)
   
           
            mouse_x_loc = (mouse_x_loc - candleChartRef.current.width.current_X_origin);
            mouse_x_loc = mouse_x_loc - (candle_width.current / 2);
 
          
        
            let pixelStart = (index * full_candle_width) + (full_candle_width/2)
            pixelStart = pixelStart - 2.5
        
            let x_rect = candleChartRef.current.width.current_X_origin + (pixelStart - 75) - candle_width.current;
            let y_rect = 0;
            let width_rect = 150;
            let height_rect = 40;
       
            if(index>=0){
                ctx_date.beginPath();
                // ctx_date.fillStyle = "teal";
                ctx_date.fillStyle = "#151c20e0";
                ctx_date.fillRect( (-pixelStart - candleChartRef.current.width.current_X_origin)-80, y_rect, width_rect, canvas_date.height);
                ctx_date.stroke();
                ctx_date.restore()
            }
            
            
        }
        const draw_date_text = () => {
            let mouse_x_loc = candleChartRef.current.mouse.pos.x;
            let mouse_x_loc_with_x_spacing = -(mouse_x_loc + candleChartRef.current.width.current_X_origin);

            // Track Candle Width
            let full_candle_width = candleChartRef.current.candles.complete_width;

            // Track Index
            let index = Math.floor(mouse_x_loc_with_x_spacing / full_candle_width);

            let pixelStart = (index * full_candle_width) + (full_candle_width / 2);
            pixelStart = pixelStart - 2.5;

            // Get Hovered Candle Date
            const hoveredCandle = candleChartRef.current.candles.candles[index]?.date;

            // ✅ Just strip time, keep full original string
            const finalFormat = hoveredCandle?.split(" 00:")[0] || "";

            const metrics = ctx_date.measureText(finalFormat);
            const textHeight = metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent;

            let y_text = 30;
            if (index >= 0) {
                ctx_date.beginPath();
                ctx_date.font = "16px Arial";
                ctx_date.fillStyle = "gray";
                ctx_date.fillText(
                    finalFormat,
                    (-pixelStart - candleChartRef.current.width.current_X_origin) - 65,
                    (canvas_date.height / 2) + textHeight / 2
                );
                ctx_date.stroke();
            }
        };

        const draw_x_grid_date = () => {
            // REMOVE DRAW FOR THIS FUNCTION WHEN JUST MOUSE MOVES
            let startingX = -(candleChartRef.current.current_candle_width / 2);
            let candle_index = 0;

            candleChartRef.current.candles.candles.forEach(item => {
                const x = Math.floor(startingX - candleChartRef.current.width.current_X_origin);

                // ✅ keep the full date string, but cut off the time part
                const date = candleChartRef.current.candles.candles[candle_index]?.date.split(" 00:")[0];

                const metrics = ctx_date.measureText(date);
                const textHeight = metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent;

                ctx.save();
                ctx.beginPath();
                ctx_date.fillText(date, x - 25, (canvas_date.height / 2) + textHeight / 2);
                ctx.stroke();
                ctx.restore();

                startingX -= candleChartRef.current.x_grid_width;
                let current_index = -Math.trunc(startingX / candleChartRef.current.candles.complete_width);
                candle_index = current_index;
            });
        };

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
        
            
            // if (e.deltaY < 0) resize.zoomIn(candleChartRef, expand_threshold);
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
            animationFrameId = null;
            if (animationFrameId) cancelAnimationFrame(animationFrameId);
            animationFrameId = requestAnimationFrame(draw); 
        }
        
        const draw = () => {

           
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.beginPath();

            ctx_price.clearRect(0, 0, cp.width, cp.height);
            
         
            
            const draw_retracement = () => {
                
                ctx.save()
                // --- A Coordinates ---
                let a_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.a_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

                let a_x_loc = -candleChartRef.current.candles.complete_width * ab.a;
                a_x_loc -= candleChartRef.current.width.current_X_origin;
                a_x_loc += candleChartRef.current.candles.complete_width / 2;

                // --- B Coordinates ---
                let b_y_loc = candleChartRef.current.height.currentBaselineY - (ab.b_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

                let b_x_loc = -candleChartRef.current.candles.complete_width * ab.b;
                b_x_loc -= candleChartRef.current.width.current_X_origin;
                b_x_loc += candleChartRef.current.candles.complete_width / 2;

                // --- C Coordinates ---
                let c_y_loc = candleChartRef.current.height.currentBaselineY - (ab.c_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

                let c_x_loc = -candleChartRef.current.candles.complete_width * ab.c;
                c_x_loc -= candleChartRef.current.width.current_X_origin;
                c_x_loc += candleChartRef.current.candles.complete_width / 2;

                // --- D Coordinates ---
                let d_y_loc = candleChartRef.current.height.currentBaselineY - (ab.d_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

                let d_x_loc = -candleChartRef.current.candles.complete_width * ab.d;
                d_x_loc -= candleChartRef.current.width.current_X_origin;
                d_x_loc += candleChartRef.current.candles.complete_width / 2;

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
            const draw_price_levels = () => {
                ctx.save();

                // // Calculate all y positions
                const y_stop_loss = candleChartRef.current.height.currentBaselineY - (ab.stop_loss  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                const y_take_profit = candleChartRef.current.height.currentBaselineY - (ab.take_profit  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                const y_entered_price = candleChartRef.current.height.currentBaselineY - (ab.entered_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

                // Calculate horizontal start and end positions
                let x_start = -candleChartRef.current.candles.complete_width * ab.d;
                x_start -= candleChartRef.current.width.current_X_origin;
                x_start += candleChartRef.current.candles.complete_width / 2;

                let x_end = -candleChartRef.current.candles.complete_width * ab.exit_date;
                x_end -= candleChartRef.current.width.current_X_origin;
                x_end += candleChartRef.current.candles.complete_width / 2;
                

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
                let starting_ = -candleChartRef.current.candles.complete_width * ab.exit_date;
                starting_ -= candleChartRef.current.width.current_X_origin;
                starting_ += candleChartRef.current.candles.complete_width / 2;
                let end_ = candleChartRef.current.width.grid_width
                ctx.beginPath();
                ctx.strokeStyle = 'white';
                ctx.lineWidth = 1;
                ctx.setLineDash([5, 5]);   
                ctx.moveTo(starting_, y_entered_price);
                ctx.lineTo(end_, y_entered_price);
                ctx.stroke();
                ctx.setLineDash([]);

                let profit_starting_ = -candleChartRef.current.candles.complete_width * ab.exit_date;
                profit_starting_ -= candleChartRef.current.width.current_X_origin;
                profit_starting_ += candleChartRef.current.candles.complete_width / 2;
                let profit_end_ = candleChartRef.current.width.grid_width
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

            const draw_ABCD_lines = () => {
                
                
        
                // A
                let a_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.a_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                let a_x_loc = -candleChartRef.current.candles.complete_width * ab.a;
                a_x_loc -= candleChartRef.current.width.current_X_origin;
                a_x_loc += candleChartRef.current.candles.complete_width / 2;
              
                // B
                let b_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.b_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                let b_x_loc = -candleChartRef.current.candles.complete_width * ab.b;
                b_x_loc -= candleChartRef.current.width.current_X_origin;
                b_x_loc += candleChartRef.current.candles.complete_width / 2;

                // C
                let c_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.c_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                let c_x_loc = -candleChartRef.current.candles.complete_width * ab.c;
                c_x_loc -= candleChartRef.current.width.current_X_origin;
                c_x_loc += candleChartRef.current.candles.complete_width / 2;

                // D
                let d_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.d_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))
                let d_x_loc = -candleChartRef.current.candles.complete_width * ab.d;
                d_x_loc -= candleChartRef.current.width.current_X_origin;
                d_x_loc += candleChartRef.current.candles.complete_width / 2;

                // Exit
                let exit_y_loc =  candleChartRef.current.height.currentBaselineY - (ab.exit_price  * (candleChartRef.current.price.current_pixels_per_price_unit / candleChartRef.current.unit_amount))

           
                let exit_x_loc = -candleChartRef.current.candles.complete_width * ab.exit_date;
                exit_x_loc -= candleChartRef.current.width.current_X_origin;
                exit_x_loc += candleChartRef.current.candles.complete_width / 2;
              
              
              
                ctx.beginPath();
                ctx.lineTo(a_x_loc, a_y_loc);
                ctx.lineTo(b_x_loc, b_y_loc);
                ctx.lineTo(c_x_loc, c_y_loc);
                ctx.lineTo(d_x_loc, d_y_loc);
                ctx.lineTo(exit_x_loc, exit_y_loc);
                ctx.strokeStyle = 'gray';
                ctx.lineWidth = 5;
                ctx.stroke();

                   // --- Optional stroke on top ---
                ctx.strokeStyle = 'orange';
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
                ctx.strokeStyle = abcd.result === 'Win' ? 'teal' : 'orange';
                ctx.lineWidth = 3;
                ctx.shadowColor = abcd.result === 'Win' ? 'teal' : 'orange'; // match line color
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
      
            console.log('unit_amount',candleChartRef.current.unit_amount)
  
            
            CandleChartTools.drawGrid(candleChartRef, ctx, canvas);
            CandleChartTools.draw_x_grid(candleChartRef, ctx, canvas)
            docs.draw_candles(candleChartRef, ctx);
            CandleChartTools.drawWicks(candleChartRef, ctx);
            CandleChartTools.drawPrices(candleChartRef, ctx_price, cp);
            // CandleChartTools.highlight_selected_pattern(candleChartRef, ctx, canvas)
            // CandleChartTools.display_mid_point(canvas, ctx, ctx_price, candleChartRef)
            draw_Y_mouse()
            CandleChartTools.draw_Y_price_tag(candleChartRef, canvas, ctx_price)
            draw_X_mouse()
            docs.draw_mouse_price(candleChartRef, ctx_price)
            ctx_date.clearRect(0, 0, canvas_date.width, canvas_date.height);
            // draw_x_grid_date();
            draw_X_date_tag()
            draw_date_text()
            // draw_start_AB()
            draw_retracement()
            draw_price_levels()
            draw_ABCD_lines()
            // draw_StopLoss_Enter_TakeProfit()
            // draw_SL_EN_TP_prices()
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
            cancelAnimationFrame(animationFrameId);canvas.removeEventListener('wheel', handleWheel);
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
    }, [selected_candles, abcd, selected_pattern, ab]);

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
            candleChartRef.current.width.current_X_origin = -(candleChartRef.current.width.grid_width/2) - (candleChartRef.current.candles.complete_width * index_A) 
            candleChartRef.current.width.prev_X_origin = -(candleChartRef.current.width.grid_width/2) - (candleChartRef.current.candles.complete_width * index_A) 


   

    

            resize.push_price_to_middle_screen(candleChartRef, selected_pattern)

            candleChartRef.current.selected_candle = parseFloat(selected_pattern.pattern_A_high)
        
      
  


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

                    {/* <div className='header-bar'>
    
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
     */}
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

