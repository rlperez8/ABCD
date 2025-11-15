import { useState } from "react"
import { Candle_Chart } from './candle_chart';
import abcd from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/abcd.png';
import prices from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/prices.png';
import retracement from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/retracement.png';
import add from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/add.png';
import menu from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/menu.png';
import * as route from '../backend_routes.js';

const ChartMain = (props) => {

    const {
        chart_data,
        is_loading_patterns,
        all_watchlists,
        is_sections_expanded,
        set_sections_expanded
    } = props 

    console.log(is_loading_patterns)



    const [is_abcd_pattern, set_abcd_pattern] = useState(true)
    const [is_price_levels, set_price_levels] = useState(true)
    const [is_retracement, set_retracement] = useState(true)
    const [is_add_wl, set_add_wl] = useState(false)
   
    const bc_pct = (chart_data.abcd_pattern?.pattern_BC_bar_length / chart_data.abcd_pattern?.pattern_AB_bar_length) * 100
    const cd_pct = (chart_data.abcd_pattern?.pattern_CD_bar_length / chart_data.abcd_pattern?.pattern_AB_bar_length) * 100


    return(

      <div className="chart_margin_container">
        <div className="margin-">
          
          <div className='chart_container'>
            
      

            <div className='data_'>

              <div className='chart-header-wrapper'>

                <div className="header-buttons-wrapper">

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

                  <div className='chart_icon_' onClick={()=>{set_add_wl(!is_add_wl)}}>

                    <img className='abcd_img' src={add}></img>

                    {is_add_wl && <div className="wl-options-wrapper">

                      {all_watchlists.map(item=>{
                        return(
                        <div className="wl-row" onClick={()=>{
                          
                        
                          // const closeNew = chart_data.candles[0].candle_close;
                          // const closeOld = chart_data.candles[1].candle_close;
                          // const pctChange = ((closeNew - closeOld) / closeOld) * 100;
                          // const change = closeNew - closeOld
                          // const volume = chart_data.candles[1].volume

                      
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

                    <img className='abcd_img' src={menu}></img>
                  </div>
                </div>

                <div className='chart_tool_header'>

                    <div className="chart-header-stat-wrapper">
                      <div className="stat-key">C Retracement</div>
                      <div className="stat-value">{chart_data.rust_patterns?.trade_bc_price_retracement.toFixed(2)}</div>
                    </div>
                    
                    <div className="chart-header-stat-wrapper">
                      <div className="stat-key">D Retracement</div>
                              <div className="stat-value">{chart_data.rust_patterns?.trade_cd_price_retracement.toFixed(2)}</div>
                    </div>

                    <div className="chart-header-stat-wrapper">
                      <div className="stat-key">BC Length</div>
                      <div className="stat-value">{parseFloat(bc_pct.toFixed(2))}%</div>
                    </div>

                    <div className="chart-header-stat-wrapper">
                      <div className="stat-key">CD Length</div>
                      <div className="stat-value">{parseFloat(cd_pct.toFixed(2))}%</div>
                    </div>

                </div>

              </div>

          
                {/* {is_loading_patterns &&
                  <div className="overlay">
                            <div className='loading_container'>Loading...</div>
                  </div>
                } */}

                
                {chart_data.candles.length > 0 && 

                  <Candle_Chart 
                  chart_data={chart_data}
                  is_price_levels={is_price_levels}
                  is_retracement={is_retracement}
                  is_abcd_pattern={is_abcd_pattern}
          
                />
    }
        

      
            </div>
            
          </div>

        </div>        
        
      </div>
    )
}

export default ChartMain