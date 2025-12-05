import React, { useState, useEffect, useRef } from 'react';
import './new.css'
import ChartMain from './candle_chart/chart_main';
import * as route from './backend_routes.js';
import Section from './Section.js';
import InfiniteTable from './InfiniteTable.js';
import * as tools from './MainTools.js'


const App = () => {

  const [market, set_market] = useState("Bullish")
  const [table_result, set_table_result] = useState('Won')
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

  // === FILTER ===
  const [activeFilters, setActiveFilters] = useState({
    market: {
      active: true,
      filter: "Bullish",
    },
    result: {
      active: false,
      filter: null,
    },
    retracement: {
      active: false,
      filter: null,
    },
  });

  const filter_stacker = (patterns) => {

    let result = patterns;

    if(activeFilters.market.active){
      result =  result.filter(p=>p.market===activeFilters.market.filter)
    }
    if(activeFilters.result.active){
      if(activeFilters.result.filter==="Open"){
        result =  result.filter(p=>p.trade_open==="true")
      }
      else{
        result =  result.filter(p=>p.trade_result===activeFilters.result.filter)
      }
      
    }

    if(activeFilters.retracement.active){
      
      result = result.filter(p =>
        p.trade_ab_price_retracement >= activeFilters.retracement.filter.ratios.ab_xa_gr &&
        p.trade_ab_price_retracement <= activeFilters.retracement.filter.ratios.ab_xa_lt &&

        p.trade_bc_price_retracement >= activeFilters.retracement.filter.ratios.bc_ab_gr &&
        p.trade_bc_price_retracement <= activeFilters.retracement.filter.ratios.bc_ab_lt &&

        p.trade_cd_bc_price_retracement >= activeFilters.retracement.filter.ratios.cd_bc_gr &&
        p.trade_cd_bc_price_retracement <= activeFilters.retracement.filter.ratios.cd_bc_lt &&

        p.trade_cd_xa_price_retracement >= activeFilters.retracement.filter.ratios.cd_xa_gr &&
        p.trade_cd_xa_price_retracement <= activeFilters.retracement.filter.ratios.cd_xa_lt
      );
    
    }
 
     return result
  }

  const [selected_harmonic_pattern, set_selected_harmonic_pattern] = useState("AB=CD Standard")
  const [price_retracement, set_price_retracement] = useState({
          ab_xa_gr: null,
          ab_xa_lt: null,
          bc_ab_gr: null,
          bc_ab_lt: null,
          cd_bc_gr: null,
          cd_bc_lt: null,
          cd_xa_gr: null,
          cd_xa_lt: null,
  })
  const harmonic_patterns = [
  {
    type: "AB=CD Standard",
    ratios: {
      ab_xa_gr: 0,  
      ab_xa_lt: 100,
      bc_ab_gr: 32,
      bc_ab_lt: 61,
      cd_bc_gr: 100,
      cd_bc_lt: 100,
      cd_xa_gr: 0,   
      cd_xa_lt: 100,
    }
  },
  {
    type: "AB=CD Alternate",
    ratios: {
      ab_xa_gr: 0,
      ab_xa_lt: 100,
      bc_ab_gr: 61,
      bc_ab_lt: 78,
      cd_bc_gr: 127,
      cd_bc_lt: 161,
      cd_xa_gr: 0,
      cd_xa_lt: 100,
    }
  },
  {
    type: "AB=CD Deep",
    ratios: {
      ab_xa_gr: 0,
      ab_xa_lt: 100,
      bc_ab_gr: 78,
      bc_ab_lt: 88,
      cd_bc_gr: 161,
      cd_bc_lt: 224,
      cd_xa_gr: 0,
      cd_xa_lt: 100,
    }
  },
  {
    type: "AB=CD Extended",
    ratios: {
      ab_xa_gr: 0,
      ab_xa_lt: 100,
      bc_ab_gr: 23,
      bc_ab_lt: 38,
      cd_bc_gr: 161,
      cd_bc_lt: 261,
      cd_xa_gr: 0,
      cd_xa_lt: 100,
    }
  },
  {
    type: "Gartley",
    ratios: {
      ab_xa_gr: 50,
      ab_xa_lt: 80,
      bc_ab_gr: 38.2,
      bc_ab_lt: 88.6,
      cd_bc_gr: 127.2,
      cd_bc_lt: 161.8,
      cd_xa_gr: 75,
      cd_xa_lt: 82,
    }
  },
  {
    type: "Bat",
    ratios: {
      ab_xa_gr: 38.2,
      ab_xa_lt: 50.0,
      bc_ab_gr: 38.2,
      bc_ab_lt: 88.6,
      cd_bc_gr: 161.8,
      cd_bc_lt: 261.8,
      cd_xa_gr: 85,
      cd_xa_lt: 92,
    }
  },
  {
    type: "Butterfly",
    ratios: {
      ab_xa_gr: 60,
      ab_xa_lt: 90,
      bc_ab_gr: 38.2,
      bc_ab_lt: 88.6,
      cd_bc_gr: 161.8,
      cd_bc_lt: 261.8,
      cd_xa_gr: 127,
      cd_xa_lt: 162,
    }
  },
  {
    type: "Crab",
    ratios: {
      ab_xa_gr: 30,
      ab_xa_lt: 60,
      bc_ab_gr: 38.2,
      bc_ab_lt: 88.6,
      cd_bc_gr: 224,
      cd_bc_lt: 361.8,
      cd_xa_gr: 155,
      cd_xa_lt: 170,
    }
  },
  {
    type: "Deep Crab",
    ratios: {
      ab_xa_gr: 80,
      ab_xa_lt: 95,
      bc_ab_gr: 38.2,
      bc_ab_lt: 88.6,
      cd_bc_gr: 224,
      cd_bc_lt: 361.8,
      cd_xa_gr: 85,
      cd_xa_lt: 92,
    }
  },
  {
    type: "Shark",
    ratios: {
      ab_xa_gr: 70,
      ab_xa_lt: 95,
      bc_ab_gr: 113,
      bc_ab_lt: 161.8,
      cd_bc_gr: 88.6,
      cd_bc_lt: 113,
      cd_xa_gr: 100,
      cd_xa_lt: 161.8,
    }
  },
  {
    type: "Cypher",
    ratios: {
      ab_xa_gr: 38,
      ab_xa_lt: 61,
      bc_ab_gr: 113,
      bc_ab_lt: 141,
      cd_bc_gr: 78,
      cd_bc_lt: 78,
      cd_xa_gr: 78,
      cd_xa_lt: 78,
    }
  },
  {
    type: "Three-Drive",
    ratios: {
      ab_xa_gr: 127,
      ab_xa_lt: 161,
      bc_ab_gr: 127,
      bc_ab_lt: 161,
      cd_bc_gr: 161,
      cd_bc_lt: 261,
      cd_xa_gr: 161,
      cd_xa_lt: 261,
    }
  }
  ];

  const harmonic_pattern_names = [
        "AB=CD Standard",
        "AB=CD Alternate",
        "AB=CD Deep",
        "AB=CD Extended",
        "Gartley",
        "Bat",
        "Butterfly",
        "Crab",
        "Deep Crab",
        "Shark",
        "Cypher",
        "Three-Drive"
  ]
  const update_harmonic_pattern = (selected_type) => {

      const pt = harmonic_patterns.filter(i => i.type === selected_type)[0].ratios;
      set_price_retracement(pt);
      const result = recent_patterns.filter(p =>
        p.trade_ab_price_retracement >= pt.ab_xa_gr &&
        p.trade_ab_price_retracement <= pt.ab_xa_lt &&

        p.trade_bc_price_retracement >= pt.bc_ab_gr &&
        p.trade_bc_price_retracement <= pt.bc_ab_lt &&

        p.trade_cd_bc_price_retracement >= pt.cd_bc_gr &&
        p.trade_cd_bc_price_retracement <= pt.cd_bc_lt &&

        p.trade_cd_xa_price_retracement >= pt.cd_xa_gr &&
        p.trade_cd_xa_price_retracement <= pt.cd_xa_lt
      );
      set_filtered_patterns(result);

  };

  useEffect(()=>{
    const rust_patterns = filter_stacker(recent_patterns)
    set_filtered_patterns(rust_patterns)

  },[activeFilters])

  // ===============

  useEffect(()=>{

    const fetch = async () => {

      // === LOADING ===
      set_loading(true);
      set_loading_patterns(true)



      // === FETCH  DATA === 
      const data = await route.fetch_abcd_patterns(market, filters)

      // === ALL PATTERNS ===
      const rust_patterns = data.patterns

      set_filtered_patterns(filter_stacker(data.patterns))
      set_recent_patterns(data.patterns)
      
      // === MONTNLY PEFORMANCE ===
      let rust_statistics = data.monthly_stats

      
      

      rust_statistics.sort((a, b) => a.month - b.month);
      set_monthly_peformance(rust_statistics)

      // CANDLES
      let candles = await route.get_candles(rust_patterns[0]?.symbol)
      candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
      candles = candles?.map(item => {
        const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

        return {
          ...item,
          candle_date: toISO(item.candle_date),
        };
      });


      // SUPPORT & RESISTANCE
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

  },[filters, market])

  const [filtered_patterns, set_filtered_patterns] = useState()
  
  const handle_updated_recent_patterns = async (month) => {

      const filtered_patterns = recent_patterns.filter(
          p => Number(p.trade_month) === Number(month) + 1
      );

      set_filtered_patterns(filtered_patterns);
      set_selected_month(Number(month) + 1)  
  }
  const sort_by_result = async (result) => {

    if(result==="Open"){
      const filtered_patterns = recent_patterns.filter(
        p => p.trade_open === "true"
      );
      set_table_result("Open")
      set_filtered_patterns(filtered_patterns);
    }
    else {
      const filtered_patterns = recent_patterns.filter(
        p => p.trade_result === result
      );
      if(result==='true'){
        set_table_result("Won")
      }
      else{
        set_table_result("Lost")
      }
      
      set_filtered_patterns(filtered_patterns);

    }
    


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

              <Section 
                price_retracement={price_retracement}
                update_harmonic_pattern={update_harmonic_pattern}
                harmonic_patterns={harmonic_patterns}
                selected_harmonic_pattern={selected_harmonic_pattern}
                set_selected_harmonic_pattern={set_selected_harmonic_pattern}
                sort_by_result={sort_by_result} table_result={table_result}
                market={market}set_market={set_market} title={'Recent Patterns'} 
                length={recent_patterns?.length}
                activeFilters={activeFilters}
                setActiveFilters={setActiveFilters}
                body={<InfiniteTable
                  recent_patterns={filtered_patterns}
                  set_loading_patterns={set_loading_patterns}
                  set_chart_data={set_chart_data}
                  selected_row_index={selected_row_index}
                  set_selected_row_index={set_selected_row_index}
                />}
              />

            </div>

            <div className='main_left'>

                  
              <ChartMain
                chart_data={chart_data}
                is_loading_patterns={is_loading_patterns}
                all_watchlists={all_watchlists}
                is_sections_expanded={is_sections_expanded}
                set_sections_expanded={set_sections_expanded}
                market={market}

              />

           

            </div>
      
     
          
          </div>

			</div>
  );
}

export default App;

   











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