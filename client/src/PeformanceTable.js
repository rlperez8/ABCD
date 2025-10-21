
import * as route from './backend_routes.js';

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
        filters


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
            

    return(

        <div className='peformance_table_main'>


            <div className='peformance_table_header'>
                <div className='ticker_column'>Symbol</div>
                <div className='ticker_column' onClick={()=>{sort_by_win_pct()}}>Win Pct.</div>
                <div className='ticker_column'>Total</div>
                <div className='ticker_column'>Won</div>
                <div className='ticker_column'>Lost</div>
                <div className='ticker_column'>Open</div>
            </div>

            <div className='peformance_table_body'>
                {ticker_performance.map((ticker,key)=>{

                return(
                <div 
                    key={key}
                    onClick={()=>{
                        
                        const fetch = async () => {   
                            set_ticker_symbol(ticker.ticker)
                            const candles = await route.get_candles(ticker.ticker)
                            set_candles(candles)
                            set_selected_peformance(ticker.ticker)

                            const abcd_patterns = await route.get_abcd_candles(ticker.ticker,filters)
                            set_abcd_patterns(abcd_patterns)
                            set_table(abcd_patterns)          
                        }
                      
                        fetch()
                    }} 
                    
                    className={selected_peformance === ticker.ticker ? 'ticker_row_selected' : 'ticker_row'}>
                    
                    
                    <div className='ticker_column_symbol'>{ticker.ticker}</div>
                    <div className='ticker_column' >{ticker.win_pct}%</div>
                    <div className='ticker_column'>{ticker.count_total}</div>
                    <div className='ticker_column'>{ticker.count_won}</div>
                    <div className='ticker_column'>{ticker.count_lost}</div>
                    <div className='ticker_column'>{ticker.count_open}</div>

                </div>

               
               
                )
                
            })}

            </div>

            

            
        </div>
    )
}


export default PerformanceTable