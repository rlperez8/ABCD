  useEffect(() => {
    // const fetch_new_patterns = async () => {
    //   try {
    //     set_loading(true);

    //     // Fetch performances
    //     // const filtered_peformances = await route.fetch_filtered_peformances(filters);

    //     // Check if current selected peformance is in the new filtered peformances and if not then update it to the first peformance in the new peformances
    //     // const first_symbol = filtered_peformances[0]?.ticker;
    //     // const has_symbol = filtered_peformances.some(item => item.ticker === selected_peformance);
    //     // const target_symbol = has_symbol ? selected_peformance : first_symbol;

    //     // if (!target_symbol) return null;

    
    //     // Fetch both patterns & candles in parallel
    //     let [filtered_patterns, candles] = await Promise.all([
    //       // route.get_abcd_candles(target_symbol, filters),
    //       // route.get_candles(target_symbol),
    //       // route.get_recent_patterns()
    //     ]);
    //     console.log(candles)

    //     // Update filtered_peformances[0] candles
   
    //     candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
    //     candles = candles.map(item => {
    //       const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    //       return {
    //         ...item,
    //         candle_date: toISO(item.candle_date),
    //       };
    //     });
    //     set_candles(candles)

    //     // recent_patterns.sort((a,b)=> a.trade_entered_price = b.trade_entered_price)
   



    //     // Return everything together
    //     return {
    //       // filtered_peformances,
    //       // filtered_patterns,
    //       candles,
    //       // selected_symbol: target_symbol,
    //       recent_patterns
    //     };

    //   } catch (err) {
    //     console.error("Error fetching:", err);
    //     return null;
    //   }
    // };

    // const update_state = async () => {
    //   const result = await fetch_new_patterns();
    //   if (!result) return;

    //   // Set all at once
    //   // set_ticker_peformance(result.filtered_peformances);
    //   // set_abcd_patterns(result.filtered_patterns);
    //   set_table(result.filtered_patterns);
    //   set_sorted_abcd_patterns(result.filtered_patterns)
    //   set_candles(result.candles);
    //   set_formatted_candles(result.formatted_candles);
    //   set_selected_peformance(result.selected_symbol);
    //   set_selected_pattern(result.filtered_patterns[0]);
    //   set_selected_pattern_index(0);
    //   set_recent_patterns(result.recent_patterns)

    //   set_loading(false);
    // };
    handle_updated_recent_patterns(set_recent_patterns, set_wl_patterns, set_monthly_peformance, set_chart_data, filters)
    // update_state();
  }, [filters]);
  
   // Pattern Data Sorting
  const sort_patterns = (result) => {
    
    const wonRows = table.filter(row => row.trade_result === result);
    set_sorted_abcd_patterns(wonRows)
    // set_table(wonRows)
    // set_abcd_patterns(wonRows)

  }
  const [data, set_data] = useState()
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

  // Mount
  useEffect(()=>{

    
    // const inital_load_data = async () => {
    //   try {

    //     // Update filtered peformances
    //     const filtered_peformances = await route.fetch_filtered_peformances(filters);
    //     set_ticker_peformance(filtered_peformances)

    //     // Update selected data if symbol is no longer in filtered peformances
    //     const first_peformance_symbol = filtered_peformances[0].ticker
    //     const has_symbol = filtered_peformances.some(item=>item.ticker===selected_peformance)

    //     if(!has_symbol){

    //           set_selected_peformance(first_peformance_symbol)

    //           // Updated filtered abcd patterns
    //           let filtered_patterns = await route.get_abcd_candles(first_peformance_symbol, filters)
    //           filtered_patterns = filtered_patterns.map(item => {
    //             const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    //             return {
    //               ...item,
    //               pattern_A_pivot_date: toISO(item.pattern_A_pivot_date),
    //             };
    //           });
    //           set_abcd_patterns(filtered_patterns)
    //           set_table(filtered_patterns)
    //           set_sorted_abcd_patterns(filtered_patterns)

    //           // Update filtered_peformances[0] candles
    //           let candles = await route.get_candles(first_peformance_symbol)
    //           candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));
    //           candles = candles.map(item => {
    //             const toISO = d => (d ? new Date(d).toISOString().split('T')[0] : null);

    //             return {
    //               ...item,
    //               candle_date: toISO(item.candle_date),
    //             };
    //           });
    //           set_candles(candles)

    //           const candleDates = Object.values(candles).map(c => c.candle_date);
          
            
    //           set_selected_pattern(filtered_patterns[0]);  
    //           // // set_selected_pattern_index(0)

    //     }

    //     set_loading(false)

    //   } catch (err) {
    //     console.error('Error fetching candles:', err);
    //   }
    // };
    // inital_load_data();
  
  }, []) 

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
  
const [abcd_patterns, set_abcd_patterns] = useState([])
const [table, set_table] = useState([])
const [sorted_abcd_patterns, set_sorted_abcd_patterns] = useState()

const [selected_pattern, set_selected_pattern] = useState()
const [candles, set_candles] = useState([])
const [ab_candles, set_ab_candles] = useState([])
const [abc_patterns, set_abc_patterns] = useState([])
const [selected_peformance, set_selected_peformance] = useState('')
const [selected_peformance_index, set_peformance_index] = useState(0)
const [watchlist, set_watchlist] = useState([])

// Pattern
const [selected_pattern_index, set_selected_pattern_index] = useState(0)
  let [chart_height, set_chart_height] = useState(0)
  const chart_container = useRef(null)




      {/* <Filter
              filters={filters}
              set_filters={set_filters}
              fetch_filtered_peformances={route.fetch_filtered_peformances}
              set_ticker_peformance={set_ticker_peformance}
              is_loading={is_loading}
              set_loading={set_loading}
              
            />

          

              {is_loading && <div className='overlay'>
                    <div className='loading_container'>Loading...</div>
  

                  </div>}

                  

            <PerformanceTable
              set_ticker_peformance={set_ticker_peformance}
              ticker_performance={ticker_performance}
              set_candles={set_candles}
              set_selected_peformance={set_selected_peformance}
              selected_peformance={selected_peformance}

              set_ticker_symbol={set_ticker_symbol}     
              set_abcd_patterns={set_abcd_patterns}
              set_table={set_table}   
              filters={filters}
              is_loading_patterns={is_loading_patterns}
              set_loading_patterns={set_loading_patterns}
              selected_peformance_index={selected_peformance_index}
              set_peformance_index={set_peformance_index}
              set_selected_pattern={set_selected_pattern}
              set_sorted_abcd_patterns={set_sorted_abcd_patterns}
            />

            <PatternTable
              table={table}
              selected_pattern_index={selected_pattern_index}
              set_selected_pattern={set_selected_pattern}
              set_selected_pattern_index={set_selected_pattern_index}
              abcd_patterns={table}
              is_loading_patterns={is_loading_patterns}
              sort_patterns={sort_patterns}
              set_sorted_abcd_patterns={set_sorted_abcd_patterns}
              sorted_abcd_patterns={sorted_abcd_patterns}
            />
              */}