import React, { useState, useEffect, useRef } from 'react';
import { Candle_Chart } from './candle_chart';
import './new.css'
import search from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/search.png';
import abcd from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/abcd.png';
import prices from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/prices.png';
import retracement from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/retracement.png';
import candles_ from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/candles.png';
import Table from './table';
import Filter from './filter';

const App = () => {

  const chart_container = useRef(null)
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
  const get_ab_candles = async () => {
  try {
    const res = await fetch("http://localhost:8000/ab_candles", {
      method: "GET",
      headers: { "Content-Type": "application/json" }
    });

    const responseData = await res.json();

    const sortedData = responseData.data.sort((a, b) => {
    const firstCompare = new Date(a.pattern_A_pivot_date) - new Date(b.pattern_A_pivot_date);
    
    if (firstCompare !== 0) {
      return firstCompare; // sort by first date
    }
    
    // if same first date → sort by second date
    return new Date(a.pattern_B_pivot_date) - new Date(b.pattern_B_pivot_date);
  });

set_ab_candles(sortedData);

  } catch (error) {
    console.log(error);
  }
  };
  const get_abc_candles = async () => {
    try {
        const res = await fetch("http://localhost:8000/abc_patterns", {
          method: "GET",
          headers: { "Content-Type": "application/json" }
        });

        const responseData = await res.json();
       
        set_abc_patterns(responseData.data)
    
    } catch (error) {
        console.log(error);
    }
  };
  const get_abcd_candles = async () => {
    try {
        const res = await fetch("http://localhost:8000/abcd_patterns", {
          method: "GET",
          headers: { "Content-Type": "application/json" }
        });

        const responseData = await res.json();
 
        set_statistics(responseData.stats)
        set_abcd_patterns(responseData.data)
        set_data(responseData)
        
    
    } catch (error) {
        console.log(error);
    }
  };
  const get_candles = async (symbol) => {

    try{
      const res = await fetch('http://localhost:8000/get_selected_ticker', 
        {
          method: "POST", 
          headers: {"Content-Type": "application/json",}, 
          body: JSON.stringify({'symbol':symbol})
        });

      if (!res.ok) {
        console.error(`Server Error: ${res.status} - ${res.statusText}`);
        throw new Error("Request failed");
      }
      const responseData = await res.json();
      set_candles(responseData.data)

    } catch(error) {
      console.error(error)
    }

  }
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
          // o: item.candle_open,
      }))
      

  
      let chartHeight = canvas_dimesions.chart_height

      const current_pixels_per_price_unit = chartHeight / 10

      const toCanvasY = (price) => {
          let a = price * current_pixels_per_price_unit
          let price_px = chartHeight - a;
          return price_px
      }         
      const toHeight = (close, open) => (close - open) * current_pixels_per_price_unit;

      let startX = 0;
      new_candles = new_candles.map((candle) => {
          startX += 15;
          const bottom = toCanvasY(Math.min(candle.open, candle.close));
          const height = (candle.candle_close - candle.candle_open) * current_pixels_per_price_unit; 
          const high = toCanvasY(candle.high);
          const low = toCanvasY(candle.low);

          return {
              ...candle,
              prev_high : high,
              prev_height : height,
              prev_bottom : bottom,
              prev_low : low,
              current_high : high,
              current_height : height,
              current_bottom : bottom,
              current_low : low,
          };
      });
      set_formatted_candles(new_candles)
  


  }, [candles])
  useEffect(()=>{
    get_candles(ticker_symbol); 
    // get_listed_tickers('',asset_type)
    // get_abcd_of_selected_symbol(ticker_symbol)
    get_ab_candles()
    get_abc_candles()
    get_abcd_candles()
  
  }, [])  
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
  
  const optionCells = [
      "X",
    "A",
    "B",
    "C",
    "D",
    "AB",
    "BC",
    "CD",
    "ABC",
    "ABCD",
    "Prices",
    "Dates",
    "Lifecycle",
    "Performance"
  ]
  const [table_columns, set_table_columns] = useState([])
  const columnGroups = {
    a: ["pattern_A_pivot_date", "pattern_A_high", "pattern_A_close","pattern_A_open","pattern_A_low"],
    b: ["pattern_B_pivot_date", "pattern_B_high", "pattern_B_close","pattern_B_open","pattern_B_low"],
    c: ["pattern_C_pivot_date", "pattern_C_high", "pattern_C_close","pattern_C_open","pattern_C_low","pattern_C_price_retracement",'pattern_C_bar_retracment'],
    d: ["pattern_d_created_date","pattern_D_price_retracement", "pattern_D_bar_retracement","d_dropped_below_b"],
    ab: [
      "pattern_AB_start_date", "ab_price_length",
      "pattern_AB_end_date", "pattern_AB_bar_length"
    ],
    bc: ["bc_price_length", "pattern_BC_bar_length"],
    cd: ["cd_price_length", "pattern_CD_bar_length"],
    abc: [
      "pattern_ABC_start_date", "pattern_ABC_end_date",
      "pattern_ABC_bar_length"
    ],
    abcd: [
      "pattern_ABCD_bar_length", "pattern_ABCD_start_date",
      "pattern_ABCD_end_date"
    ],
    lifecycle: [
      "trade_created", "trade_entered_date", "trade_exited_date",
      "trade_duration_bars", "trade_duration_days",
      "trade_is_open", "trade_is_closed", "valid"
    ],
    prices: [
      "trade_entered_price", "trade_exited_price",
      "trade_stop_loss", "trade_take_profit"
    ],
    performance: [
      "trade_pnl", "trade_result", "trade_return_percentage",
      "trade_rrr", "trade_risk", "trade_reward"
    ]
  };
  const columnGroupsTitles = {
  a: ["Date", "High", "Close", "Open", "Low"],
  b: ["Date", "High", "Close", "Open", "Low"],
  c: ["Date", "High", "Close", "Open", "Low", "Retracement", 'Retracement (Candles)'],
  d: ["Date", "Retracement", 'Retracement (Candles)'],

  ab: [
    "AB Start Date", "AB Price Length",
    "AB End Date", "AB Bar Length"
  ],
  bc: ["BC Price Length", "BC Bar Length"],
  cd: ["CD Price Length", "CD Bar Length"],
  abc: [
    "ABC Start Date", "ABC End Date",
    "ABC Bar Length"
  ],
  abcd: [
    "ABCD Bar Length", "ABCD Start Date",
    "ABCD End Date"
  ],
  lifecycle: [
    "Trade Created", "Trade Entered Date", "Trade Exited Date",
    "Trade Duration Bars", "Trade Duration Days",
    "Trade Is Open", "Trade Is Closed", "Valid"
  ],
  prices: [
    "Trade Entered Price", "Trade Exited Price",
    "Trade Stop Loss", "Trade Take Profit"
  ],
  performance: [
    "Trade PnL", "Trade Result", "Trade Return %",
    "Trade RRR", "Trade Risk", "Trade Reward"
  ]
  };
  

  // FETCH FILTERED =====
  const fetch_filtered = async (value) => {

    try {
        const res = await fetch("http://localhost:8000/filtered_patterns", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ value }) 
        });

        const responseData = await res.json();
        set_statistics(responseData.stats)
        set_abcd_patterns(responseData.data)  
        handle_selected_option(responseData.data, selected_table_option)

        
    
    } catch (error) {
        console.log(error);
    }
  }

  // SET TABLE
  const handle_selected_option = (abcd_patterns, option) => {

  
    const key = option.toLowerCase();
    const selectedColumns = columnGroups[key]; 
    if (!selectedColumns) {
      console.warn("No columns found for option:", key);
      return;
    }
    const newObj = abcd_patterns.map((row) =>
      selectedColumns.reduce((acc, col) => {
        acc[col] = row[col];
        return acc;
      }, {})
    );
    set_table(newObj);
    set_table_columns(columnGroupsTitles[key])
    console.log(newObj.length)
  };


  

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
                  get_candles(item.symbol); 
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

            <div className='statistics'>
              <div className='stat1_header'>
                Count
              </div>
              <div className='stat1'>
                <div className='key'>Win Pct</div>
                <div className='value'>{statistics.win_pct}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Total</div>
                <div className='value'>{statistics.count_total}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Won</div>
                <div className='value'>{statistics.count_won}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Lost</div>
                <div className='value'>{statistics.count_lost}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Open</div>
                <div className='value'>{statistics.count_open}</div>
              </div>

              <div className='stat1_header'>
                BC Retracement
              </div>
              <div className='stat1'>
                <div className='key'>Avg</div>
                <div className='value'>{statistics.retracement?.bc.avg}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Max</div>
                <div className='value'>{statistics.retracement?.bc.max}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Median</div>
                <div className='value'>{statistics.retracement?.bc.median}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Min</div>
                <div className='value'>{statistics.retracement?.bc.min}</div>
              </div>
              <div className='stat1'>
                <div className='key'>STD</div>
                <div className='value'>{statistics.retracement?.bc.std}</div>
              </div>

              <div className='stat1_header'>
                CD Retracement
              </div>
              <div className='stat1'>
                <div className='key'>Avg</div>
                <div className='value'>{statistics.retracement?.cd.avg}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Max</div>
                <div className='value'>{statistics.retracement?.cd.max}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Median</div>
                <div className='value'>{statistics.retracement?.cd.median}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Min</div>
                <div className='value'>{statistics.retracement?.cd.min}</div>
              </div>
              <div className='stat1'>
                <div className='key'>STD</div>
                <div className='value'>{statistics.retracement?.cd.std}</div>
              </div>

              <div className='stat1_header'>
                AB Size
              </div>
              <div className='stat1'>
                <div className='key'>Avg</div>
                <div className='value'>{statistics.size?.ab.avg}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Max</div>
                <div className='value'>{statistics.size?.ab.max}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Median</div>
                <div className='value'>{statistics.size?.ab.median}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Min</div>
                <div className='value'>{statistics.size?.ab.min}</div>
              </div>
              <div className='stat1'>
                <div className='key'>STD</div>
                <div className='value'>{statistics.size?.ab.std}</div>
              </div>

              <div className='stat1_header'>
                BC Size
              </div>
              <div className='stat1'>
                <div className='key'>Avg</div>
                <div className='value'>{statistics.size?.bc.avg}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Max</div>
                <div className='value'>{statistics.size?.bc.max}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Median</div>
                <div className='value'>{statistics.size?.bc.median}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Min</div>
                <div className='value'>{statistics.size?.bc.min}</div>
              </div>
              <div className='stat1'>
                <div className='key'>STD</div>
                <div className='value'>{statistics.size?.bc.std}</div>
              </div>

              <div className='stat1_header'>
                CD Size
              </div>
              <div className='stat1'>
                <div className='key'>Avg</div>
                <div className='value'>{statistics.size?.cd.avg}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Max</div>
                <div className='value'>{statistics.size?.cd.max}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Median</div>
                <div className='value'>{statistics.size?.cd.median}</div>
              </div>
              <div className='stat1'>
                <div className='key'>Min</div>
                <div className='value'>{statistics.size?.cd.min}</div>
              </div>
              <div className='stat1'>
                <div className='key'>STD</div>
                <div className='value'>{statistics.size?.cd.std}</div>
              </div>




            </div> 

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

                <div className='data_half'>

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
  
                <div className='data_half'>

                  <div className='table_options'>
                  {optionCells.map((label, idx) => (
                    <div key={idx} className={label === selected_table_option ? 'option_cell_selected': 'option_cell'}
                    onClick={()=>{set_selected_table_option(label);
                      handle_selected_option(abcd_patterns, label)
                    }}>
                          {label}
                        </div>
                      ))}

                  </div>
                  <div className='strategy_station_con'>
                    
                    <div className='chart_header'>
                        {table_columns.map((col,index)=>{
                            return(
                                <div key={index}  className='col1'>{col}</div>
                            )
                        })}
                        
                    </div>

                    <Table 
                      table_columns={table_columns}
                      selected_pattern_index={selected_pattern_index}
                      set_selected_pattern={set_selected_pattern}
                      set_selected_pattern_index={set_selected_pattern_index}
                      table={table}
                      abcd_patterns={abcd_patterns}
                    />
                
              
                  </div> 
                </div>
              </div>


            </div>

            

            <Filter
              filters={filters}
              set_filters={set_filters}
              fetch_filtered={fetch_filtered}
            />
            
          </div>

        </>}
        

        
			</div>
  );
}

export default App;

   