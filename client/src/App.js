import React, { useState, useEffect } from 'react';
import './new.css'
import ChartMain from './candle_chart/chart_main';
import * as route from './backend_routes.js';
import Section from './Section.js';
import InfiniteTable from './InfiniteTable.js';
import * as tools from './MainTools.js'

import FilterDropDown from './FilterDropDown.js';

const handle_candles = async(symbol) => {

  let candles = await route.get_candles(symbol)
  candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
  candles = candles?.map(item => {
    const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    return {
      ...item,
      candle_date: toISO(item.candle_date),
    };
  });

  return candles

}
const format_patterns = async (candles, patterns) => {
  
  const formatted_patterns = []
  patterns.map(p=>{
    const pattern = tools.format_pattern_(candles, p)
    formatted_patterns.push(pattern)
  })

  return formatted_patterns

}
const update_selected_pattern = async (selected_pattern, set_chart_data) => {

  // GET SELECTED PATTERN CANDLES
  let candles = await route.get_candles(selected_pattern?.symbol)
  candles?.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
  candles = candles?.map(item => {
    const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    return {
      ...item,
      candle_date: toISO(item.candle_date),
    };
  });

  // GET SELECTED PATTERN SUPPORT & RESISTANCE
  const snr_lines = await route.get_support_resistance_lines(selected_pattern?.symbol)

  // FORMAT SELECTED PATTERN FOR CANVAS 
  tools.format_pattern(candles, selected_pattern, snr_lines, set_chart_data)

}
const filter_stacker = (patterns, activeFilters) => {

    let result = patterns;

    if(activeFilters.market.active){
      result =  result.filter(p=>p.market===activeFilters.market.filter)
    }
    if(activeFilters.result.active){
      if(activeFilters.result.filter==="Open"){
        result =  result.filter(p=>p.trade_open==="1")
      }
      else{
        result =  result.filter(p=>p.trade_result===activeFilters.result.filter)
      }
      
    }
    if(activeFilters.retracement.active){
     
      result = result.filter(p =>
        p.trade_ab_price_retracement >= activeFilters.retracement.filter.ab_xa_gr &&
        p.trade_ab_price_retracement <= activeFilters.retracement.filter.ab_xa_lt &&

        p.trade_bc_price_retracement >= activeFilters.retracement.filter.bc_ab_gr &&
        p.trade_bc_price_retracement <= activeFilters.retracement.filter.bc_ab_lt &&

        p.trade_cd_bc_price_retracement >= activeFilters.retracement.filter.cd_bc_gr &&
        p.trade_cd_bc_price_retracement <= activeFilters.retracement.filter.cd_bc_lt &&

        p.trade_cd_xa_price_retracement >= activeFilters.retracement.filter.cd_xa_gr &&
        p.trade_cd_xa_price_retracement <= activeFilters.retracement.filter.cd_xa_lt
      );
    
    }
    if(activeFilters.date.active){
      result =  result.filter(p=>p.d_date===activeFilters.date.filter)
    }
    if(activeFilters.symbol.active){
      result =  result.filter(p=>p.symbol===activeFilters.symbol.filter)
    }
    if(activeFilters.snr.active){

      if(activeFilters.snr.filter === 3){
        result = result.filter(p=>p.three_month === "true")
      }
      if(activeFilters.snr.filter === 6){
        result = result.filter(p=>p.six_month === "true")
      }
      if(activeFilters.snr.filter === 12){
        result = result.filter(p=>p.twelve_month === "true")
      }
      
   
    }

 
     return result
}


