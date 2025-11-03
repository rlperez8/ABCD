
import * as route from './backend_routes.js';
import settings from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/SETTINGS.png';
import calendar from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/calendar2.png';
import dropup from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/arrow_up.png';
import dropdown from 'C:/Users/rpere/Desktop/abcd_local_v3/client/src/images/dropdown.png';
import React, {useState} from 'react';
const PerformanceTable = (props) => {

    const {

        set_ticker_peformance,
        ticker_performance,
        set_candles,
        set_selected_peformance,
        selected_peformance,
        set_ticker_symbol,
        set_abcd_patterns,
        set_table,
        filters,
        is_loading_patterns,
        set_loading_patterns,
        selected_peformance_index,
        set_peformance_index,
        set_selected_pattern,
        set_sorted_abcd_patterns


    } = props

    const [win_sort_desc, set_win_sort_desc] = useState(true);

    const sort_by_win_pct = () => {
        const sorted = [...ticker_performance].sort(
            (a, b) => win_sort_desc
                ? Number(b.win_pct) - Number(a.win_pct)
                : Number(a.win_pct) - Number(b.win_pct)
        );
        set_ticker_peformance(sorted);
        set_win_sort_desc(!win_sort_desc);
    }
    
    const [is_collapse, set_collapse] = useState(false)

    return(

        <div className={is_collapse ? 'patterns_table_main_expanded' : 'patterns_table_main' }>

            <div className='table_header_main'>
                <div className='table_header_text'>Symbol Peformance</div>
                <div className='settings_icon'># {ticker_performance.length}</div>
                
                <div className='settings_icon' onClick={()=>{set_collapse(!is_collapse)}}>
                    <img className='icon_img' src={is_collapse ? dropdown : dropup}/>
                </div>

                 
            </div>

            <div className='patterns_settings'>
            <div className='peformance-date-icon'>
                <img className='' src={calendar}/>
            </div>
            </div>

            <div className='peformance_table_header'>
                <div className='ticker_column'>Symbol</div>
                <div className='ticker_column' onClick={()=>{sort_by_win_pct()}}>Win Pct.</div>
                <div className='ticker_column'>Total</div>
                <div className='ticker_column'>Won</div>
                <div className='ticker_column'>Lost</div>
                <div className='ticker_column'>Open</div>
                <div className='ticker_column'>Avg Vol</div>
            </div>

            <div className='table_body_main'>
                <div className='peformance_table_body'>
                    {ticker_performance.map((ticker,key)=>{

                    return(
                    <div 
                        key={key}
                        onClick={() => {
                            const fetchData = async () => {
                                try {
                                    set_loading_patterns(true);
                                    set_ticker_symbol(ticker.ticker);
                                    set_selected_peformance(ticker.ticker);
                                    set_peformance_index(key);

                                    // Run both requests in parallel and return results
                                    const [candles, abcd_patterns] = await Promise.all([
                                        route.get_candles(ticker.ticker),
                                        route.get_abcd_candles(ticker.ticker, filters)
                                    ]);

                                    candles.sort((a, b) => new Date(b.candle_date) - new Date(a.candle_date));

                                    return { candles, abcd_patterns };
                                } catch (error) {
                                    console.error('Error fetching data:', error);
                                    return { candles: [], abcd_patterns: [] };
                                } finally {
                                    set_loading_patterns(false);
                                }
                            };

                            

                            fetchData().then(({ candles, abcd_patterns }) => {
                                set_candles(candles);
                                set_abcd_patterns(abcd_patterns);
                                set_table(abcd_patterns);
                                set_sorted_abcd_patterns(abcd_patterns)
                                set_selected_pattern(abcd_patterns[0])
                            });
                        }}
            
                        className={
                            selected_peformance === ticker.ticker
                                ? 'ticker_row_selected'
                                : key % 2 === 0
                                ? 'ticker_row_even'
                                : 'ticker_row_odd'
                            }>
                        
                        
                        <div className='ticker_column_symbol'>{ticker.ticker}</div>
                        <div className='ticker_cell' >{ticker.win_pct}%</div>
                        <div className='ticker_cell'>{ticker['count_total']}</div>
                        <div className='ticker_cell'>{ticker.count_won}</div>
                        <div className='ticker_cell'>{ticker.count_lost}</div>
                        <div className='ticker_cell'>{ticker.count_open}</div>
                        <div className='ticker_cell'>{ticker.avg_volume.toLocaleString()}</div>


                    </div>

                    
                    
                    )
                    
                })}

                </div>
            </div>

        </div>

    )
}


export default PerformanceTable