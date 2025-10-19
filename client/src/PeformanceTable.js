
import * as route from './backend_routes.js';
import Table from './table';
import React, { useState, useEffect, useRef } from 'react';
const PerformanceTable = (props) => {

    const {

        ticker_performance,
        set_candles,
        set_selected_peformance,
        selected_peformance,

        selected_pattern_index,
        set_selected_pattern_index,
        set_selected_pattern,
        table,
        abcd_patterns


    } = props

    return(

        <div className='peformance_table_main'>
            {ticker_performance.map((ticker,key)=>{
                return(
                    <>
                <div 
                    key={key}
                    onClick={()=>{
                
                        const fetch = async () => {
                            const candles = await route.get_candles(ticker.ticker)
                            set_candles(candles)
                            set_selected_peformance(ticker.ticker)
                            await route.get_abcd_candles(ticker.ticker)
                        }
                        fetch()
                    }} 
                    
                    className={selected_peformance === ticker.ticker ? 'ticker_row_selected' : 'ticker_row'}>
                    
                    
                    <div className='ticker_column'>{ticker.ticker}</div>
                    <div className='ticker_column'>{ticker.win_pct}</div>
                    <div className='ticker_column'>{ticker.count_total}</div>
                    <div className='ticker_column'>{ticker.count_won}</div>
                    <div className='ticker_column'>{ticker.count_lost}</div>
                    <div className='ticker_column'>{ticker.count_open}</div>

                </div>

                {selected_peformance === ticker.ticker &&
                    <div className='patterns_table_main'>
                        <div className='patterns_table_inner'>
                        {table.length > 0 &&
                            <Table 
                                selected_pattern_index={selected_pattern_index}
                                set_selected_pattern={set_selected_pattern}
                                set_selected_pattern_index={set_selected_pattern_index}
                                table={table}
                                abcd_patterns={abcd_patterns}
                            />
                        }
                        </div>
                    </div> 
                }
               
                </>)
                
            })}

            
        </div>
    )
}


export default PerformanceTable