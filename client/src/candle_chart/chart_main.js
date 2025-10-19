

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


    return(
        
            <div className='chart_container'>
              
              <div className='chart_tool_header'>

             
              </div>

              <div className='data_'>

                <div className='chart_button'>
                  <div className='but'>

                    <img className='abcd_img' src={abcd}></img>
                  </div>

                  <div className='but'>

                    <img className='abcd_img' src={prices}></img>
                  </div>

                  <div className='but'>

                    <img className='abcd_img' src={retracement}></img>
                  </div>

                  <div className='but'>

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
                    // selected_abcd={selected_abcd}  
                    // selected_ab={selected_ab}
                    selected_pattern={selected_pattern}
                  />
                  
          
  
       
              </div>


            </div>
    )
}

export default ChartMain