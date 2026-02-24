import { useState } from "react"
import { CandleChart } from './candle_chart';

import * as route from '../backend_routes.js';

const ChartMain = (props) => {

    const {
        chart_data,
        is_loading_patterns,
        all_watchlists,
        is_sections_expanded,
        set_sections_expanded,
        market,
        set_selected_xabcd,
        is_selected_xabcd,
        set_is_selected_xabcd
        
    } = props 
  
   
    const [is_abcd_pattern, set_abcd_pattern] = useState(true)
    const [is_price_levels, set_price_levels] = useState(true)
    const [is_retracement, set_retracement] = useState(true)
    const [is_add_wl, set_add_wl] = useState(false)
   
    const bc_pct = (chart_data.abcd_pattern?.pattern_BC_bar_length / chart_data.abcd_pattern?.pattern_AB_bar_length) * 100
    const cd_pct = (chart_data.abcd_pattern?.pattern_CD_bar_length / chart_data.abcd_pattern?.pattern_AB_bar_length) * 100


    return(

      <div className={is_selected_xabcd ? "" : "charts_container"} onClick={()=>{set_selected_xabcd(chart_data); set_is_selected_xabcd(!is_selected_xabcd)}}>

        <div className="margin-">
          
          <div className='chart_container'>
            
      

            <div className='data_'>

              {/* <div className='chart-header-wrapper'>

                <div className="header-buttons-wrapper">

                  <div className={is_abcd_pattern ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                    
                    set_abcd_pattern(!is_abcd_pattern)}}>

                    <img className='abcd_img' src="/images/dropdown.png"></img>
                  </div>

                  <div className={is_price_levels ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                    
                    set_price_levels(!is_price_levels)}}>

                    <img className='abcd_img' src="/images/prices.png"></img>
                  </div>

                  <div className={is_retracement ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                    
                    set_retracement(!is_retracement)}}>

                    <img className='abcd_img' src="/images/retracement.png"></img>
                  </div>

                  <div className='chart_icon_' onClick={()=>{set_add_wl(!is_add_wl)}}>

                    <img className='abcd_img' src="/images/add.png"></img>

                    {is_add_wl && <div className="wl-options-wrapper">

                      {all_watchlists.map(item=>{
                        return(
                        <div className="wl-row" onClick={()=>{

                          route.add_pattern_to_a_watchlist(
                            item.wl_name, 
                            chart_data.abcd_pattern.id,            
                          )
                          
                        }}>{item.wl_name}</div>
                        )
                      })}
                    </div>
      }
                    
                    
                  </div>

                  <div className={false ? 'chart_icon_active' : 'chart_icon_'} onClick={()=> {
                    
                    set_sections_expanded(!is_sections_expanded)
                    
                    }}>

                    <img className='abcd_img' src="/images/abcd.png"></img>
                  </div>
                </div>


              </div> */}

          
                {/* {is_loading_patterns &&
                  <div className="overlay">
                            <div className='loading_container'>Loading...</div>
                  </div>
                } */}

                
                {chart_data.candles.length > 0 && 

                  <CandleChart 
                    chart_data={chart_data}
                    is_price_levels={is_price_levels}
                    is_retracement={is_retracement}
                    is_abcd_pattern={is_abcd_pattern}
                    market={market}
                    is_sections_expanded={is_sections_expanded}
                  />
                  
    }

    
        

      
            </div>
            
          </div>

        </div>        
        
      </div>
    )
}

export default ChartMain