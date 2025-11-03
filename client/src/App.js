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
      
      const monthly_peformance = await route.get_monthly_peformance(filters)
      set_monthly_peformance(monthly_peformance)
  

      const recent_patterns = await route.get_recent_patterns(filters)
      // const selected_pattern = recent_patterns[0]
      set_recent_patterns(recent_patterns)


      // let candles = await route.get_candles(recent_patterns[0]?.symbol)
      // candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
      // candles = candles?.map(item => {
      //   const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

      //   return {
      //     ...item,
      //     candle_date: toISO(item.candle_date),
      //   };
      // });
      
      // const sorted = recent_patterns.sort((a, b) => a.symbol.localeCompare(b.symbol));
  
      // set_recent_patterns(sorted)

      // tools.format_pattern(candles, selected_pattern, set_chart_data)

      // const wl_patterns = await route.get_all_patterns_in_watchlist()

      // set_wl_patterns(wl_patterns)

   

    
    }
    fetch()

    
  
  },[])

  const handle_updated_recent_patterns = async () => {
    
    set_loading(true)
    const recent_patterns = await route.get_recent_patterns(filters)
    const selected_pattern = recent_patterns[0]
    const sorted = recent_patterns.sort((a, b) => a.symbol.localeCompare(b.symbol));
    set_recent_patterns(sorted)


    // let candles = await route.get_candles(recent_patterns[0]?.symbol)
    // candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
    // candles = candles?.map(item => {
    //   const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    //   return {
    //     ...item,
    //     candle_date: toISO(item.candle_date),
    //   };
    // });
    // tools.format_pattern(candles, selected_pattern, set_chart_data)


    // // const wl_patterns = await route.get_all_patterns_in_watchlist()
    // // set_wl_patterns(wl_patterns)


    const monthly_peformance = await route.get_monthly_peformance(filters)
    set_monthly_peformance(monthly_peformance)
    set_loading(false)
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
              {months.map((monthName, index) => {
                const monthData = monthly_performance[index];
                const pct = monthData?.win_pct || 0;

                const r = 0;
                const g = Math.round(128 * (pct / 100)); 
                const b = Math.round(128 * (pct / 100)); 
                const fillColor = `rgba(${r}, ${g}, ${b}, 0.8)`; 

                return (
                  <div
                    key={index}
                    className="month-wrapper"
                    style={{
                      position: "relative",
                      borderRadius: "4px",
                      overflow: "hidden",
                    }}
                  >
                    {/* Background fill */}
                    <div
                      className="win_pct_background"
                      style={{
                        width: `${pct}%`,
                        backgroundColor: fillColor,
                        transition: "width 0.3s ease",
                      }}
                    />

                    {/* Centered text */}
                    <span
                      style={{
                        position: "absolute",
                        left: "50%",
                        top: "50%",
                        transform: "translate(-50%, -50%)",
                        fontWeight: "bold",
                        color: "#fff",
                      }}
                    >
                       <div className='row__'> {monthName}</div>
                       <div className='row__'> {monthly_performance[index]?.count_total}</div>
                      <div className='row__'> {pct}%</div>
                     
                    </span>
                  </div>
                );
              })}

            </div>

            <div className={is_sections_expanded ? 'sections-wrapper-expanded':'sections-wrapper-minimized'}>

              <Section title={'Build Pattern'} body={<Filter
                filters={filters}
                set_filters={set_filters}
                fetch_filtered_peformances={route.fetch_filtered_peformances}
                set_ticker_peformance={set_ticker_peformance}
                is_loading={is_loading}
                set_loading={set_loading}
                handle_updated_recent_patterns={handle_updated_recent_patterns}
           
                
              />}/>

              {/* <Section title={'Watchlist'} body={<WatchList
                
                all_watchlists={all_watchlists}
                set_all_watchlist={set_all_watchlist}
                all_wl_patterns={all_wl_patterns}
                set_chart_data={set_chart_data}
              />}/> */}

              <Section title={'Recent Patterns'} length={recent_patterns?.length}body={<InfiniteTable
                recent_patterns={recent_patterns}
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
          
          </div>

			</div>
  );
}

export default App;

   