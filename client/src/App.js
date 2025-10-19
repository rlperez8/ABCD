import React, { useState, useEffect, useRef } from 'react';
import { Candle_Chart } from './candle_chart/candle_chart';
import './new.css'
import search from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/search.png';
import abcd from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/abcd.png';
import prices from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/prices.png';
import retracement from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/retracement.png';
import candles_ from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/candles.png';
import Table from './table';
import Filter from './filter';
import ChartMain from './candle_chart/chart_main';
import * as route from './backend_routes.js';
import PerformanceTable from './PeformanceTable.js';


const App = () => {

  const chart_container = useRef(null)
  const [selected_peformance, set_selected_peformance] = useState('AAON')
  const [ticker_performance, set_ticker_peformance] = useState([])
  const [ab_candles, set_ab_candles] = useState([])
  const [abc_patterns, set_abc_patterns] = useState([])
  const [abcd_patterns, set_abcd_patterns] = useState([])
  const [selected_pattern, set_selected_pattern] = useState()
  const [candles, set_candles] = useState([])
  let [chart_height, set_chart_height] = useState(0)
  const [formatted_candles, set_formatted_candles] = useState([])
  const [listing_status, set_listing_status] = useState([])
  const [is_listing_status, set_is_listing_status] = useState(false)
  const [ticker_symbol, set_ticker_symbol] = useState('AAON')
  const [searchValue, setSearchValue] = useState('');
  const asset_types = ['All','Stock','ETF','Crypto']
  const [asset_type, set_asset_type] = useState('All')
  const [current_abcds, set_current_abcds] = useState([])
  const [canvas_dimesions, set_canvas_dimensions] = useState({
    chart_height: 0,
    price_height: 0,
    date_height: 0
  })
  const [abcd_stage, set_abcd_stage] = useState(['All','Won','Lost','Live','Failed'])
  const [selected_abcd_stage, set_selected_abcd_stage] = useState('All')
  const [statistics, set_statistics] = useState({})
  const get_listed_tickers = async (searched_ticker, selected_type) => {
    try {
        const res = await fetch('http://localhost:8000/get_listed_tickers', {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          searched: searched_ticker,
          type: selected_type
        })
      });
      if (!res.ok) {
        console.error(`Server Error: ${res.status} - ${res.statusText}`);
        throw new Error("Request failed");
      }
      const responseData = await res.json();

      set_listing_status(responseData.data)
     
    } catch (error) {
      console.error("Error during fetch:", error);
  }}
  const [selected_pattern_index, set_selected_pattern_index] = useState(1)
  const [table, set_table] = useState([])

  const get_abcd_of_selected_symbol = async (symbol) => {
    try {
        const res = await fetch('http://localhost:8000/get_abcd_of_selected_symbol', {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          symbol: symbol,
        })
      });
      if (!res.ok) {
        console.error(`Server Error: ${res.status} - ${res.statusText}`);
        throw new Error("Request failed");
      }
      const responseData = await res.json();
  
      // set_current_abcds(responseData.data)
      
    } catch (error) {
      console.error("Error during fetch:", error);
  }}
  const [selected_table_option, set_selected_table_option] = useState("A")

  const [filters, set_filters] = useState({
    bc_retracement_greater: 0,
    bc_retracement_less: 5000,
    cd_retracement_greater: 0,
    cd_retracement_less: 5000,
    ab_leg_greater: 0,
    ab_leg_less: 5000,
    bc_leg_greater: 0,
    bc_leg_less: 5000,
    cd_leg_greater: 0,
    cd_leg_less: 5000,

  })
  const [data, set_data] = useState()

  // GET
 

  // const get_ticker_peformances
  // Define Chart Height
  useEffect(() => {
    if (chart_container.current) {
     
      set_chart_height(chart_container.current.clientHeight-1)

    }
  }, []);
  // Format Candles To Canvas Dimensions
  useEffect(()=>{

      let new_candles = candles.reverse().map((item)=> ({
          ...item,
          date: item.candle_date,
          high: item.candle_high,
          open: Math.min(item.candle_open, item.candle_close),
          close: Math.max(item.candle_open, item.candle_close),
          low: item.candle_low,
          prev_high : 0,
          prev_height : 0,
          prev_bottom : 0,
          prev_low : 0,
          current_high : 0,
          current_height : 0,
          current_bottom : 0,
          current_low : 0,
          color: item.candle_open > item.candle_close ? '#ef5350' : '#26a69a',

      }))

      set_formatted_candles(new_candles)

  }, [candles])
   
  useEffect(() => {
    const handleKeyDown = (event) => {
    
      if (event.key === "ArrowRight") {
        
        if(selected_pattern_index < abcd_patterns.length-1){
          set_selected_pattern_index((prev) => {
          const newIndex = prev + 1;
          set_selected_pattern(abcd_patterns[newIndex]);
          return newIndex;
        });
        }
      }

      if (event.key === "ArrowLeft") {
        if(selected_pattern_index > 0){
        set_selected_pattern_index((prev) => {
          const newIndex = Math.max(0, prev - 1);
          set_selected_pattern(abcd_patterns[newIndex]);
          return newIndex;
        });
      }}
      };
      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
  }, [ab_candles, selected_pattern_index]);
  


 


  // First Mount
  useEffect(()=>{

    const fetchCandles = async () => {
      try {
        // Get Candles
        const candles = await route.get_candles(ticker_symbol);
        set_candles(candles)

        // Get Peformances
        const filter = {
          'bc_retracement_greater': '0', 
          'bc_retracement_less': 5000, 
          'cd_retracement_greater': 0, 
          'cd_retracement_less': 5000, 
          'ab_leg_greater': 0, 
          'ab_leg_less': 5000, 
          'bc_leg_greater': 0, 
          'bc_leg_less': 5000, 
          'cd_leg_greater': 0, 
          'cd_leg_less': 5000
        }
        const peformances = await route.fetch_filtered(filter)
        set_ticker_peformance(peformances)

        // Get ABCD Patterns
        const abcd_patterns = await route.get_abcd_candles('AAON')
        set_abcd_patterns(abcd_patterns)
        set_table(abcd_patterns)
      } catch (err) {
        console.error('Error fetching candles:', err);
      }
    };
    fetchCandles();

    // // get_listed_tickers('',asset_type)
    // // get_abcd_of_selected_symbol(ticker_symbol)
    // get_ab_candles()
    // get_abc_candles()
    // 
  
  }, [ticker_symbol]) 

  return (

      <div className='App' >

        {true && <>
  
          {
            is_listing_status && (
              <div className='overlay'>
                <div className='ticker_selection_container'>
                  <div className='select_ticker_header'>
                    <div className='select_ticker_text'>Select Ticker</div>
                    <div className='X' onClick={()=>{set_is_listing_status(false)}}>X</div>
                  </div>
                  
                  <div className='input_container'>
              
                  <div className='search_img_container'>
                    <img className='search_img' src={search} />
                    
                  </div>
              
                  <input className='input_box' 
                    defaultValue={searchValue}
                    onChange={(e)=>{
                      
                      get_listed_tickers(e.target.value, asset_type)
                      setSearchValue(e.target.value)

                  }}
                  
                  />
                  <div className='search_img_container'>
                    
                  </div>
                    
              
              </div>

            <div className='select_type'>
                {asset_types.map((type, index) => (
                  <div className={asset_type===type ? 'type_selected' :'type'} key={index} 
                  onClick={()=>{
                    set_asset_type(type);
                    get_listed_tickers(searchValue,type);
                    
                  }}
                  >{type}</div>
                ))}
                            
          
            </div>

      
            <div className='select_ticker'>
                <div className='symbol_status1'>Ticker</div>
                <div className='symbol_status2'>Security</div>
                <div className='symbol_status3'>Exchange</div>
                <div className='symbol_status3'>Type</div>
            </div>
            <div className='ticker_container'>
            {listing_status.map((item, index) => {
              return (
                <div className={item.symbol === ticker_symbol ? 'symbol_status_selected' : 'symbol_status'} key={index} onClick={()=>{
                  route.get_candles(item.symbol); 
                  set_is_listing_status(!is_listing_status);
                  set_ticker_symbol(item.symbol);
                  get_abcd_of_selected_symbol(item.symbol)
                  
                  }}>
            
                  <div className='symbol_status1'>{item.symbol}</div>
                  <div className='symbol_status2'>{item.name}</div>
                  <div className='symbol_status3'>{item.exchange}</div>
                  <div className='symbol_status3'>{item.assetType}</div>
                </div>
              );
          })}
                        

            </div>
            </div>
              </div>

            )
          }

          <div className='main'>

            <ChartMain
              abcd={abcd}
            prices={prices}
            retracement={retracement}
            candles_={candles_}
            formatted_candles={formatted_candles}
            chart_height={chart_height}
            Candle_Chart={Candle_Chart}
            is_listing_status={is_listing_status}
            set_is_listing_status={set_is_listing_status}
            ticker_symbol={ticker_symbol}
            set_canvas_dimensions={set_canvas_dimensions}
              selected_pattern={selected_pattern}
            />


            <PerformanceTable
              ticker_performance={ticker_performance}
              set_candles={set_candles}
              set_selected_peformance={set_selected_peformance}
              selected_peformance={selected_peformance}

              selected_pattern_index={selected_pattern_index}
              set_selected_pattern_index={set_selected_pattern_index}
              set_selected_pattern={set_selected_pattern}
              table={table}
              abcd_patterns={abcd_patterns}
            
            />

      
            
          </div>
          
              <Filter
              filters={filters}
              set_filters={set_filters}
              fetch_filtered={route.fetch_filtered}
            />

        </>}
        

        
			</div>
  );
}

export default App;

   