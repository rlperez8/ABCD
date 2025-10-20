import { useState } from "react"


const ChartMain = (props) => {

    const {
        abcd,
        prices,
        retracement,
        candles_,
        formatted_candles,
        chart_height,
        Candle_Chart,
        is_listing_status,
        set_is_listing_status,
        ticker_symbol,
        set_canvas_dimensions,
        selected_pattern
    } = props

    const [is_abcd_pattern, set_abcd_pattern] = useState(true)
    const [is_price_levels, set_price_levels] = useState(true)
    const [is_retracement, set_retracement] = useState(true)


    return(
        
            <div className='chart_container'>
              
              <div className='chart_tool_header'>

             
              </div>

              <div className='data_'>

                <div className='chart_button'>
                  <div className={is_abcd_pattern ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                   
                    set_abcd_pattern(!is_abcd_pattern)}}>

                    <img className='abcd_img' src={abcd}></img>
                  </div>

                  <div className={is_price_levels ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                   
                    set_price_levels(!is_price_levels)}}>

                    <img className='abcd_img' src={prices}></img>
                  </div>

                   <div className={is_retracement ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                   
                    set_retracement(!is_retracement)}}>

                    <img className='abcd_img' src={retracement}></img>
                  </div>

                  <div className='chart_icon_'>

                    <img className='abcd_img' src={candles_}></img>
                  </div>
                  
                  
                </div>


                  <Candle_Chart 
                    selected_candles={formatted_candles} 
                    chart_height={chart_height}
                    set_is_listing_status={set_is_listing_status}
                    is_listing_status={is_listing_status}
                    ticker_symbol={ticker_symbol}
                    set_canvas_dimensions={set_canvas_dimensions}
                    selected_pattern={selected_pattern}
                    is_price_levels={is_price_levels}
                    is_retracement={is_retracement}
                    is_abcd_pattern={is_abcd_pattern}
                  />
                  
          
  
       
              </div>


            </div>
    )
}

export default ChartMain