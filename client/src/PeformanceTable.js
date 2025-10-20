
import * as route from './backend_routes.js';

const PerformanceTable = (props) => {

    const {

        ticker_performance,
        set_candles,
        set_selected_peformance,
        selected_peformance,
        set_ticker_symbol,
        set_abcd_patterns,
        set_table

    } = props
    // console.log(selected_peformance)

    return(

        <div className='peformance_table_main'>


            <div className='peformance_table_header'>
                <div className='ticker_column'>Symbol</div>
                <div className='ticker_column'>Win Pct.</div>
                <div className='ticker_column'>Total</div>
                <div className='ticker_column'>Won</div>
                <div className='ticker_column'>Lost</div>
                <div className='ticker_column'>Open</div>

            </div>

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

                            const abcd_patterns = await route.get_abcd_candles(ticker.ticker)
                            set_abcd_patterns(abcd_patterns)
                            set_table(abcd_patterns)

                            

                            

                            
                }
                        fetch()
                    }} 
                    
                    className={selected_peformance === ticker.ticker ? 'ticker_row_selected' : 'ticker_row'}>
                    
                    
                    <div className='ticker_column_symbol'>{ticker.ticker}</div>
                    <div className='ticker_column'>{ticker.win_pct}%</div>
                    <div className='ticker_column'>{ticker.count_total}</div>
                    <div className='ticker_column'>{ticker.count_won}</div>
                    <div className='ticker_column'>{ticker.count_lost}</div>
                    <div className='ticker_column'>{ticker.count_open}</div>

                </div>

               
               
                )
                
            })}

            
        </div>
    )
}


export default PerformanceTable