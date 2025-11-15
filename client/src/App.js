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
import PatternTable from './PatternTable.js';
import Section from './Section.js';
import RecentPatterns from './RecentPatterns.js';
import InfiniteTable from './InfiniteTable.js';

import * as tools from './MainTools.js'
import Trades from './Trades.js';
import WatchList from './WatchList.js';


const App = () => {

  const [chart_data, set_chart_data] = useState({candles: [], abcd_pattern: []})
  const [selected_month, set_selected_month] = useState('1')
  const [is_sections_expanded, set_sections_expanded] = useState(true)
  const [selected_row_index, set_selected_row_index] = useState(0)
  const [monthly_performance, set_monthly_peformance] = useState([])
  const [all_watchlists, set_all_watchlist] = useState([])
  const [all_wl_patterns, set_wl_patterns] = useState([])
  const [is_loading, set_loading] = useState(false)
  const [is_loading_patterns, set_loading_patterns] = useState(false)
  const [ticker_performance, set_ticker_peformance] = useState([])
  const [recent_patterns, set_recent_patterns] = useState([])
  const [filters, set_filters] = useState({
    bc_retracement_greater: 0,
    bc_retracement_less: 101,
    cd_retracement_greater: 0,
    cd_retracement_less: 1000,
    ab_leg_greater: 0,
    ab_leg_less: 5000,
    bc_leg_greater: 0,
    bc_leg_less: 5000,
    cd_leg_greater: 0,
    cd_leg_less: 5000,
  })
  const months = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December"
  ];

  // useEffect(() => {
 
  //   handle_updated_recent_patterns(set_recent_patterns, set_wl_patterns, set_monthly_peformance, set_chart_data, filters)
  
  // }, [filters]);
  
  // useEffect(() => {
  // const handleKeyDown = async (event) => {
  //   if (event.key === "ArrowRight") {
  //     if (selected_row_index < recent_patterns.length - 1) {
  //       const newIndex = selected_row_index + 1;
  //       // set_selected_pattern_index(newIndex);
  //       set_selected_row_index(newIndex);

  //       let candles = await route.get_candles(recent_patterns[newIndex].symbol)
  //       candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
  //       candles = candles.map(item => {
  //         const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

  //         return {
  //           ...item,
  //           candle_date: toISO(item.candle_date),
  //         };
  //       });
  //       tools.format_pattern(candles, recent_patterns[newIndex], set_chart_data)

  //     }
  //   }

  //   if (event.key === "ArrowLeft") {
  //     if (selected_row_index > 0) {
  //         const newIndex = selected_row_index - 1;
  //         // set_selected_pattern_index(newIndex);
  //         set_selected_row_index(newIndex);
  //         let candles = await route.get_candles(recent_patterns[newIndex].symbol)
  //         candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
  //         candles = candles.map(item => {
  //           const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

  //           return {
  //             ...item,
  //             candle_date: toISO(item.candle_date),
  //           };
  //         });
  //         tools.format_pattern(candles, recent_patterns[newIndex], set_chart_data)
  //       }
  //   }
  // };

  // window.addEventListener("keydown", handleKeyDown);
  // return () => window.removeEventListener("keydown", handleKeyDown);
  // }, [selected_row_index, recent_patterns]);

  useEffect(()=>{

    const fetch = async () => {

      set_loading(true);
      set_loading_patterns(true)


      const data = await route.fetch_abcd_patterns(filters)
      const rust_patterns = data.patterns

      let rust_statistics = data.monthly_stats
     
      rust_statistics.sort((a, b) => a.month - b.month);
      
      
      // MONTHLY PEFORMANCE
      // const monthly_peformance = await route.get_monthly_peformance(filters, selected_month)
      // const stats2025 = rust_statistics
      // .filter(entry => entry.year === 2025)       // keep only year 2025
      // .sort((a, b) => a[1] - b[1]);   
      // console.log(stats2025)
      set_monthly_peformance(rust_statistics)
  
      // RECENT PATTERNS
      let recent_patterns = await route.get_recent_patterns(filters,selected_month)
      // recent_patterns = recent_patterns.sort((a, b) => a.symbol.localeCompare(b.symbol));
      recent_patterns = recent_patterns.sort((a, b) => new Date(a.pattern_A_pivot_date) - new Date(b.pattern_A_pivot_date));
      set_recent_patterns(rust_patterns)
      set_filtered_patterns(rust_patterns)

      // CANDLES
      const selected_pattern = recent_patterns[0]
      let candles = await route.get_candles(rust_patterns[0]?.symbol)
      candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
      candles = candles?.map(item => {
        const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

        return {
          ...item,
          candle_date: toISO(item.candle_date),
        };
      });


      
//       // SUPPORT & RESISTANCE
      const snr_lines = await route.get_support_resistance_lines(recent_patterns[0]?.symbol)
      // tools.format_pattern(candles, selected_pattern, snr_lines, set_chart_data, rust_patterns[0])
      tools.format_pattern(candles, rust_patterns[0], snr_lines, set_chart_data)

      // WATCHLIST
      // const wl_patterns = await route.get_all_patterns_in_watchlist()
      // set_wl_patterns(wl_patterns)

      set_loading(false);
      set_loading_patterns(false)
    
    }
    fetch()

    
  
  },[filters])

  const [filtered_patterns, set_filtered_patterns] = useState()
  
  const handle_updated_recent_patterns = async (month) => {

  
      const filtered_patterns = recent_patterns.filter(
          p => Number(p.trade_month) === Number(month) + 1
      );

      set_filtered_patterns(filtered_patterns);
      set_selected_month(Number(month) + 1)
      // set_loading(false);
    
  }
  

  return (

      <div className='App' >
  
          <div className='main'>
            {is_loading && 
              <div className='overlay'>
                <div className='loading_container'>Loading...</div>
              </div>
            }

            <div className='monthly_peformance_wrapper'>

                <div className='table_header_text'>
                  Peformance

                </div>

                {Object.keys(monthly_performance).map((key, index)=>{

        
                  
                  const pct = monthly_performance[key]?.win_pct || 0;
                  const count_total = monthly_performance[key]?.closed || 0;

                  return (
                    <div
                      key={index}
                      className={selected_month === Number(key) + 1 ? 'month-wrapper-active' : 'month-wrapper'}
                      onClick={() => handle_updated_recent_patterns(key)}
                      style={{
                        position: "relative",
                        borderRadius: "4px",
                        overflow: "hidden",
                      }}
                    >
                
                      <div
                        className="win_pct_background"
                        style={{
                          width: `${pct}%`,
                        
                          transition: "width 0.3s ease",
                          border: '2px solid rgba(75, 214, 226, 0.36)',
                          height: '100%',
                          boxSizing: 'border-box'

                        }}
                      />

                  
                      <div
                        style={{
                          position: "absolute",
                          left: "50%",
                          top: "50%",
                          transform: "translate(-50%, -50%)",
                          fontWeight: "bold",
                          color: "#fff",
                          textAlign: "center",
                          display: 'flex',
                          width: '100%'
                          
                        }}
                      >
                        <div className='row__'>{key}</div>
                        <div className='row__'>{count_total}</div>
                        <div className='row__'>{pct.toFixed(0)}%</div>
                      </div>
                    </div>
                    );
                  })}

                <div className='table_header_main'>
            
                </div>

            </div>

            <div className={is_sections_expanded ? 'sections-wrapper-expanded':'sections-wrapper-minimized'}>

         
              {/* <Section title={'Build Pattern'} body={<Filter
                filters={filters}
                set_filters={set_filters}
                fetch_filtered_peformances={route.fetch_filtered_peformances}
                set_ticker_peformance={set_ticker_peformance}
                is_loading={is_loading}
                set_loading={set_loading}
                handle_updated_recent_patterns={handle_updated_recent_patterns}
           
                
              />}/>  */}
              {/* <Section title={'Watchlist'} body={<WatchList
                
                all_watchlists={all_watchlists}
                set_all_watchlist={set_all_watchlist}
                all_wl_patterns={all_wl_patterns}
                set_chart_data={set_chart_data}
              />}/> */}

              <Section title={'Recent Patterns'} length={recent_patterns?.length}body={<InfiniteTable
                recent_patterns={filtered_patterns}
                set_loading_patterns={set_loading_patterns}
                set_chart_data={set_chart_data}
                selected_row_index={selected_row_index}
                set_selected_row_index={set_selected_row_index}
              />}/>

            </div>

            <div className='main_left'>

                  
              <ChartMain
                chart_data={chart_data}
                is_loading_patterns={is_loading_patterns}
                all_watchlists={all_watchlists}
                is_sections_expanded={is_sections_expanded}
                set_sections_expanded={set_sections_expanded}

              />

           

            </div>
            
            <div className='pattern-builder-wrapper'>
                          <Filter
                filters={filters}
                set_filters={set_filters}
                fetch_filtered_peformances={route.fetch_filtered_peformances}
                set_ticker_peformance={set_ticker_peformance}
                is_loading={is_loading}
                set_loading={set_loading}
                handle_updated_recent_patterns={handle_updated_recent_patterns}
           
                
              />

            </div>

     
          
          </div>

			</div>
  );
}

export default App;

   