const App = () => {

  const [pattern_group_ids, set_pattern_group_ids] = useState([])
  const [grouped_pattern_data, set_grouped_pattern_data] = useState({
    symbol: null,
    candles: [],
    rust_patterns: [],
  })
  const [is_selected_xabcd, set_is_selected_xabcd] = useState(false)
  const [selected_xabcd, set_selected_xabcd] = useState({})


  useEffect(()=>{

    const fetch = async () => {

      // === LOADING ===
      set_loading(true);
      set_loading_patterns(true)

      // FETCH PATTERNS
      const data = await route.fetch_abcd_patterns(market, filters)

      const rust_patterns = data.patterns
      const filtered_patterns = filter_stacker(data.patterns, activeFilters)
      
      set_filtered_patterns(filtered_patterns)
      set_recent_patterns(data.patterns)

      // GET UNIQUE DATES FOR PATTERN DATE LIST
      const uniqueDates = [...new Set(filtered_patterns.map(item => item.d_date))];
      set_unique_d_dates(uniqueDates)

      // GET UNIQUE SYMBOLS FOR PATTERN DATE LIST
      const uniqueSymbols = [...new Set(filtered_patterns.map(item => item.symbol))];
      set_unique_symbols(uniqueSymbols)

      // GET UNIQUE GROUP IDS
      const uniqueGroupIds = [...new Set(filtered_patterns.map(item => item.pattern_group_id))];
      set_pattern_group_ids(uniqueGroupIds)

      const selected_group_id = uniqueGroupIds[1]

      const selected_grouped_pattern = filtered_patterns.filter(p => p.pattern_group_id === selected_group_id);


      
      const candles = await handle_candles(selected_grouped_pattern[0].symbol)
      const formatted_patterns = await format_patterns(candles, selected_grouped_pattern)

      // === MONTNLY PEFORMANCE ===
      // let rust_statistics = data.monthly_stats
      // rust_statistics.sort((a, b) => a.month - b.month);
      // set_monthly_peformance(rust_statistics)

      // SET INITAL SELECTED PATTERN
      let selected_pattern = rust_patterns[0]
      update_selected_pattern(selected_pattern, set_chart_data)

      const formatted_data = {
          symbol: selected_grouped_pattern[0].symbol,
          candles: candles,
          rust_patterns: formatted_patterns
      }
      set_grouped_pattern_data(formatted_data)
  
          
      // === LOADING ===
      set_loading(false);
      set_loading_patterns(false)
    
    }
    fetch()

  },[])

  // ============================================

  const [market, set_market] = useState("Bullish")
  const [table_result, set_table_result] = useState('Won')
  const [chart_data, set_chart_data] = useState({candles: [], abcd_pattern: []})

  const [is_sections_expanded, set_sections_expanded] = useState(true)
  const [selected_row_index, set_selected_row_index] = useState(0)
  // const [monthly_performance, set_monthly_peformance] = useState([])
  // const [all_watchlists, set_all_watchlist] = useState([])
  // const [all_wl_patterns, set_wl_patterns] = useState([])
  const [is_loading, set_loading] = useState(false)
  const [is_loading_patterns, set_loading_patterns] = useState(false)
  // const [ticker_performance, set_ticker_peformance] = useState([])
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
    snr: {
      active: false,
      filter: null,
    },
    date: {
      active: false,
      filter: null,
    },
    symbol: {
      active: false,
      filter: null,
    },
  });

  

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
    type: "Standard",
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
    type: "Alternate",
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
    type: "Deep",
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
    type: "Extended",
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

  

  
  const [unique_d_dates, set_unique_d_dates] = useState([])
  const [unique_symbols, set_unique_symbols] = useState([])

  

  const [filtered_patterns, set_filtered_patterns] = useState()
  

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


  const options4 = [ "Standard",
        "Alternate",
        "Deep",
        "Extended",
        "Gartley",
        "Bat",
        "Butterfly",
        "Crab",
        "Deep Crab",
        "Shark",
        "Cypher",
        "Three-Drive"];


  const activateMarket = value => {
        setActiveFilters(prev => ({
            ...prev,
            market: { active: true, filter: value }
        }));
  };
  const activateResult = value => {

    let val = value === 'Open' ? "Open" : value === 'Won' ? '1' : '0'
        setActiveFilters(prev => ({
            ...prev,
            result: { active: true, filter: val }
        }));
  };
  const activateRetracement = value => {
  
    let val = harmonic_patterns.find(p => p.type === value)?.ratios || null;


    setActiveFilters(prev => ({
        ...prev,
        retracement: { 
            active: true, 
            filter: val 
        }
    }));
  };
  const activateDate = value => {
        setActiveFilters(prev => ({
            ...prev,
            date: { active: true, filter: value }
        }));
  };
  const activateSymbol = value => {
        setActiveFilters(prev => ({
            ...prev,
            symbol: { active: true, filter: value }
        }));
  };

  const [selected_filter, set_selected_filter] = useState('')

  useEffect(() => {
    let isMounted = true;

    

    const loadPattern = async () => {
      try {
        set_loading(true);

     
        const rust_patterns = filter_stacker(recent_patterns);
    
        
        set_filtered_patterns(rust_patterns.sort((a,b)=> a.trade_enter_price - b.trade_enter_price));

        // const uniqueDates = [...new Set(rust_patterns.map(item => item.d_date))];
        // set_unique_d_dates(uniqueDates)
        // const uniqueSymbols = [...new Set(rust_patterns.map(item => item.symbol))];
        // set_unique_symbols(uniqueSymbols)

        if (!rust_patterns?.length) {
          set_chart_data({ selected: null, candles: [] });
          return;
        }

        await update_selected_pattern(rust_patterns[0], (data) => {
          if (isMounted) set_chart_data(data);
        });

      } catch (err) {
        console.error("Pattern load failed:", err);
        if (isMounted) {
          set_chart_data({ selected: null, candles: [] });
        }
      } finally {
        if (isMounted) set_loading(false);
      }
    };

    loadPattern();

    set_selected_row_index(0)

    return () => {
      isMounted = false;
    };
  }, [activeFilters, recent_patterns]);

  console.log(grouped_pattern_data)

  return (

      <div className='App' >

        <div className='app-inner'>

          <div className='main'>

            {
  is_selected_xabcd ? (
    <ChartMain
      chart_data={{
        candles: grouped_pattern_data.candles,
        rust_patterns: grouped_pattern_data.rust_patterns[0],
      }}
      is_loading_patterns={is_loading_patterns}
      is_sections_expanded={is_sections_expanded}
      set_sections_expanded={set_sections_expanded}
      market={market}
      set_selected_xabcd={set_selected_xabcd}
      is_selected_xabcd={is_selected_xabcd}
      set_is_selected_xabcd={set_is_selected_xabcd}
    />
  ) : (
    <>
      {is_loading && (
        <div className="overlay">
          <div className="loading_container">Loading...</div>
        </div>
      )}

      <div className="app-header">
        <FilterDropDown
          name="Market"
          activateMarket={activateMarket}
          options={["Bullish", "Bearish", "Both"]}
          set_selected_filter={set_selected_filter}
          selected_filter={selected_filter}
          open={selected_filter === "Market"}
        />

        <FilterDropDown
          name="Status"
          activateMarket={activateResult}
          options={["Open", "Won", "Lost"]}
          set_selected_filter={set_selected_filter}
          selected_filter={selected_filter}
          open={selected_filter === "Status"}
        />

        <FilterDropDown
          name="Pattern"
          activateMarket={activateRetracement}
          options={options4}
          set_selected_filter={set_selected_filter}
          selected_filter={selected_filter}
          open={selected_filter === "Pattern"}
        />

        <FilterDropDown
          name="Date"
          activateMarket={activateDate}
          options={unique_d_dates}
          set_selected_filter={set_selected_filter}
          selected_filter={selected_filter}
          open={selected_filter === "Date"}
        />

        <FilterDropDown
          name="Symbol"
          activateMarket={activateSymbol}
          options={unique_symbols}
          set_selected_filter={set_selected_filter}
          selected_filter={selected_filter}
          open={selected_filter === "Symbol"}
        />
      </div>

      <div className="main_left">
        {grouped_pattern_data.rust_patterns?.map((pattern, index) => (
          <ChartMain
            key={pattern.id ?? index}
            chart_data={{
              candles: grouped_pattern_data.candles,
              rust_patterns: pattern,
            }}
            is_loading_patterns={is_loading_patterns}
            is_sections_expanded={is_sections_expanded}
            set_sections_expanded={set_sections_expanded}
            market={market}
            set_selected_xabcd={set_selected_xabcd}
            is_selected_xabcd={is_selected_xabcd}
            set_is_selected_xabcd={set_is_selected_xabcd}
          />
        ))}
      </div>
    </>
  )
}


             {/* <ChartMain
                    
                    chart_data={{
                      candles: grouped_pattern_data.candles,
                      rust_patterns: grouped_pattern_data.rust_patterns[0],
                    }}
                    is_loading_patterns={is_loading_patterns}
                    is_sections_expanded={is_sections_expanded}
                    set_sections_expanded={set_sections_expanded}
                    market={market}
                    set_selected_xabcd={set_selected_xabcd}
                  /> */}


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










            {/* <div className={is_sections_expanded ? 'sections-wrapper-expanded':'sections-wrapper-minimized'}> */}

         
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

              {/* <Section 
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
                  set_loading_patterns={set_loading}
                  set_chart_data={set_chart_data}
                  selected_row_index={selected_row_index}
                  set_selected_row_index={set_selected_row_index}
                  update_selected_pattern={update_selected_pattern}
                />}
              /> */}





















              {/* 
            
              <div className="setting-container">
                <div className="setting-key">Pattern:</div>

                <div
                  className="setting-value-outer"
                  onClick={() => setOpen4(!open4)}
                >
                  <div className="setting-value-inner">
                    {value4}
                  </div>

                  {open4 && (
                    <div className="setting-dropdown">
                      {options4.map(opt => (
                        <div
                          key={opt}
                          className="setting-dropdown-item"
                          onClick={() => {
                            setValue4(opt);
                            // activateResult(opt === 'Open' ? "Open" : opt === 'Won' ? 'true' : 'false');
                            setOpen4(false);
                          }}
                        >
                          {opt}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              <div className="setting-container">
                <div className="setting-key">Price &lt;:</div>

                <div
                  className="setting-value-outer"
                  onClick={() => setOpen5(!open5)}
                >
                  <div className="setting-value-inner">
                    {value5}
                  </div>

                  {open5 && (
                    <div className="setting-dropdown">
                      {options5.map(opt => (
                        <div
                          key={opt}
                          className="setting-dropdown-item"
                          onClick={() => {
                            setValue5(opt);
                            // activateResult(opt === 'Open' ? "Open" : opt === 'Won' ? 'true' : 'false');
                            setOpen5(false);
                          }}
                        >
                          {opt}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>

              <div className="setting-container">
                <div className="setting-key">Symbol:</div>

                <div
                  className="setting-value-outer"
                  onClick={() => setOpen5(!open5)}
                >
                  <div className="setting-value-inner">
                    {value5}
                  </div>

                  {open5 && (
                    <div className="setting-dropdown">
                      {options5.map(opt => (
                        <div
                          key={opt}
                          className="setting-dropdown-item"
                          onClick={() => {
                            setValue5(opt);
                            // activateResult(opt === 'Open' ? "Open" : opt === 'Won' ? 'true' : 'false');
                            setOpen5(false);
                          }}
                        >
                          {opt}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </div>

 

              {/* <div className='monthly_peformance_wrapper'>

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

            </div> */}