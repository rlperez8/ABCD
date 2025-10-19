
import * as route from './backend_routes.js';

const PerformanceTable = (props) => {

    const {

        ticker_performance,
        set_candles,
        set_selected_peformance,
        selected_peformance

    } = props

    return(

        <div className='peformance_table_main'>
            {ticker_performance.map(ticker=>{
                return(
                <div onClick={()=>{
                    
                    const fetch = async () => {
                    const candles = await route.get_candles(ticker.ticker)
                    set_candles(candles)

                    set_selected_peformance(ticker.ticker)
                    }

                    fetch()

                    
                    
            
                    }} className={selected_peformance === ticker.ticker ? 'ticker_row_selected' : 'ticker_row'}>
                    <div className='ticker_column'>{ticker.ticker}</div>
                    <div className='ticker_column'>{ticker.win_pct}</div>
                    <div className='ticker_column'>{ticker.count_total}</div>
                    <div className='ticker_column'>{ticker.count_won}</div>
                    <div className='ticker_column'>{ticker.count_lost}</div>
                    <div className='ticker_column'>{ticker.count_open}</div>
                </div>)
            })}
        </div>
    )
}


export default PerformanceTable